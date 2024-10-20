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
    fn set(&mut self, x: u32) -> impl Future<Output = ()>;
//~^ 14:39: 14:45: In return value of method 'set', impl Future is not supported by savefile-abi. You can try using Pin<Box<Future < Output = () >>> instead.
}

fn main() {}