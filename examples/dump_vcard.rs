extern crate carddav;
extern crate failure;

use failure::Error;
use carddav::{Credentials, CardDAV};

fn main_err() -> Result<(), Error> {
    let cr = Credentials::new("test", "1234", "http://localhost:5232");
    let cd = CardDAV::from_credentials(cr);

    for addressbook in cd.addressbooks()? {
        addressbook.create_contact("test-id", include_str!("/home/leonardo/.contacts/2f19599a-fa69-43c8-9444-0973b9472f25/7be5df25-c148-4a1c-8e5e-e8a00ea39430.vcf"))?;
        println!("{:?}", addressbook);
        println!("{}", addressbook.vcard_dump()?);
    }

    Ok(())
}

fn main() {
    main_err().unwrap();
}
