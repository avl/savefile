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
use savefile_derive::savefile_abi_export;

#[savefile_abi_exportable(version = 0)]
pub trait ExampleTrait {
    fn get(&mut self, x: u32) -> u32;
}
#[derive(Default)]
struct ExampleImpl {

}

// Forgot to implement trait
savefile_abi_export!(ExampleImpl, ExampleTrait);
//~^ 23:22: 23:47: the trait bound `ExampleImpl: ExampleTrait` is not satisfied [E0277]

fn main() {}