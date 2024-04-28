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
    fn get_str(&self) -> &str;
//~^ 14:34: 14:35: Method 'get': savefile-abi does not support methods returning &str.
}

fn main() {}