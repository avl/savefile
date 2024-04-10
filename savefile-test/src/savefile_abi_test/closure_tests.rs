#![allow(non_camel_case_types)]
use savefile_abi::{AbiConnection, AbiExportable};
use savefile_abi_test::basic_abi_tests::{CallbackImpl, TestInterface, TestInterfaceImpl};
use savefile_abi_test::closure_tests::new_version::ExampleNewer;

#[derive(Savefile)]
pub struct CustomArg {
    pub x: u32,
}

#[savefile_abi_exportable(version=0)]
pub trait Example {
    fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32);
    fn call_closure(&self, simple: &dyn Fn(u32, &u32) -> u32);
    fn call_closure_with_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> u32) -> u32;
}

pub mod new_version {

    #[derive(Savefile)]
    pub struct CustomArg {
        pub x: u32,
        #[savefile_versions="1.."]
        pub y: String
    }
    #[savefile_abi_exportable(version=1)]
    pub trait ExampleNewer {
        fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32);
        fn call_closure(&self, simple: &dyn Fn(u32, &u32) -> u32);
        fn call_closure_with_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> u32) -> u32;
    }

}

struct ExampleImplementation {

}
impl Example for ExampleImplementation {
    fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32) {
        println!("Output: {}", simple(43,&42));
    }
    fn call_closure(&self, simple: & dyn Fn(u32, &u32) -> u32) {
        println!("Output: {}", simple(43,&42));
    }

    fn call_closure_with_custom_arg(&self, simple: &dyn Fn(&CustomArg) -> u32) -> u32 {
        let t = simple(&CustomArg {x: 42});
        println!("Output: {}", t);
        t
    }
}

#[test]
fn test_closure() {
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation{});
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn Example as AbiExportable>::ABI_ENTRY, boxed).unwrap() };

    conn.call_closure(&|x,y|(x+y));
}

#[test]
fn test_mut_closure() {
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation{});
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn Example as AbiExportable>::ABI_ENTRY, boxed).unwrap() };


    let mut num_calls = 0;
    conn.call_mut_closure(&mut |x,y|{
        num_calls += 1;

        x+y
    });
    assert_eq!(num_calls, 1);
}

#[test]
fn test_closure_with_custom_arg() {
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation{});
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn Example as AbiExportable>::ABI_ENTRY, boxed).unwrap() };

    let result = conn.call_closure_with_custom_arg(&|arg|{
        arg.x
    });
    assert_eq!(result, 42);
}
#[test]
fn test_closure_with_custom_arg_call_older() {
    //TODO: Figure out why the following _crashes_!
    //let boxed: Box<dyn Example> = Box::new(ExampleImplementation{});
    //let conn = unsafe { AbiConnection::<dyn ExampleNewer>::from_boxed_trait(<dyn Example as AbiExportable>::ABI_ENTRY, boxed).unwrap() };
    let iface1 : Box<dyn Example> = Box::new(ExampleImplementation {});
    let conn = unsafe { AbiConnection::<dyn ExampleNewer>::from_boxed_trait_for_test(<dyn Example>::ABI_ENTRY, iface1 ) }.unwrap();


    let result = conn.call_closure_with_custom_arg(&|arg|{
        arg.x
    });
    assert_eq!(result, 42);
}
#[test]
#[should_panic(expected="Function arg is not layout-compatible")]
fn test_closure_with_custom_arg_call_older2() {
    //TODO: Figure out why the following _crashes_!
    let boxed: Box<dyn Example> = Box::new(ExampleImplementation{});
    // Here we deliberately use the ABI_ENTRY ExampleNewer, with a trait object for Example.
    // This is not a compatible thing to do, and _should_ panic (but not be unsafe)
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn ExampleNewer as AbiExportable>::ABI_ENTRY, boxed).unwrap() };

    let result = conn.call_closure_with_custom_arg(&|arg|{
        arg.x
    });
    assert_eq!(result, 42);
}