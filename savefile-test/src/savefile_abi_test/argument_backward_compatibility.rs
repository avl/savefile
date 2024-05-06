use savefile::prelude::AbiRemoved;
use savefile::{get_schema, SavefileError, WithSchemaContext};
use savefile_abi::RawAbiCallResult::AbiError;
use savefile_abi::{verify_compatiblity, AbiConnection, AbiExportable};
use savefile_abi_test::argument_backward_compatibility::v1::{ArgInterfaceV1, EnumArgument, Implementation1};
use savefile_abi_test::argument_backward_compatibility::v2::{ArgInterfaceV2, Implementation2};
use savefile_abi_test::basic_abi_tests::CowSmuggler;
use savefile_derive::Savefile;

mod v1 {
    #[derive(Savefile)]
    pub struct Argument {
        pub data1: u32,
        pub data2: u32,
    }

    #[derive(Savefile)]
    pub enum EnumArgument {
        Variant1,
        Variant2,
    }

    #[savefile_abi_exportable(version = 0)]
    pub trait ArgInterfaceV1 {
        fn sums(&self, a: Argument, b: Argument) -> u32;
        fn enum_arg(&self, a: EnumArgument) -> String;
        fn function_existing_in_v1(&self);
    }
    #[derive(Default)]
    pub struct Implementation1 {}

    impl ArgInterfaceV1 for Implementation1 {
        fn sums(&self, a: Argument, b: Argument) -> u32 {
            a.data1 + a.data2 + b.data1 + b.data2
        }
        fn enum_arg(&self, a: EnumArgument) -> String {
            match a {
                EnumArgument::Variant1 => "Variant1".into(),
                EnumArgument::Variant2 => "Variant2".into(),
            }
        }
        fn function_existing_in_v1(&self) {}
    }
}

mod v2 {
    use savefile::prelude::*;
    use savefile::AbiRemoved;
    use savefile_derive::Savefile;

    #[derive(Savefile, Debug)]
    pub struct ArgArgument {
        #[savefile_versions = "0..0"]
        pub data1: AbiRemoved<u32>,
        pub data2: u32,
        #[savefile_versions = "1.."]
        pub data3: u32,
    }
    #[derive(Savefile)]
    pub enum EnumArgument {
        Variant1,
        Variant2,
        #[savefile_versions = "1.."]
        Variant3,
    }

    #[savefile_abi_exportable(version = 1)]
    pub trait ArgInterfaceV2 {
        fn sums(&self, a: ArgArgument, b: ArgArgument) -> u32;
        fn enum_arg(&self, a: EnumArgument) -> String {
            match a {
                EnumArgument::Variant1 => "Variant1".into(),
                EnumArgument::Variant2 => "Variant2".into(),
                EnumArgument::Variant3 => "Variant3".into(),
            }
        }
        fn function_existing_in_v2(&self);
    }

    #[derive(Default)]
    pub struct Implementation2 {}
    impl ArgInterfaceV2 for Implementation2 {
        fn sums(&self, a: ArgArgument, b: ArgArgument) -> u32 {
            a.data3 + a.data2 + b.data2 + b.data3
        }

        fn function_existing_in_v2(&self) {}
    }
}

#[test]
#[cfg(not(miri))]
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
pub fn test_arg_argument_metadata() {
    use savefile::WithSchema;
    let schema = get_schema::<v2::ArgArgument>(0);
    println!("Schema: {:#?}", schema);
    assert!(!schema.layout_compatible(&schema)); //Versions containing removed items should never be considered layout compatible (since their schema type is not identical to the memory type)
}

#[test]
pub fn test_caller_has_older_version() {
    let iface2: Box<dyn ArgInterfaceV2> = Box::new(Implementation2 {});
    assert_eq!(
        iface2.sums(
            v2::ArgArgument {
                data2: 3,
                data3: 2,
                data1: AbiRemoved::new()
            },
            v2::ArgArgument {
                data2: 3,
                data3: 2,
                data1: AbiRemoved::new()
            }
        ),
        10
    );

    let conn1 = unsafe {
        AbiConnection::<dyn ArgInterfaceV1>::from_boxed_trait_for_test(
            <dyn ArgInterfaceV2 as AbiExportable>::ABI_ENTRY,
            iface2,
        )
    }
    .unwrap();

    let s = conn1.sums(v1::Argument { data1: 2, data2: 3 }, v1::Argument { data1: 4, data2: 5 });
    println!("Sum: {}", s);
    assert_eq!(s, 8); //Because implementation expects data2 and data3, but we're only sending data2.

    assert_eq!(conn1.enum_arg(EnumArgument::Variant1), "Variant1".to_string());
}

#[test]
pub fn test_caller_has_newer_version() {
    let iface1: Box<dyn ArgInterfaceV1> = Box::new(Implementation1 {});
    let conn1 = unsafe {
        AbiConnection::<dyn ArgInterfaceV2>::from_boxed_trait_for_test(
            <dyn ArgInterfaceV1 as AbiExportable>::ABI_ENTRY,
            iface1,
        )
    }
    .unwrap();

    assert_eq!(
        conn1.sums(
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
        ),
        4
    ); //Because implementation expects data1 and data2, but we're only sending data2.

    assert_eq!(conn1.enum_arg(v2::EnumArgument::Variant1), "Variant1".to_string());
}

#[test]
#[should_panic(expected = "Enum EnumArgument, variant Variant3 is not present in version 0")]
pub fn test_caller_has_newer_version_and_uses_enum_that_callee_doesnt_have() {
    let iface1: Box<dyn ArgInterfaceV1> = Box::new(Implementation1 {});
    let conn1 = unsafe {
        AbiConnection::<dyn ArgInterfaceV2>::from_boxed_trait_for_test(
            <dyn ArgInterfaceV1 as AbiExportable>::ABI_ENTRY,
            iface1,
        )
    }
    .unwrap();

    assert_eq!(conn1.enum_arg(v2::EnumArgument::Variant3), "Variant3".to_string());
}

#[test]
#[should_panic(expected = "'function_existing_in_v2' does not exist in implementation.")]
pub fn test_caller_has_newer_version_calling_non_existing_function() {
    let iface1: Box<dyn ArgInterfaceV1> = Box::new(Implementation1 {});
    let conn1 = unsafe {
        AbiConnection::<dyn ArgInterfaceV2>::from_boxed_trait_for_test(
            <dyn ArgInterfaceV1 as AbiExportable>::ABI_ENTRY,
            iface1,
        )
    }
    .unwrap();
    conn1.function_existing_in_v2();
}

#[test]
#[should_panic(expected = "'function_existing_in_v1' does not exist in implementation.")]
pub fn test_caller_has_older_version_calling_non_existing_function() {
    let iface2: Box<dyn ArgInterfaceV2> = Box::new(Implementation2 {});
    let conn = unsafe {
        AbiConnection::<dyn ArgInterfaceV1>::from_boxed_trait_for_test(
            <dyn ArgInterfaceV2 as AbiExportable>::ABI_ENTRY,
            iface2,
        )
    }
    .unwrap();
    conn.function_existing_in_v1();
}
#[test]
fn test_calling_function_that_is_later_removed() {
    let boxed: Box<dyn ArgInterfaceV1> = Box::new(Implementation1 {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    conn.function_existing_in_v1();
}
#[test]
fn test_calling_function_that_is_added_in_later_version() {
    let boxed: Box<dyn ArgInterfaceV2> = Box::new(Implementation2 {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    conn.function_existing_in_v2();
}
