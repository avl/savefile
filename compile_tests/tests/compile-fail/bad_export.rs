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
struct ExampleImpl {

}
impl ExampleTrait for ExampleImpl {
    fn get(&mut self, x: u32) -> u32 {
        x
    }
}
// Test what happens when you mix up the ordering of trait and impl:
savefile_abi_export!(ExampleTrait, ExampleImpl);
//~^ 26:1: 26:48: expected trait, found struct `ExampleImpl` [E0404]

fn main() {}