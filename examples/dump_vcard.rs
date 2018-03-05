extern crate carddav;
extern crate failure;

use failure::Error;
use carddav::{Credentials, CardDAV};

fn main_err() -> Result<(), Error> {
    let cr = Credentials::new("email@example.com", "secretpassword", "https://carddav.fastmail.com");
    let cd = CardDAV::from_credentials(cr);

    for addressbook in cd.addressbooks()? {
        println!("{}", addressbook.vcard_dump()?);
    }

    Ok(())
}

fn main() {
    main_err().unwrap();
}
