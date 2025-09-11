use savefile::prelude::Savefile;
use std::marker::PhantomData;

#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric<T> {
    pub x: PhantomData<T>,
}

/*#[derive(Debug, Savefile, PartialEq)]
pub enum TestStructEnum {
    Variant2 { a: u8, b: u8 },
}
*/
#[test]
fn test() {}
