use savefile::prelude::Savefile;
use std::marker::PhantomData;

#[derive(Savefile, Debug, PartialEq)]
#[repr(u32)]
#[savefile_doc_hidden]
pub enum Example {
    A(u32, u32),
    B { a: u32, b: u32, c: u32 },
}

/*#[derive(Debug, Savefile, PartialEq)]
pub enum TestStructEnum {
    Variant2 { a: u8, b: u8 },
}
*/
#[test]
fn test() {}
