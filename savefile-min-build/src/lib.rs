extern crate savefile_abi;
extern crate savefile_derive;

use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};

#[derive(Debug, Savefile, PartialEq)]
pub enum TestTupleEnum {
    Variant1(u8),
}
#[test]
pub fn test_generic_example_enum2() {}
