use crate::assert_roundtrip;
use savefile::prelude::*;
use std::fmt::Debug;
use std::marker::{PhantomData, PhantomPinned};
use savefile::TIGHT;

#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric<T> {
    pub x: T,
}

#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric2<T: Debug + Serialize + Deserialize> {
    pub x: T,
}

#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric3<T: Debug>
where
    T: Serialize + Deserialize,
{
    pub x: T,
}
#[derive(Savefile, Debug, PartialEq)]
pub struct ExampleGeneric4<T: Debug> {
    phantom: PhantomData<T>,
}

#[derive(Savefile, Debug, PartialEq)]
pub enum ExampleGenericEnum<T> {
    Value1,
    Value2(T),
}

#[test]
pub fn test_generic_example_u32() {
    let a = ExampleGeneric { x: 42u32 };
    assert_roundtrip(a);
}

#[test]
pub fn test_generic_example_string() {
    let a = ExampleGeneric { x: "hej".to_string() };
    assert_roundtrip(a);
}
#[test]
pub fn test_generic_example2_string() {
    let a = ExampleGeneric2 { x: "hej".to_string() };
    assert_roundtrip(a);
}
#[test]
pub fn test_generic_example3_tuple() {
    let a = ExampleGeneric3 {
        x: ("hej".to_string(), 42u32),
    };
    assert_roundtrip(a);
}
#[test]
pub fn test_generic_example4_phantom() {
    let a: ExampleGeneric4<String> = ExampleGeneric4 { phantom: PhantomData };
    assert_roundtrip(a);
}

#[test]
pub fn test_generic_example_enum() {
    let a = ExampleGenericEnum::Value2(42u32);
    assert_roundtrip(a);
}

#[repr(u8)]
#[derive(Savefile, Debug, PartialEq)]
pub enum ExampleGenericEnum2<T1> {
    Value1(T1),
    Value2(T1),
}
#[test]
pub fn test_generic_example_enum2() {
    let a = ExampleGenericEnum::Value2(42u8);
    assert_roundtrip(a);
    if !TIGHT {
        assert!(unsafe { ExampleGenericEnum2::<u8>::repr_c_optimization_safe(0) }.is_yes());
    }
    assert!(unsafe { ExampleGenericEnum2::<u16>::repr_c_optimization_safe(0) }.is_false());
    //Padding
}
