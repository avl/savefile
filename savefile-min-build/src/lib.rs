
extern crate savefile_abi;
extern crate savefile_derive;


use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
#[repr(u8)]
#[derive(Savefile, Debug, PartialEq)]
pub enum ExampleGenericEnum2<T1> {
    Value1(T1),
    Value2(T1),
}
#[test]
pub fn test_generic_example_enum2() {
    let a = ExampleGenericEnum2::Value2(42u8);
    assert!(unsafe{ExampleGenericEnum2::<u8>::repr_c_optimization_safe(0)}.is_yes());
}

