use savefile_derive::{Savefile, savefile_abi_exportable};

#[derive(Debug, Savefile, PartialEq)]
pub enum TestStructEnum {
    Variant2 { a: u8, b: u8 },
}


#[savefile_abi_exportable(version = 0)]
pub trait AdderCallback {
    fn set(&self, value: u32);
    fn get(&self) -> Box<str>;
}

#[test]
fn test() {

}
