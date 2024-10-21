use savefile::prelude::*;

#[derive(Debug, PartialEq, Savefile)]
struct Version1 {
    a: String,
    b: Vec<String>,
    c: usize,
}
#[derive(Debug, PartialEq, Savefile)]
struct Version2 {
    a: String,
    #[savefile_versions = "0..0"]
    b: Removed<Vec<String>>,
    #[savefile_default_val = "123"]
    #[savefile_versions = "1.."]
    newb: u32,
    c: usize,
}

#[derive(Debug, PartialEq, Savefile, Clone)]
struct Version3 {
    a: String,
    #[savefile_versions = "0..0"]
    b: Removed<Vec<String>>,
    #[savefile_versions = "1..1"]
    newb: u32,
    c: usize,
    #[savefile_versions = "2.."]
    d: usize,
}
use quickcheck::{Arbitrary, Gen};

impl Arbitrary for Version3 {
    fn arbitrary(g: &mut Gen) -> Version3 {
        Version3 {
            a: String::arbitrary(g),
            b: Removed::new(),
            newb: 0,
            c: usize::arbitrary(g),
            d: usize::arbitrary(g),
        }
    }
}

#[quickcheck]
#[cfg(not(miri))]
fn test_quickcheck_version3(xs: Version3) -> bool {
    xs == roundtrip_version(xs.clone(), 2)
}

#[test]
fn simple_vertest1() {
    use crate::assert_roundtrip_to_new_version;
    let ver2: Version2 = assert_roundtrip_to_new_version(
        Version1 {
            a: "Hello".to_string(),
            b: vec!["a".to_string(), "b".to_string()],
            c: 412,
        },
        0,
        Version2 {
            a: "Hello".to_string(),
            b: Removed::new(),
            newb: 123,
            c: 412,
        },
        1,
    );

    assert_roundtrip_to_new_version(
        ver2,
        1,
        Version3 {
            a: "Hello".to_string(),
            b: Removed::new(),
            newb: 123,
            c: 412,
            d: 0,
        },
        2,
    );
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVer1 {
    Variant1,
    Variant2,
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVer2 {
    Variant1,
    Variant2,
    #[savefile_versions = "1.."]
    Variant3,
}

#[test]
fn test_versioning_of_enums() {
    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(EnumVer1::Variant1, 0, EnumVer2::Variant1, 1);
    assert_roundtrip_to_new_version(EnumVer1::Variant2, 0, EnumVer2::Variant2, 1);
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVerA1 {
    Variant1,
    Variant2 { x: u32, y: u32 },
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVerA2 {
    Variant1,
    Variant2 {
        x: u32,
        #[savefile_versions = "0..0"]
        y: Removed<u32>,
    },
}

#[test]
fn test_versioning_of_enums2() {
    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        EnumVerA1::Variant2 { x: 32, y: 33 },
        0,
        EnumVerA2::Variant2 {
            x: 32,
            y: Removed::new(),
        },
        1,
    );
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVerB1 {
    Variant1,
    Variant2(u32, u32),
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVerB2 {
    Variant1,
    Variant2(u32, #[savefile_versions = "0..0"] Removed<u32>),
}

#[test]
fn test_versioning_of_enums3() {
    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        EnumVerB1::Variant2(32, 33),
        0,
        EnumVerB2::Variant2(32, Removed::new()),
        1,
    );
}

#[derive(Debug, PartialEq, Savefile)]
struct SubSubData1 {
    x: u32,
}
#[derive(Debug, PartialEq, Savefile)]
struct SubData1 {
    some_sub: SubSubData1,
}
#[derive(Debug, PartialEq, Savefile)]
struct ComplexData1 {
    some_field: SubData1,
}

#[derive(Debug, PartialEq, Savefile)]
struct SubSubData2 {
    y: u32,
}
#[derive(Debug, PartialEq, Savefile)]
struct SubData2 {
    some_sub: SubSubData2,
}
#[derive(Debug, PartialEq, Savefile)]
struct ComplexData2 {
    some_field: SubData2,
}

#[test]
fn test_versioning_of_enums4() {
    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        ComplexData1 {
            some_field: SubData1 {
                some_sub: SubSubData1 { x: 43 },
            },
        },
        0,
        ComplexData2 {
            some_field: SubData2 {
                some_sub: SubSubData2 { y: 43 },
            },
        },
        1,
    );
}

#[derive(Debug, PartialEq, Savefile, Default)]
enum DefTraitEnum {
    #[default]
    VariantA,
    VariantB,
    VariantC,
}

#[derive(Debug, PartialEq, Savefile)]
struct DefTraitTest {
    #[savefile_versions = "1.."]
    removed_enum: DefTraitEnum,
}

#[test]
fn test_default_trait1() {
    use crate::assert_roundtrip_version;
    assert_roundtrip_version::<DefTraitTest>(
        DefTraitTest {
            removed_enum: DefTraitEnum::VariantA,
        },
        1,
        true,
    );
}

#[test]
fn test_custom_default_fn() {
    #[derive(Debug, PartialEq, Savefile)]
    struct VersionB1 {
        a: String,
    }

    fn b_default() -> String {
        "custom_default_value".to_string()
    }
    #[derive(Debug, PartialEq, Savefile)]
    struct VersionB2 {
        a: String,
        #[savefile_default_fn = "b_default"]
        #[savefile_versions = "1.."]
        b: String,
    }

    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        VersionB1 { a: "test".to_string() },
        0,
        VersionB2 {
            a: "test".to_string(),
            b: "custom_default_value".to_string(),
        },
        1,
    );
}

#[derive(Debug, PartialEq, Savefile)]
struct StructWithOneType {
    a_str: String,
}

#[derive(Debug, PartialEq, Savefile)]
struct AnewType {
    an_u32: u32,
}

use crate::{roundtrip, roundtrip_version};
use std::convert::From;

impl From<String> for AnewType {
    fn from(_dummy: String) -> AnewType {
        AnewType { an_u32: 9999 }
    }
}

#[derive(Debug, PartialEq, Savefile)]
struct StructWithAnotherType {
    #[savefile_versions_as = "0..0:String"]
    #[savefile_versions = "1.."]
    a_str: AnewType,
}

#[test]
fn test_change_type_of_field() {
    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        StructWithOneType {
            a_str: "test".to_string(),
        },
        0,
        StructWithAnotherType {
            a_str: AnewType { an_u32: 9999 },
        },
        1,
    );
}

fn convert2newtype(s: String) -> AnewType {
    AnewType {
        an_u32: s.parse().unwrap(),
    }
}
#[derive(Debug, PartialEq, Savefile)]
struct StructWithAnotherType2 {
    #[savefile_versions_as = "0..0:convert2newtype:String"]
    #[savefile_versions = "1.."]
    a_str: AnewType,
}

#[test]
fn test_change_type_of_field2() {
    use crate::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        StructWithOneType {
            a_str: "422".to_string(),
        },
        0,
        StructWithAnotherType2 {
            a_str: AnewType { an_u32: 422 },
        },
        1,
    );
}

#[derive(Debug, PartialEq, Savefile)]
struct FastVersionA0 {
    a: u32,
    b: u32,
    c: u32,
}
#[derive(Debug, PartialEq, Savefile)]
struct FastVersionA1 {
    a: u32,
    #[savefile_versions = "0..0"]
    b: Removed<u32>,
    c: u32,
}

#[test]
fn simple_vertest_a() {
    use crate::assert_roundtrip_to_new_version;
    let _ver1: FastVersionA1 = assert_roundtrip_to_new_version(
        FastVersionA0 { a: 2, b: 3, c: 4 },
        0,
        FastVersionA1 {
            a: 2,
            b: Removed::new(),
            c: 4,
        },
        1,
    );
}

#[derive(Debug, PartialEq, Savefile)]
struct FastVersionB0 {
    a: u32,
    b: u32,
}
#[derive(Debug, PartialEq, Savefile)]
struct FastVersionB1 {
    a: u32,
    b: u32,
    #[savefile_versions = "1.."]
    c: u32,
}

#[test]
fn simple_vertest_b() {
    use crate::assert_roundtrip_to_new_version;
    let _ver1: FastVersionB1 =
        assert_roundtrip_to_new_version(FastVersionB0 { a: 2, b: 3 }, 0, FastVersionB1 { a: 2, b: 3, c: 0 }, 1);
}
