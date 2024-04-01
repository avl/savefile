
use std::io::Cursor;
use std::panic::catch_unwind;
use std::ptr::{null, slice_from_raw_parts};
use std::slice;
use std::str::from_utf8;
use std::sync::atomic::AtomicU64;
use byteorder::ReadBytesExt;
use savefile_abi::{RawAbiCallResult, AbiConnection, AbiConnectionMethod, AbiExportable, AbiMethod, AbiMethodInfo, AbiSignallingAction, AbiTraitDefinition, AbiErrorMsg};
use savefile::{CURRENT_SAVEFILE_LIB_VERSION, Deserialize, Deserializer, SavefileError, Schema, SchemaPrimitive, Serialize, Serializer};
use savefile_abi::RawAbiCallResult::AbiError;

pub trait ExampleExternalAbi {
    fn add(&self, x: u32, y: u32) -> u32;
}

impl AbiExportable for dyn ExampleExternalAbi {
    fn get_definition( version: u32) -> AbiTraitDefinition {
        AbiTraitDefinition {
            name: "ExampleExternalAbi".to_string(),
            methods: vec![
                AbiMethod {
                    name: "add".to_string(),
                    info: AbiMethodInfo{
                        return_value: Schema::Primitive(SchemaPrimitive::schema_u32),
                        arguments: vec![
                            Schema::Primitive(SchemaPrimitive::schema_u32),
                            Schema::Primitive(SchemaPrimitive::schema_u32)
                        ]
                    }
                }
            ]
        }
    }

    fn get_latest_version() -> u32 {
        0
    }
}

#[derive(Default)]
pub struct ExternalAbi {
}

impl ExampleExternalAbi for ExternalAbi {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

pub static TEST_ABI: ExternalAbi = ExternalAbi {};



#[no_mangle]
pub extern "C" fn test_abi_entry(flag: AbiSignallingAction){
    match flag {
        AbiSignallingAction::RegularCall { trait_object, compatibility_mask, data, data_length, abi_result, receiver } => {
            match catch_unwind(||{
                let data = unsafe { slice::from_raw_parts(data, data_length) };

                fn parse(trait_object : (*const (), *const ()), compatibility_mask:u64, data: &[u8],  abi_result: *mut (), receiver: extern "C" fn(outcome: *const RawAbiCallResult, result_receiver: *mut ()/*Result<T,SaveFileError>>*/)) -> Result<(),SavefileError> {
                    let trait_object: &dyn ExampleExternalAbi = unsafe { std::mem::transmute(trait_object) };

                    let a;
                    let b;
                    if compatibility_mask & 1 != 0 {
                        // Fast path
                        a = u32::from_le_bytes(data[0..4].try_into().map_err(|_|SavefileError::ShortRead)?);
                    } else {
                        // Serialized
                        todo!()
                    }
                    if compatibility_mask & 2 != 0 {
                        // Fast path
                        b = u32::from_le_bytes(data[4..8].try_into().map_err(|_|SavefileError::ShortRead)?);
                    } else {
                        // Serialized
                        todo!()
                    }
                    let ret = trait_object.add(a,b);
                    if compatibility_mask & (1<<63) != 0 {
                        // Fast path
                        let ret_data = &ret as *const u32 as *const u8;
                        let outcome = RawAbiCallResult::Success {data: ret_data, len: 4};
                        receiver(&outcome as *const _, abi_result)
                    } else {
                        // Serialized
                        todo!()
                    }

                    Ok(())
                }
                match parse(trait_object, compatibility_mask, data, abi_result, receiver) {
                    Ok(_) => {

                    }
                    Err(err) => {
                        let msg = format!("{:?}", err);
                        let err = RawAbiCallResult::AbiError(AbiErrorMsg{error_msg_utf8: msg.as_ptr(), len: msg.len() });
                        receiver(&err, abi_result)
                    }
                }
            }) {
                Ok(()) => {}
                Err(err) => {
                    let msg = format!("{:?}", err);
                    let err = RawAbiCallResult::AbiError(AbiErrorMsg{error_msg_utf8: msg.as_ptr(), len: msg.len() });
                    receiver(&err, abi_result)
                }
            }

        }
        AbiSignallingAction::InterrogateVersion { schema_version_receiver, abi_version_receiver} => {
            /// # SAFETY
            /// The pointers come from another savefile-implementation, and are known to be valid
            unsafe {
                *schema_version_receiver = CURRENT_SAVEFILE_LIB_VERSION;
                *abi_version_receiver = <dyn ExampleExternalAbi as AbiExportable>::get_latest_version();
            }
        }
        AbiSignallingAction::InterrogateMethods { schema_version_required, callee_schema_version_interrogated, result_receiver, callback } => {
            // Note! Any conforming implementation must send a 'schema_version_required' number that is
            // within the ability of the receiving implementation. It can interrogate this using 'AbiSignallingAction::InterrogateVersion'.
            let abi = <dyn ExampleExternalAbi as AbiExportable>::get_definition(callee_schema_version_interrogated);
            let mut temp = vec![];
            let Ok(_) = Serializer::save(&mut temp, schema_version_required as u32, &abi, false) else {
                return;
            };
            callback(result_receiver, schema_version_required, temp.as_ptr(), temp.len());
        }
        AbiSignallingAction::CreateInstance { trait_object_receiver, error_receiver, error_callback } => {
            match catch_unwind(||{
                let obj:Box<dyn ExampleExternalAbi> = Box::new(ExternalAbi::default());
                unsafe { *trait_object_receiver = std::mem::transmute(Box::<_>::into_raw(obj)); }
            }) {
                Ok(_) => {}
                Err(err) => {
                    let msg = format!("{:?}", err);
                    let err = AbiErrorMsg{error_msg_utf8: msg.as_ptr(), len: msg.len() };
                    error_callback(error_receiver, &err as *const _)
                }
            }
        }
        AbiSignallingAction::DropInstance { trait_object } => {
            let trait_object_ptr : *mut dyn ExampleExternalAbi = unsafe { std::mem::transmute(trait_object) };
            _ = unsafe  { Box::from_raw(trait_object_ptr) };
        }
    }

}


fn parse_outcome<T:Deserialize>(outcome: &RawAbiCallResult) -> Result<T, SavefileError> {
    let outcome = unsafe { &*outcome };
    match outcome {
        RawAbiCallResult::Success { data, len } => {
            let data = unsafe { std::slice::from_raw_parts(*data, *len) };
            let mut reader = Cursor::new(data);
            Deserializer::load::<T>(&mut reader, 0)
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

impl ExampleExternalAbi for AbiConnection {
    fn add(&self, x: u32, y: u32) -> u32 {
        let info: &AbiConnectionMethod = &self.methods[0];

        extern "C" fn result_receiver<T:Deserialize>(outcome: *const RawAbiCallResult, result_receiver: *mut ()) {
            /// # SAFETY
            /// The pointers come from a detected savefile-implementation in the other library.
            /// We trust it.
            let outcome = unsafe { &*outcome };
            /// # SAFETY
            /// The pointers come from a detected savefile-implementation in the other library.
            /// We trust it.
            let result_receiver = unsafe { &mut *(result_receiver as *mut Result<T, SavefileError>) };
            *result_receiver = parse_outcome::<T>(outcome)
        }
        let mut result_buffer: Result<u32,SavefileError> = Ok(0);
        if (info.compatibility_mask & 1 == 1) {
            let data = [x,y];
            (self.entry)(AbiSignallingAction::RegularCall {
                trait_object: self.trait_object,
                compatibility_mask: info.compatibility_mask,
                data: data.as_ptr() as *const u8,
                data_length: 8,
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

    let connection = AbiConnection::new_internal::<dyn ExampleExternalAbi>("test", test_abi_entry, 0).unwrap();

    assert_eq!(connection.add(1,2),3);
}

