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
include!("AdvancedTestInterface.rs");
include!("TempAdvancedTestInterface_many_callbacks_1_x.rs");
include!("TempTempAdvancedTestInterface_many_callbacks_1_x_docall_1_x0.rs");
include!("TempTempTempAdvancedTestInterface_many_callbacks_1_x_docall_1_x0_docall_1_x0.rs");
*/

#[savefile_abi_exportable(version = 0)]
pub trait AdvancedTestInterface {

    fn return_boxed_closure(&self) -> Box<dyn Fn() -> ()>;
}
//include!("AdvancedTestInterface.rs");
struct AdvancedTestInterfaceImpl{

}
impl AdvancedTestInterface for AdvancedTestInterfaceImpl {
    fn return_boxed_closure(&self) -> Box<dyn Fn() -> ()> {
        Box::new(||{})
    }
}
#[test]
fn test_call_many_callbacks() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let temp = conn.return_boxed_closure();
}
