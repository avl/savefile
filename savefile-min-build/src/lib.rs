
/*
#[derive(Debug, Savefile, PartialEq)]
pub enum TestStructEnum {
    Variant2 { a: u8, b: u8 },
}

#[test]
fn test() {}
*/

use savefile_derive::savefile_abi_exportable;
use savefile_derive::Savefile;
use std::future::Future;
include!{"AdderInterface.rs"}

#[test]
fn test() {

}