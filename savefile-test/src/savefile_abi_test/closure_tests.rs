#![allow(non_camel_case_types)]
use savefile_abi::{AbiConnection, AbiExportable};
use savefile_abi_test::basic_abi_tests::{CallbackImpl, TestInterface, TestInterfaceImpl};


#[savefile_abi_exportable(version=0)]
pub trait Example {
    fn call_mut_closure(&self, simple: &mut dyn FnMut(u32, &u32) -> u32);
    fn call_closure(&self, simple: &dyn Fn(u32, &u32) -> u32);
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

