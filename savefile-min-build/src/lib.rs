extern crate savefile_abi;
extern crate savefile_derive;

use std::collections::HashMap;
use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version = 0)]
pub trait SimpleInterface {
    fn do_call(&self, x: u32) -> u32;
}
#[savefile_abi_exportable(version = 0)]
pub trait AdvancedTestInterface {
    fn return_closure(&self) -> Box<dyn SimpleInterface>;
}
#[test]
pub fn test_generic_example_enum2() {

}
