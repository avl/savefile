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
use std::borrow::Cow;
#[savefile_abi_exportable(version = 0)]
pub trait CowSmuggler {
    fn smuggle(&mut self, x: Cow<str>) -> Cow<'_, str>;
}
impl CowSmuggler for () {
    fn smuggle(&mut self, x: Cow<str>) -> Cow<'_, str> {
        x
//~^ 18:9: 18:10: lifetime may not live long enough
    }
    // If someone calls smuggle(..) with a reference to a long-lived, but not static item,
    // it is important to understand that the returned Cow<str> cannot have the same lifetime.
    // it may have to be deserialized, and will then be an owned value. It will not be a reference
    // with the same lifetime as the argument.
}
fn main() {}