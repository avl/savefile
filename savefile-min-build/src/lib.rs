use savefile_derive::Savefile;

#[derive(Debug, Savefile, PartialEq)]
pub enum TestStructEnum {
    Variant2 { a: u8, b: u8 },
}

#[test]
fn test() {}
