use savefile_abi::RawAbiCallResult::AbiError;
use savefile_abi::{AbiConnection, AbiExportable};

#[savefile_abi_exportable(version = 0)]
pub trait InterfaceV1 {
    fn old(&self);
    fn add(&self, x: u32, y: u32) -> u32;
    fn mul(&self, x: u32, y: u32) -> u32;
}

#[savefile_abi_exportable(version = 0)]
pub trait InterfaceV2 {
    fn newer1(&self);
    fn newer2(&self);
    fn mul(&self, x: u32, y: u32) -> u32;
    fn add(&self, x: u32, y: u32) -> u32;
}

#[derive(Default)]
struct Implementation1 {}

#[derive(Default)]
struct Implementation2 {}

impl InterfaceV1 for Implementation1 {
    fn old(&self) {}

    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
    fn mul(&self, x: u32, y: u32) -> u32 {
        x * y
    }
}
savefile_abi_export!(Implementation1, InterfaceV1);
impl InterfaceV2 for Implementation2 {
    fn newer1(&self) {}

    fn newer2(&self) {}

    fn mul(&self, x: u32, y: u32) -> u32 {
        x * y
    }
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}
savefile_abi_export!(Implementation2, InterfaceV2);

#[test]
pub fn test_caller_has_older_version() {
    let iface2: Box<dyn InterfaceV2> = Box::new(Implementation2 {});
    let conn1 = unsafe {
        AbiConnection::<dyn InterfaceV1>::from_boxed_trait_for_test(
            <dyn InterfaceV2 as AbiExportable>::ABI_ENTRY,
            iface2,
        )
    }
    .unwrap();

    assert_eq!(conn1.add(2, 3), 5);
    assert_eq!(conn1.mul(2, 3), 6);
}

#[test]
pub fn test_caller_has_newer_version() {
    let iface1: Box<dyn InterfaceV1> = Box::new(Implementation1 {});
    let conn1 = unsafe {
        AbiConnection::<dyn InterfaceV2>::from_boxed_trait_for_test(
            <dyn InterfaceV1 as AbiExportable>::ABI_ENTRY,
            iface1,
        )
    }
    .unwrap();

    assert_eq!(conn1.add(2, 3), 5);
    assert_eq!(conn1.mul(2, 3), 6);
}

#[test]
#[should_panic(expected = "Method 'old' does not exist in implementation.")]
pub fn test_calling_removed_method() {
    let iface2: Box<dyn InterfaceV2> = Box::new(Implementation2 {});
    let conn1 = unsafe {
        AbiConnection::<dyn InterfaceV1>::from_boxed_trait_for_test(
            <dyn InterfaceV2 as AbiExportable>::ABI_ENTRY,
            iface2,
        )
    }
    .unwrap();

    conn1.old();
}

#[test]
#[should_panic(expected = "Method 'newer1' does not exist in implementation.")]
pub fn test_calling_not_yet_existing_method() {
    let iface1: Box<dyn InterfaceV1> = Box::new(Implementation1 {});
    let conn1 = unsafe {
        AbiConnection::<dyn InterfaceV2>::from_boxed_trait_for_test(
            <dyn InterfaceV1 as AbiExportable>::ABI_ENTRY,
            iface1,
        )
    }
    .unwrap();

    conn1.newer1();
}
