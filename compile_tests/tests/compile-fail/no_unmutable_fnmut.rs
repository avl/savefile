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
    fn example_func(&self, x: &dyn FnMut());
//~^ 14:36: 14:41: Method example_func, argument x: When using FnMut, it must be referenced using &mut, not &. Otherwise, it is impossible to call.
}

fn main() {}
