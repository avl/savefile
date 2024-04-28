extern crate savefile;
extern crate savefile_abi;
extern crate savefile_derive;
use std::collections::HashMap;
use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
use savefile_abi::AbiConnection;
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version = 0)]
pub trait ExampleTrait {
    fn example_func(&self, x: &Box<u32>);
//~^ 14:32: 14:35: Savefile does not support reference to Box. This is also generally not very useful, just use a regular reference for arguments.
}

fn main() {}
