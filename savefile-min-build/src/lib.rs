extern crate savefile_abi;
extern crate savefile_derive;

use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version = 0)]
pub trait TestInterface {
    fn count_chars_str(&self, x: &str) -> usize;
}
#[test]
pub fn test_generic_example_enum2() {

}
