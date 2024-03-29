use std::fmt::Debug;
use std::marker::{PhantomData, PhantomPinned};
use ::savefile::prelude::*;
use assert_roundtrip;


#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric<T> {
    pub x: T
}

#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric2<T:Debug+Serialize+Deserialize> {
    pub x: T
}

#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric3<T:Debug> where T: Serialize+Deserialize {
    pub x: T
}
#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric4<T:Debug> {
    phantom: PhantomData<T>,
}


#[derive(Savefile, Debug, PartialEq)]
pub enum ExampleGenericEnum<T> {
    Value1,
    Value2(T),
}


#[test]
pub fn test_generic_example_u32() {
    let a = ExampleGeneric{x:42u32};
    assert_roundtrip(a);
}


#[test]
pub fn test_generic_example_string() {
    let a = ExampleGeneric{x:"hej".to_string()};
    assert_roundtrip(a);
}
#[test]
pub fn test_generic_example2_string() {
    let a = ExampleGeneric2{x:"hej".to_string()};
    assert_roundtrip(a);
}
#[test]
pub fn test_generic_example3_tuple() {
    let a = ExampleGeneric3{x:("hej".to_string(),42u32)};
    assert_roundtrip(a);
}
#[test]
pub fn test_generic_example4_phantom() {
    let a:ExampleGeneric4<String> = ExampleGeneric4{phantom: PhantomData};
    assert_roundtrip(a);
}

#[test]
pub fn test_generic_example_enum() {
    let a = ExampleGenericEnum::Value2(42u32);
    assert_roundtrip(a);
}