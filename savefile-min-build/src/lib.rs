extern crate savefile_abi;
extern crate savefile_derive;

use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version = 0)]
pub trait ExampleTrait {
    fn set(&mut self, x: Box<dyn Fn()>) -> u32;
}

#[test]
fn dummy_test() {}
