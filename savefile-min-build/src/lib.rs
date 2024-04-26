extern crate savefile_abi;
extern crate savefile_derive;

use std::collections::HashMap;
use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};
use savefile_abi::AbiConnection;
use savefile_derive::savefile_abi_exportable;




include!("TempAdvancedTestInterface_return_boxed_closure_2____retval.rs");
include!("TempAdvancedTestInterface_return_boxed_closure_1_returnvalue.rs");
include!("AdvancedTestInterface.rs");

//#[savefile_abi_exportable(version = 0)] pub trait AdvancedTestInterface { fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32>; }

struct AdvancedTestInterfaceImpl{

}
impl AdvancedTestInterface for AdvancedTestInterfaceImpl {
    fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32> {
        Box::new(||{42})
    }
}
#[test]
fn test_call_many_callbacks() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let temp = conn.return_boxed_closure();
    assert_eq!((temp)(), 42);
}
