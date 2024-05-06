use assert_roundtrip_version;
use savefile::{Packed, Removed};
use {assert_roundtrip, assert_roundtrip_to_new_version};

#[repr(u8)]
#[derive(Savefile, Debug, PartialEq)]
pub enum EnumAVer1 {
    Var1,
    Var2,
}

#[repr(u16)]
#[derive(Savefile, Debug, PartialEq)]
pub enum EnumAVer2 {
    Var1,
    Var2,
}
#[repr(u32)]
#[derive(Savefile, Debug, PartialEq)]
pub enum EnumAVer3 {
    Var1,
    Var2,
}

#[test]
#[should_panic(
    expected = "Saved schema differs from in-memory schema for version 0. Error: At location [.EnumAVer1]: In memory enum has a representation with 2 bytes for the discriminant, but disk format has 1."
)]
fn test_change_of_discriminant_size() {
    assert_roundtrip_to_new_version(EnumAVer1::Var1, 0, EnumAVer2::Var1, 1);
}
#[test]
#[should_panic(
    expected = "Saved schema differs from in-memory schema for version 0. Error: At location [.EnumAVer2]: In memory enum has a representation with 1 bytes for the discriminant, but disk format has 2."
)]
fn test_change_of_discriminant_size2() {
    assert_roundtrip_to_new_version(EnumAVer2::Var1, 0, EnumAVer1::Var1, 1);
}

#[test]
#[should_panic(
    expected = "Saved schema differs from in-memory schema for version 0. Error: At location [.EnumAVer2]: In memory enum has a representation with 4 bytes for the discriminant, but disk format has 2."
)]
fn test_change_of_discriminant_size3() {
    assert_roundtrip_to_new_version(EnumAVer2::Var1, 0, EnumAVer3::Var1, 1);
}
#[test]
#[should_panic(
    expected = "Saved schema differs from in-memory schema for version 0. Error: At location [.EnumAVer3]: In memory enum has a representation with 2 bytes for the discriminant, but disk format has 4."
)]
fn test_change_of_discriminant_size4() {
    assert_roundtrip_to_new_version(EnumAVer3::Var1, 0, EnumAVer2::Var1, 1);
}

#[derive(Savefile, Debug, PartialEq)]
pub enum EnumBVer1 {
    Var1,
    Var2,
}

#[derive(Savefile, Debug, PartialEq)]
pub enum EnumBVer2 {
    Var1,
    Var2,
    #[savefile_versions = "1.."]
    Var3,
}

#[test]
fn test_change_add_enum_variants() {
    assert_roundtrip_to_new_version(EnumBVer1::Var1, 0, EnumBVer2::Var1, 0);
}

#[derive(Savefile, Debug, PartialEq)]
pub enum EnumBVer3 {
    Var1,
    Var2,
    #[savefile_versions = "1.."]
    Var3,
    #[savefile_versions = "2.."]
    Var4,
}
#[test]
fn test_change_add_enum_variants2() {
    assert_roundtrip_to_new_version(EnumBVer2::Var3, 1, EnumBVer3::Var3, 1);
}
#[test]
#[should_panic(expected = "Enum EnumBVer2, variant Var3 is not present in version 0")]
fn test_change_add_enum_variants3() {
    assert_roundtrip_to_new_version(EnumBVer2::Var3, 0, EnumBVer3::Var3, 0);
}

#[derive(Savefile, Debug, PartialEq)]
#[repr(u32)]
pub enum EnumCVer2 {
    Var1,
    Var2 {
        #[savefile_versions = "..0"]
        a: Removed<u32>,
        b: u32,
    },
}
#[test]
fn test_change_remove_enum_field() {
    assert!(unsafe { EnumCVer2::repr_c_optimization_safe(0) }.is_false());
    assert!(unsafe { EnumCVer2::repr_c_optimization_safe(1) }.is_yes());
    assert_roundtrip_version(
        EnumCVer2::Var2 {
            b: 42,
            a: Removed::new(),
        },
        1,
        true,
    );
}
