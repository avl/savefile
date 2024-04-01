use std::io::Cursor;
use savefile_abi::{RawAbiCallResult, AbiSignallingAction, TraitObject, AbiExportableImplementation, abi_entry, AbiConnection, AbiConnectionMethod, AbiExportable, parse_return_value};
use savefile::{Deserialize, Deserializer, SavefileError};
use savefile_derive::savefile_abi_exportable;


#[savefile_abi_exportable(version=0)]
pub trait AdderInterface {
    fn add(&self, x: u32, y: u32) -> u32;
}




pub struct AdderImplementation {
    _name: String
}

impl Default for AdderImplementation {
    fn default() -> Self {
        AdderImplementation {
            _name: "Adderaren Kalle".to_string()
        }
    }
}
impl AdderInterface for AdderImplementation {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}
unsafe impl AbiExportableImplementation for AdderImplementation {
    type AbiInterface = dyn AdderInterface;
    fn new() -> Box<Self::AbiInterface> {
        Box::new(AdderImplementation::default())
    }

}
compile_error!("Figure out if we can maybe implement above trait with macro, and also emit the extern C func!")
#[no_mangle]
pub extern "C" fn test_abi_entry(flag: AbiSignallingAction){
    abi_entry::<AdderImplementation>(flag);
}




fn main() {

    let connection = AbiConnection::<dyn AdderInterface>::new_internal("test", test_abi_entry).unwrap();

    let res = connection.add(1,2);
    assert_eq!(res,3);
    println!("Result: {}", res);
}
