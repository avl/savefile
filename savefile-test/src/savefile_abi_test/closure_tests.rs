#![allow(non_camel_case_types)]
use savefile_abi::{AbiConnection, AbiExportable};
use savefile_abi_test::basic_abi_tests::{CallbackImpl, TestInterface, TestInterfaceImpl};
use savefile_abi_test::closure_tests::new_version::ExampleImplementationNewer;

#[derive(Savefile)]
pub struct CustomArg {
    pub x: u32,
}

#[savefile_abi_exportable(version = 0)]
pub trait Example {
    fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32);
    fn call_closure(&self, simple: &dyn Fn(u32, &u32) -> u32);
    fn call_closure_with_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> u32) -> u32;

    fn call_closure_return_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> CustomArg) -> u32;
}

pub mod new_version {

    #[derive(Savefile)]
    pub struct CustomArg {
        pub x: u32,
        #[savefile_versions = "1.."]
        pub y: String,
    }
    #[savefile_abi_exportable(version = 1)]
    pub trait Example {
        fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32);
        fn call_closure(&self, simple: &dyn Fn(u32, &u32) -> u32);
        fn call_closure_with_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> u32) -> u32;
        fn call_closure_return_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> CustomArg) -> u32;
    }
    pub struct ExampleImplementationNewer {}
    impl Example for ExampleImplementationNewer {
        fn call_mut_closure(&self, _simple: &mut dyn FnMut(u32, &u32) -> u32) {
            todo!()
        }

        fn call_closure(&self, _simple: &dyn Fn(u32, &u32) -> u32) {
            todo!()
        }

        fn call_closure_with_custom_arg(&self, _simple: &dyn Fn(&CustomArg) -> u32) -> u32 {
            todo!()
        }

        fn call_closure_return_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> CustomArg) -> u32 {
            simple(&CustomArg {
                x: 42,
                y: "hello".to_string(),
            })
            .x
        }
    }
}

struct ExampleImplementation {}
impl Example for ExampleImplementation {
    fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32) {
        println!("Output: {}", simple(43, &42));
    }
    fn call_closure(&self, simple: &dyn Fn(u32, &u32) -> u32) {
        println!("Output: {}", simple(43, &42));
    }

    fn call_closure_with_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> u32) -> u32 {
        let t = simple(&CustomArg { x: 42 });
        println!("Output: {}", t);
        t
    }

    fn call_closure_return_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> CustomArg) -> u32 {
        let x = simple(&CustomArg { x: 42 });
        x.x
    }
}

#[test]
fn test_closure() {
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    conn.call_closure(&|x, y| (x + y));
}

#[test]
fn test_mut_closure() {
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let mut num_calls = 0;
    conn.call_mut_closure(&mut |x, y| {
        num_calls += 1;

        x + y
    });
    assert_eq!(num_calls, 1);
}

#[test]
fn test_closure_with_custom_arg() {
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let result = conn.call_closure_with_custom_arg(&|arg| arg.x);
    assert_eq!(result, 42);
}

#[test]
fn test_closure_with_custom_arg_call_older() {
    let iface1: Box<dyn Example> = Box::new(ExampleImplementation {});
    let conn =
        unsafe { AbiConnection::<dyn Example>::from_boxed_trait_for_test(<dyn Example>::ABI_ENTRY, iface1) }.unwrap();

    let result = conn.call_closure_with_custom_arg(&|arg| arg.x);
    assert_eq!(result, 42);
}
#[test]
fn test_closure_with_custom_return_call_older() {
    let iface1: Box<dyn Example> = Box::new(ExampleImplementation {});
    let conn = unsafe {
        AbiConnection::<dyn new_version::Example>::from_boxed_trait_for_test(<dyn Example>::ABI_ENTRY, iface1)
    }
    .unwrap();

    //let old_def = <dyn ExampleNewer as AbiExportable>::get_definition(0);
    //println!("Old def: {:#?}", old_def);
    {
        use savefile_abi_test::closure_tests::new_version::Example;
        let result = conn.call_closure_return_custom_arg(&|arg| new_version::CustomArg {
            x: arg.x,
            y: "hej".to_string(),
        });

        assert_eq!(result, 42);
    }

    _ = conn;
}
#[test]
fn test_closure_with_custom_return_call_newer() {
    let iface1: Box<dyn new_version::Example> = Box::new(ExampleImplementationNewer {});
    let conn = unsafe {
        AbiConnection::<dyn Example>::from_boxed_trait_for_test(<dyn new_version::Example>::ABI_ENTRY, iface1)
    }
    .unwrap();

    let result = conn.call_closure_return_custom_arg(&|arg| CustomArg { x: arg.x });
    assert_eq!(result, 42);

    _ = conn;
}
