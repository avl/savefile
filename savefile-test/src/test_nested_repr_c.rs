#![cfg(test)]
use savefile::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
#[savefile_unsafe_and_fast]
struct Inner {
    x: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
struct Nested {
    misaligner: u8,
    inner: Inner,
}

#[allow(unused)]
#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
#[savefile_unsafe_and_fast]
#[repr(u8)]
pub enum TestReprEnum {
    A,
    B,
    C,
}

#[test]
fn test_not_raw_memcpy2() {
    use std::io::Cursor;
    let sample = vec![Nested {
        misaligner: 0,
        inner: Inner { x: 32 },
    }];

    let mut f = Cursor::new(Vec::new());
    {
        Serializer::save_noschema(&mut f, 0, &sample).unwrap();
    }

    let f_internal_size = f.get_ref().len();

    let vec_overhead = 8;
    let version = 4;
    let savefile_header = 9;
    let savefile_lib_version = 2;
    let is_compressed = 1;
    let misaligner = 1;
    let inner = 4;
    assert_eq!(
        f_internal_size,
        version + vec_overhead + misaligner + inner + savefile_header + savefile_lib_version + is_compressed
    ); //3 bytes padding also because of Packed-optimization
}

#[derive(Savefile, Clone, Copy)]
#[savefile_unsafe_and_fast]
#[repr(C)]
struct MyUnitStruct {}

#[derive(Savefile, Clone, Copy)]
#[savefile_unsafe_and_fast]
#[repr(C)]
struct UnnamedFieldsStruct(usize);

#[test]
fn test_various_types_for_reprc() {
    assert_eq!(unsafe { <() as Packed>::repr_c_optimization_safe(0).is_yes() }, true);
    assert_eq!(unsafe { <u8>::repr_c_optimization_safe(0) }.is_yes(), true);

    assert_eq!(unsafe { <MyUnitStruct>::repr_c_optimization_safe(0) }.is_yes(), true);
    assert_eq!(
        unsafe { UnnamedFieldsStruct::repr_c_optimization_safe(0) }.is_yes(),
        false
    ); //usize is 32 bit on 32 bit platforms.

    assert_eq!(unsafe { <(u32, u32)>::repr_c_optimization_safe(0) }.is_yes(), true);
    assert_eq!(unsafe { <(u32, u8)>::repr_c_optimization_safe(0) }.is_yes(), false);
    assert_eq!(
        unsafe { <(u32, u8, u8, u16)>::repr_c_optimization_safe(0) }.is_yes(),
        true
    );
}
