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
    fn example_func(&self, x: &&u32);
//~^ 14:32: 14:33: Method example_func, argument x: Method arguments cannot be reference to reference in savefile-abi. Try removing a '&' from the type: & u32
}

fn main() {}
