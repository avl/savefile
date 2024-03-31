use core::str::from_raw_parts;
use std::io::Cursor;
use std::ptr::slice_from_raw_parts;
use std::sync::atomic::AtomicU64;
use savefile::{CURRENT_SAVEFILE_LIB_VERSION, Deserialize, Deserializer, diff_schema, SavefileError, Schema};
use savefile::prelude::Savefile;
use savefile::SavefileError::MissingMethod;

#[derive(Savefile, Clone)]
pub struct AbiMethodInfo {
    pub return_value: Schema,
    pub arguments: Vec<Schema>,
}

#[derive(Savefile, Clone)]
pub struct AbiMethod {
    pub name: String,
    pub info: AbiMethodInfo
}

/// Defines a dyn trait, basically
#[derive(Savefile, Clone)]
pub struct AbiTraitDefinition {
    pub name: String,
    pub methods: Vec<AbiMethod>
}

pub trait AbiExportable {
    fn get_definition(&self, version: u32) -> AbiTraitDefinition;
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
    pub entry: fn (flag: AbiSignallingAction)
}


#[repr(C)]
pub struct AbiErrMsg {
    pub msg: *const u8,
    pub len: usize,
}

#[repr(u8)]
pub enum AbiCallResult {
    Success,
    Panic
}

#[repr(C, u8)]
pub enum AbiSignallingAction {
    RegularCall(*const u8/*data*/, usize/*length*/, *mut AbiErrMsg, *const () /*result_receiver*/, extern "C" fn (outcome: AbiCallResult, error_receiver: *mut AbiErrMsg, result_receiver: *mut (), retval_data: *const u8, retval_length: usize)),
    /// Get callee version
    InterrogateVersion(/*abi version receiver*/ *mut u16, /*schema version receiver */ *mut u32, extern "C" fn (schema_version_receiver: *mut u16, abi_receiver: *mut u32, callee_schema_version: u16, callee_abi_version: u32)),
    /// Get schema
    InterrogateMethods(/*schema version required*/ u16, /*abi version to interrogate*/u32, /*receiver*/ *mut AbiTraitDefinition, extern "C" fn (receiver: *mut AbiTraitDefinition, schema_version: u16, data: *const u8, len: usize)),
}

pub struct AbiEntryPoint {

}

impl AbiConnection {

    fn analyze_and_create(
        trait_name: &str,
        remote_entry: fn (flag: AbiSignallingAction),
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

            if caller_native_method.info.arguments.len() > 64 {
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

            methods.push(AbiConnectionMethod{
                method_name: caller_native_method.name,
                caller_info: caller_native_method.info,
                callee_info: None,
                compatibility_mask: mask,
            })
        }


        Ok(AbiConnection{
            effective_version,
            methods,
            entry: remote_entry,
        })
    }

    pub fn new_internal(
        trait_name: &str,
        remote_entry: fn (flag: AbiSignallingAction),
        own_version: u32,
        own_abi_definition: &dyn AbiExportable) -> Result<AbiConnection, SavefileError> {

        let own_native_definition = own_abi_definition.get_definition(own_version);

        let mut callee_abi_version = 0u32;
        let mut callee_schema_version = 0u16;
        extern "C" fn version_receiver(schema_version_receiver: *mut u16, abi_version_receiver: *mut u32, callee_schema_version: u16, callee_abi_version: u32) {
            unsafe { *schema_version_receiver } = callee_schema_version;
            unsafe { *abi_version_receiver } = callee_abi_version;
        }
        (remote_entry)(AbiSignallingAction::InterrogateVersion(&mut callee_schema_version as *mut _, &mut callee_abi_version as *mut _, version_receiver));

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
            let slice = unsafe { slice_from_raw_parts(data, len) };
            let mut cursor = Cursor::new(slice);

            let mut schema_deserializer = Deserializer::new_schema_deserializer(&mut cursor, schema_version);
            let callee_schema = AbiTraitDefinition::deserialize(&mut schema_deserializer)?;
            *receiver = callee_schema;
        }
        (remote_entry)(AbiSignallingAction::InterrogateMethods(callee_schema_version, callee_abi_version, &mut callee_abi_native_definition as *mut _, definition_receiver));

        (remote_entry)(AbiSignallingAction::InterrogateMethods(callee_schema_version, effective_version, &mut callee_abi_effective_definition as *mut _, definition_receiver));

        let own_effective_definition = own_abi_definition.get_definition(effective_version);


        Self::analyze_and_create(
            trait_name,
            remote_entry,
            effective_version,
            own_effective_definition,
            callee_abi_effective_definition,
            own_native_definition,
            callee_abi_native_definition,
        )
    }

}

