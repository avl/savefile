use savefile::{AbiRemoved, SavefileError};
use savefile_abi::{AbiConnection, AbiExportable, verify_compatiblity};
use savefile_abi::RawAbiCallResult::AbiError;
use savefile_abi_test::argument_backward_compatibility::v1::{ArgInterfaceV1, Implementation1};
use savefile_abi_test::argument_backward_compatibility::v2::{ArgInterfaceV2, Implementation2};
use savefile_derive::Savefile;


mod v1 {
    #[derive(Savefile)]
    pub struct Argument {
        pub data1: u32,
        pub data2: u32,
    }
    #[savefile_abi_exportable(version=0)]
    pub trait ArgInterfaceV1 {
        fn sums(&self, a: Argument, b: Argument) -> u32;
    }
    #[derive(Default)]
    pub struct Implementation1 {}


    impl ArgInterfaceV1 for Implementation1 {
        fn sums(&self, a: Argument, b: Argument) -> u32 {
            a.data1+a.data2+b.data1+b.data2
        }
    }
}

mod v2 {
    use savefile::AbiRemoved;
    use savefile_derive::{Savefile};
    use ::savefile::prelude::*;

    #[derive(Savefile)]
    pub struct ArgArgument {
        #[savefile_versions="0..0"]
        pub data1: AbiRemoved<u32,>,
        pub data2: u32,
        #[savefile_versions="1.."]
        pub data3: u32,
    }
    #[savefile_abi_exportable(version=1)]
    pub trait ArgInterfaceV2 {
        fn sums(&self, a: ArgArgument, b: ArgArgument) -> u32;
    }

    #[derive(Default)]
    pub struct Implementation2 {}
    impl ArgInterfaceV2 for Implementation2 {
        fn sums(&self, a: ArgArgument, b: ArgArgument) -> u32 {
            a.data3+a.data2+b.data2+b.data3
        }
    }
}



#[test]
pub fn test_abi_schemas_get_def() {
    let exportable = <dyn ArgInterfaceV2 as AbiExportable>::get_definition(0);
    insta::assert_yaml_snapshot!(exportable);
}

#[test]
#[cfg(not(miri))]
pub fn test_backward_compatibility() -> Result<(), SavefileError> {
    verify_compatiblity::<dyn ArgInterfaceV2>("schemas")
}

#[test]
pub fn test_caller_has_older_version() {
    let iface2 : Box<dyn ArgInterfaceV2> = Box::new(Implementation2{});
    let conn1 = unsafe { AbiConnection::<dyn ArgInterfaceV1>::from_boxed_trait_for_test(<dyn ArgInterfaceV2 as AbiExportable>::ABI_ENTRY, iface2 ) }.unwrap();

    assert_eq!(conn1.sums(
        v1::Argument {
            data1: 2,
            data2: 3,
        },
        v1::Argument {
            data1: 4,
            data2: 5,
        },
    ), 8); //Because implementation expects data2 and data3, but we're only sending data2.
}

#[test]
pub fn test_caller_has_newer_version() {
    let iface1 : Box<dyn ArgInterfaceV1> = Box::new(Implementation1{});
    let conn1 = unsafe { AbiConnection::<dyn ArgInterfaceV2>::from_boxed_trait_for_test(<dyn ArgInterfaceV1 as AbiExportable>::ABI_ENTRY, iface1 ) }.unwrap();

    assert_eq!(conn1.sums(
        v2::ArgArgument {
            data1: AbiRemoved::new(),
            data2: 1,
            data3: 2
        },
        v2::ArgArgument {
            data1: AbiRemoved::new(),
            data2: 3,
            data3: 4
        },
    ), 4); //Because implementation expects data1 and data2, but we're only sending data2.
}
