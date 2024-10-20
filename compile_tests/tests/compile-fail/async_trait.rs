extern crate savefile;
extern crate savefile_abi;
extern crate savefile_derive;
extern crate async_trait;
use std::collections::HashMap;
use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
use savefile_abi::AbiConnection;
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version = 0)]
#[async_trait]
//~^ 14:3: 14:14: async_trait-attribute macro detected. The #[async_trait] macro must go _before_ the #[savefile_abi_exportable(..)] macro!
pub trait ExampleTrait {
    async fn set(&mut self, x: u32) -> u32;
}

fn main() {}