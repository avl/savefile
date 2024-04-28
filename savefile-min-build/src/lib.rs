extern crate savefile_abi;
extern crate savefile_derive;

use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version = 0)]
pub trait ExampleTrait {
    fn get(&mut self) -> &'static str;
}

#[test]
fn dummy_test() {}
