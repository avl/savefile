extern crate savefile_abi;
extern crate savefile_derive;

use std::collections::HashMap;
use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
use savefile_abi::AbiConnection;
use savefile_derive::savefile_abi_exportable;





/*

include!("__0_owning_.rs");
include!("__1_.rs");
include!("__2_.rs");
include!("__3_.rs");
include!("AdvancedTestInterface.rs");
*/

/*
#[savefile_abi_exportable(version = 0)]
pub trait AdvancedTestInterface {
    fn count_chars_str(&self, x: &str) -> usize;
}*/

#[test]
fn test_call_many_callbacks() {
  /*  let boxed: Box<dyn CallbackInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let temp = conn.get();
    assert_eq!(temp, 42);*/
}
