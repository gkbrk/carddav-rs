extern crate failure;
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate minidom;
extern crate reqwest;

use reqwest::{Client, Method, StatusCode};
use reqwest::header::{ContentType, IfNoneMatch};
use minidom::Element;
use failure::Error;

#[derive(Clone, Debug)]
pub struct Credentials {
    username: String,
    password: String,
    server: String,
}

impl Credentials {
    pub fn new(username: &str, password: &str, server: &str) -> Self {
        Credentials {
            username: username.into(),
            password: password.into(),
            server: server.into(),
        }
    }
}

header! { (Depth, "Depth") => [u8] }
lazy_static! {
    static ref PROPFIND: Method = Method::Extension("PROPFIND".into());
    static ref REPORT: Method = Method::Extension("REPORT".into());
}

#[derive(Debug, Clone)]
pub struct CardDAV {
    cred: Credentials,
    client: reqwest::Client,
}

impl CardDAV {
    pub fn from_credentials(cred: Credentials) -> Self {
        CardDAV {
            cred: cred,
            client: Client::new(),
        }
    }

    pub fn get_endpoint_url(&self) -> Result<String, Error> {
        let well_known = format!("{}/.well-known/carddav", self.cred.server);
        let resp = self.client
            .request(PROPFIND.clone(), well_known.as_str())
            .header(Depth(0))
            .basic_auth(
                self.cred.username.as_str(),
                Some(self.cred.password.as_str()),
            )
            .send()?;

        if resp.status() == StatusCode::NotFound {
            return Ok(self.cred.server.to_owned() + "/");
        }

        Ok(resp.url().clone().into_string())
    }

    pub fn get_principal(&self) -> Result<String, Error> {
        let endpoint = self.get_endpoint_url()?;
        let mut resp = self.client.request(PROPFIND.clone(), endpoint.as_str())
            .header(Depth(0))
            .header(ContentType("application/xml".parse()?))
            .basic_auth(self.cred.username.as_str(), Some(self.cred.password.as_str()))
            .body("<d:propfind xmlns:d=\"DAV:\"><d:prop><d:current-user-principal /></d:prop></d:propfind>")
            .send()?;

        let xml = resp.text()?;
        let el: Element = xml.parse()?;
        let principal = find_element_recursive(&el, "current-user-principal").unwrap();
        let href = find_element_recursive(&principal, "href")
            .unwrap()
            .texts()
            .next()
            .unwrap();

        Ok(self.cred.server.to_string() + href.into())
    }

    pub fn addressbook_home_set(&self) -> Result<String, Error> {
        let principal = self.get_principal()?;
        let mut resp = self.client
            .request(PROPFIND.clone(), principal.as_str())
            .header(Depth(0))
            .header(ContentType("application/xml".parse()?))
            .basic_auth(self.cred.username.as_str(), Some(self.cred.password.as_str()))
            .body("<d:propfind xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:carddav\"><d:prop><c:addressbook-home-set /></d:prop></d:propfind>")
            .send()?;

        let xml = resp.text()?;
        let el: Element = xml.parse()?;
        let home_set = find_element_recursive(&el, "addressbook-home-set").unwrap();
        let href = find_element_recursive(&home_set, "href")
            .unwrap()
            .texts()
            .next()
            .unwrap();

        Ok(self.cred.server.to_string() + href.into())
    }

    pub fn addressbooks(&self) -> Result<Vec<Addressbook>, Error> {
        let home_set = self.addressbook_home_set()?;
        let mut resp = self.client
            .request(PROPFIND.clone(), home_set.as_str())
            .header(Depth(1))
            .basic_auth(
                self.cred.username.as_str(),
                Some(self.cred.password.as_str()),
            )
            .send()?;

        let xml = resp.text()?;
        let el: Element = xml.parse()?;
        let books = el.children().skip(1);

        let mut address_books = Vec::new();
        for book in books {
            let prop = find_element_recursive(&book, "prop").unwrap();
            let href = book.children().find(|e| e.name() == "href").unwrap();
            let name = prop.children().find(|e| e.name() == "displayname").unwrap();

            let etag = {
                match prop.children().find(|e| e.name() == "getetag") {
                    Some(etag) => Some(etag.texts().next().unwrap_or("").into()),
                    None => None,
                }
            };

            let ctag = {
                match prop.children().find(|e| e.name() == "getctag") {
                    Some(etag) => Some(etag.texts().next().unwrap_or("").into()),
                    None => None,
                }
            };

            let addr = Addressbook {
                cd: self.clone(),
                url: href.texts().next().unwrap().into(),
                display_name: name.texts().next().unwrap().into(),
                etag: etag,
                ctag: ctag,
            };
            address_books.push(addr);
        }

        Ok(address_books)
    }
}

#[derive(Debug)]
pub struct Addressbook {
    cd: CardDAV,
    url: String,
    display_name: String,
    etag: Option<String>,
    ctag: Option<String>,
}

impl Addressbook {
    pub fn vcard_dump(&self) -> Result<String, Error> {
        let dump_url = self.cd.cred.server.to_string() + &self.url;
        let mut resp = self.cd
            .client
            .get(dump_url.as_str())
            .header(Depth(1))
            .basic_auth(
                self.cd.cred.username.as_str(),
                Some(self.cd.cred.password.as_str()),
            )
            .send()?;

        Ok(resp.text()?)
    }

    pub fn create_contact(&self, id: &str, vcard: &str) -> Result<(), Error> {
        let create_url = self.cd.cred.server.to_string() + &self.url + id + ".vcf";
        let mut resp = self.cd
            .client
            .put(create_url.as_str())
            .header(ContentType("text/vcard".parse()?))
            .header(IfNoneMatch::Any)
            .body(vcard.clone().to_string())
            .basic_auth(
                self.cd.cred.username.as_str(),
                Some(self.cd.cred.password.as_str()),
            ).send()?;

        Ok(())
    }
}

fn find_element_recursive<'a>(el: &'a Element, name: &str) -> Option<&'a Element> {
    for elem in el.children() {
        if elem.name() == name {
            return Some(elem);
        } else if elem.children().next().is_some() {
            return find_element_recursive(elem, name);
        }
    }

    None
}
