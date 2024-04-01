use std::io::Cursor;
use std::mem::MaybeUninit;
use savefile_abi::{RawAbiCallResult, AbiSignallingAction, TraitObject, AbiExportableImplementation, abi_entry, AbiErrorMsg, AbiConnection, AbiConnectionMethod, AbiExportable};
use savefile::{Deserialize, Deserializer, SavefileError};
use savefile_derive::savefile_abi_exportable;


#[savefile_abi_exportable(version=0)]
pub trait AdderInterface {
    fn add(&self, x: u32, y: u32) -> u32;
}

unsafe impl AbiExportableImplementation for AdderImplementation {
    type AbiInterface = dyn AdderInterface;
    fn new() -> TraitObject where AdderImplementation:Default {
        let obj : Box<dyn AdderInterface> = Box::new(AdderImplementation::default());
        unsafe {std::mem::transmute(Box::into_raw(obj))}
    }

    fn destroy(obj: TraitObject) {
        let raw_ptr : *mut dyn AdderInterface = unsafe {std::mem::transmute(obj)};
        let _ = unsafe { Box::from_raw(raw_ptr) };
    }
    fn call(trait_object : TraitObject, method_number: u16, compatibility_mask:u64, data: &[u8],  abi_result: *mut (), receiver: extern "C" fn(outcome: *const RawAbiCallResult, result_receiver: *mut ()/*Result<T,SaveFileError>>*/)) -> Result<(),SavefileError> {
        let trait_object: &Self::AbiInterface = unsafe { std::mem::transmute(trait_object) };

        <dyn AdderInterface as AbiExportable>::call(trait_object, method_number, compatibility_mask, data, abi_result, receiver)
    }

}


#[derive(Default)]
pub struct AdderImplementation {
}

impl AdderInterface for AdderImplementation {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

#[no_mangle]
pub extern "C" fn test_abi_entry(flag: AbiSignallingAction){
    abi_entry::<AdderImplementation>(flag);
}


fn parse_outcome<T:Deserialize>(outcome: &RawAbiCallResult) -> Result<T, SavefileError> {
    match outcome {
        RawAbiCallResult::Success { data, len, serialized } => {
            if *serialized {
                let data = unsafe { std::slice::from_raw_parts(*data, *len) };
                let mut reader = Cursor::new(data);
                Deserializer::bare_deserialize::<T>(&mut reader, 0, 0)
            } else {
                let mut result : MaybeUninit<T> = MaybeUninit::uninit();
                let result_ptr = result.as_mut_ptr() as *mut u8;
                unsafe  { std::ptr::copy(*data, result_ptr, *len) };
                Ok(unsafe { result.assume_init() })
            }
        }
        RawAbiCallResult::Panic(AbiErrorMsg{ error_msg_utf8, len }) => {
            let errdata = unsafe { std::slice::from_raw_parts(*error_msg_utf8, *len) };
            Err(SavefileError::CalleePanic {
                msg:String::from_utf8_lossy(errdata).into()
            })
        }
        RawAbiCallResult::AbiError(AbiErrorMsg{error_msg_utf8, len }) => {
            let errdata = unsafe { std::slice::from_raw_parts(*error_msg_utf8, *len) };
            Err(SavefileError::GeneralError {
                msg:String::from_utf8_lossy(errdata).into()
            })
        }
    }
}

impl AdderInterface for AbiConnection<dyn AdderInterface> {
    fn add(&self, x: u32, y: u32) -> u32 {
        let info: &AbiConnectionMethod = &self.methods[0];

        extern "C" fn result_receiver<T:Deserialize>(outcome: *const RawAbiCallResult, result_receiver: *mut ()) {
            // # SAFETY
            // The pointers come from a detected savefile-implementation in the other library.
            // We trust it.
            let outcome = unsafe { &*outcome };
            // # SAFETY
            // The pointers come from a detected savefile-implementation in the other library.
            // We trust it.
            let result_receiver = unsafe { &mut *(result_receiver as *mut Result<T, SavefileError>) };
            *result_receiver = parse_outcome::<T>(outcome)
        }
        let mut result_buffer: Result<u32,SavefileError> = Ok(0);
        if info.compatibility_mask & 3 == 3 {
            let data = [0/*version*/, x,y];
            (self.entry)(AbiSignallingAction::RegularCall {
                trait_object: self.trait_object,
                method_number: info.callee_method_number,
                compatibility_mask: info.compatibility_mask,
                data: data.as_ptr() as *const u8,
                data_length: 12,
                abi_result: &mut result_buffer as *mut Result<u32,SavefileError> as *mut (),
                receiver: result_receiver::<u32>,
            });
        } else {
            todo!("Serialized case not yet implemented")
        }
        result_buffer.expect("Unexpected panic in invocation target")
    }
}


fn main() {

    let connection = AbiConnection::<dyn AdderInterface>::new_internal("test", test_abi_entry).unwrap();

    let res = connection.add(1,2);
    assert_eq!(res,3);
}
