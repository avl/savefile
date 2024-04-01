extern crate savefile;
use std::io::Cursor;
use std::ptr::{null, slice_from_raw_parts};
use std::slice;
use std::sync::atomic::AtomicU64;
use savefile::{CURRENT_SAVEFILE_LIB_VERSION, Deserialize, Deserializer, diff_schema, new_schema_deserializer, SavefileError, Schema};
use savefile::prelude::Savefile;
use savefile::SavefileError::MissingMethod;

#[derive(Savefile)]
pub struct AbiMethodInfo {
    pub return_value: Schema,
    pub arguments: Vec<Schema>,
}

#[derive(Savefile)]
pub struct AbiMethod {
    pub name: String,
    pub info: AbiMethodInfo
}

/// Defines a dyn trait, basically
#[derive(Savefile, Default)]
pub struct AbiTraitDefinition {
    pub name: String,
    pub methods: Vec<AbiMethod>
}

pub trait AbiExportable {
    fn get_definition(version: u32) -> AbiTraitDefinition;
    fn get_latest_version() -> u32;
}

pub struct AbiConnectionMethod {
    pub method_name: String,
    pub caller_info: AbiMethodInfo,
    pub callee_info: Option<AbiMethodInfo>,
    /// For each of the up to 64 different arguments,
    /// a bit value of 1 means layout is identical
    pub compatibility_mask: u64,
}

/// Information about an ABI-connection. I.e,
/// a caller and callee. The caller is in one
/// particular shared object, the callee in another.
/// Any modifiable state is stored in this object,
/// and the actual callee is stateless (allowing multiple
/// different incoming 'connections').
#[repr(C)]
pub struct AbiConnection {
    pub effective_version: u32,
    pub methods: Vec<AbiConnectionMethod>,
    pub entry: extern "C" fn (flag: AbiSignallingAction),
    pub trait_object: (*const (), *const ()),
}

#[repr(C)]
pub struct AbiErrorMsg {
    pub error_msg_utf8: *const u8,
    pub len: usize
}
impl AbiErrorMsg {
    pub fn to_string(&self) -> String {
        if self.len == 0 {
            return "".to_string();
        }
        let data = unsafe { slice::from_raw_parts(self.error_msg_utf8, self.len) };
        String::from_utf8_lossy(data).into()
    }
}
#[repr(C,u8)]
pub enum RawAbiCallResult {
    /// Successful operation
    Success{data: *const u8, len: usize},
    /// The method that was called, panicked
    Panic(AbiErrorMsg),
    /// There was an error in the ABI-framework
    AbiError(AbiErrorMsg)
}


#[repr(C, u8)]
pub enum AbiSignallingAction {
    RegularCall {
        trait_object: (*const (), *const ()),
        /// Mask determining which parameters are serialized
        /// A bit value of '1' means memory-layout is compatible,
        /// a value of '0' means argument must be serialized.
        ///
        /// NOTE! The most significant bit corresponds to the return value.
        compatibility_mask: u64,
        /// Data for parameters, possibly serialized
        data: *const u8,
        /// Length of parameters-data
        data_length: usize,
        /// Instance of type AbiCallResult<T>, to which the return-value callback will
        /// write deserialized results or panic-message.
        abi_result: *mut (),
        /// Callback which will be called by callee in order to supply the return value
        /// (without having to allocate heap-memory)
        receiver: extern "C" fn(outcome: *const RawAbiCallResult, result_receiver: *mut ()/*Result<T,SaveFileError>>*/)
    },
    /// Get callee version
    InterrogateVersion {
        schema_version_receiver: *mut u16,
        abi_version_receiver: *mut u32,
    },
    /// Get schema
    InterrogateMethods {
        schema_version_required: u16,
        callee_schema_version_interrogated: u32,
        result_receiver: *mut AbiTraitDefinition,
        callback: extern "C" fn (receiver: *mut AbiTraitDefinition, callee_schema_version: u16, data: *const u8, len: usize)
    },
    CreateInstance {
        /// Pointer which will receive the fat pointer to the dyn trait object, allocated on heap using Box.
        trait_object_receiver: *mut (*const (), *const ()),
        /// Opaque pointer to callers representation of error (String)
        error_receiver: *mut ()/*String*/,
        /// Called by callee if instance creation fails (by panic)
        error_callback: extern "C" fn (error_receiver: *mut (), error: *const AbiErrorMsg)
    },
    DropInstance {
        /// dyn trait fat pointer
        trait_object: (*const (), *const ()),
    },
}

pub struct AbiEntryPoint {

}

impl AbiConnection {

    fn analyze_and_create(
        trait_name: &str,
        trait_object: (*const (), *const ()),
        remote_entry: extern "C" fn (flag: AbiSignallingAction),
        effective_version: u32,
        caller_effective_definition: AbiTraitDefinition,
        callee_effective_definition: AbiTraitDefinition,
        caller_native_definition: AbiTraitDefinition,
        callee_native_definition: AbiTraitDefinition
    ) -> Result<AbiConnection, SavefileError> {

        let mut methods = Vec::with_capacity(caller_native_definition.methods.len());
        for caller_native_method in caller_native_definition.methods.into_iter() {
            let Some(callee_native_method) = callee_native_definition.methods.iter().find(|x|x.name == caller_native_method.name) else {
                return Err(MissingMethod {method_name: caller_native_method.name.to_string()});
            };

            let Some(callee_effective_method) = callee_effective_definition.methods.iter().find(|x|x.name == caller_native_method.name) else {
                return Err(SavefileError::GeneralError {msg: format!("Internal error - missing method definition {} in signature when calculating serializable version of call (1).", caller_native_method.name)});
            };

            let Some(caller_effective_method) = caller_effective_definition.methods.iter().find(|x|x.name == caller_native_method.name) else {
                return Err(SavefileError::GeneralError {msg: format!("Internal error - missing method definition {} in signature when calculating serializable version of call (2).", caller_native_method.name)});
            };

            if caller_native_method.info.arguments.len() != callee_native_method.info.arguments.len() {
                return Err(SavefileError::GeneralError {msg: format!("Number of arguments for method {} has changed from {} to {}.", caller_native_method.name, caller_native_method.info.arguments.len(), callee_native_method.info.arguments.len())});
            }

            if caller_native_method.info.arguments.len() != caller_effective_method.info.arguments.len() {
                return Err(SavefileError::GeneralError {msg: format!("Internal error - number of arguments for method {} has differs between {} to {} (1).", caller_native_method.name, caller_native_method.info.arguments.len(), caller_effective_method.info.arguments.len())});
            }

            if caller_native_method.info.arguments.len() != callee_effective_method.info.arguments.len() {
                return Err(SavefileError::GeneralError {msg: format!("Internal error - number of arguments for method {} has differs between {} to {} (2).", caller_native_method.name, caller_native_method.info.arguments.len(), callee_effective_method.info.arguments.len())});
            }

            if caller_native_method.info.arguments.len() > 63 {
                return Err(SavefileError::TooManyArguments);
            }

            let retval_effective_schema_diff = diff_schema(
                &caller_effective_method.info.return_value,
                &callee_effective_method.info.return_value,"".to_string());
            if let Some(diff) = retval_effective_schema_diff {
                return Err(SavefileError::IncompatibleSchema{
                    message: format!("Incompatible ABI detected. Trait: {}, method: {}, return value error: {}",
                                     trait_name, &caller_native_method.name, diff
                    )
                });
            }

            let mut mask = 0;
            for index in 0..caller_native_method.info.arguments.len()
            {
                let effective_schema_diff = diff_schema(
                    &caller_effective_method.info.arguments[index],
                    &callee_effective_method.info.arguments[index],"".to_string());
                if let Some(diff) = effective_schema_diff {
                    return Err(SavefileError::IncompatibleSchema{
                        message: format!("Incompatible ABI detected. Trait: {}, method: {}, argument: #{}, error: {}",
                            trait_name, &caller_native_method.name, index, diff
                        )
                    });
                }

                if caller_native_method.info.arguments[index].layout_compatible(&callee_native_method.info.arguments[index]) {
                    mask |= 1<<index;
                }
            }
            if caller_native_method.info.return_value.layout_compatible(&callee_native_method.info.return_value) {
                mask |= 1<<63;
            }

            methods.push(AbiConnectionMethod{
                method_name: caller_native_method.name,
                caller_info: caller_native_method.info,
                callee_info: None,
                compatibility_mask: mask,
            })
        }

        Ok(AbiConnection{
            trait_object,
            effective_version,
            methods,
            entry: remote_entry,
        })
    }

    pub fn new_internal<T:AbiExportable+?Sized>(
        trait_name: &str,
        remote_entry: extern "C" fn (flag: AbiSignallingAction),
        own_version: u32) -> Result<AbiConnection, SavefileError>
    {

        let own_native_definition = T::get_definition(own_version);

        let mut callee_abi_version = 0u32;
        let mut callee_schema_version = 0u16;
        (remote_entry)(AbiSignallingAction::InterrogateVersion{
            schema_version_receiver: &mut callee_schema_version as *mut _,
            abi_version_receiver: &mut callee_abi_version as *mut _,
        });

        if callee_schema_version > CURRENT_SAVEFILE_LIB_VERSION {
            return Err(SavefileError::IncompatibleSavefileLibraryVersion);
        }

        let effective_version = own_version.min(callee_abi_version);

        let mut callee_abi_native_definition = AbiTraitDefinition {
            name: "".to_string(),
            methods: vec![],
        };
        let mut callee_abi_effective_definition = AbiTraitDefinition {
            name: "".to_string(),
            methods: vec![],
        };
        extern "C" fn definition_receiver(receiver: *mut AbiTraitDefinition, schema_version: u16, data: *const u8, len: usize) {
            let receiver = unsafe { &mut *receiver };
            let slice = unsafe { slice::from_raw_parts(data, len) };
            let mut cursor = Cursor::new(slice);

            let mut schema_deserializer = new_schema_deserializer(&mut cursor, schema_version);
            *receiver = AbiTraitDefinition::deserialize(&mut schema_deserializer).unwrap_or(Default::default());
        }


        (remote_entry)(AbiSignallingAction::InterrogateMethods {
            schema_version_required: callee_schema_version,
            callee_schema_version_interrogated: callee_abi_version,
            result_receiver: &mut callee_abi_native_definition as *mut _,
            callback: definition_receiver
        });


        let own_effective_definition = T::get_definition(effective_version);

        let mut trait_object = (null(), null());
        let mut error_msg:String = Default::default();
        extern "C" fn error_callback(error_receiver: *mut (), error: *const AbiErrorMsg) {
            let error_msg = unsafe{&mut *(error_receiver as *mut String)};
            *error_msg = unsafe{&*error}.to_string();
        }
        (remote_entry)(AbiSignallingAction::CreateInstance{
            trait_object_receiver: &mut trait_object as *mut _,
            error_receiver: &mut error_msg as *mut String as *mut _,
            error_callback,
        });
        if error_msg.len() > 0 {
            return Err(SavefileError::CalleePanic {msg: error_msg});
        }

        Self::analyze_and_create(
            trait_name,
            trait_object,
            remote_entry,
            effective_version,
            own_effective_definition,
            callee_abi_effective_definition,
            own_native_definition,
            callee_abi_native_definition,
        )
    }

}

