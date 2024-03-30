use std::sync::atomic::AtomicU64;
use crate::Schema;
pub struct AbiMethodInfo {
    pub return_value: Schema,
    pub arguments: Vec<Schema>,
}

pub struct AbiMethod {
    pub name: &'static str,
    pub info: AbiMethodInfo
}

pub trait AbiExportable {
    fn methods(&self) -> Vec<AbiMethod>;
}

pub struct AbiConnectionMethod {
    pub method_name: &'static str,
    pub caller_info: AbiMethodInfo,
    pub callee_info: Option<AbiMethodInfo>,
    /// For each of the up to 63 different arguments.
    /// A value of all 0:s means the mask has not been
    /// initialized. After initialization, top bit is always set.
    pub compatibility_mask: AtomicU64,
}

/// Information about an ABI-connection. I.e,
/// a caller and callee. The caller is in one
/// particular shared object, the callee in another.
/// Any modifiable state is stored in this object,
/// and the actual callee is stateless (allowing multiple
/// different incoming 'connections').
#[repr(C)]
pub struct AbiConnection {
    pub caller_version: u32,
    /// u32::MAX is a sentinel value
    pub callee_version: u32,
    pub methods: Vec<AbiConnectionMethod>,
    pub entry: fn (connection: *AbiConnection, arguments: *mut (), flag: AbiSignallingAction)
}


#[repr(u8)]
pub enum AbiSignallingAction {
    RegularCall,
    /// Get schema
    Interrogate
}

pub struct AbiEntryPoint {

}

/// This doesn't need no_mangle, since it's not exported
extern fn interrogation_reply_receiver(connection: *mut AbiConnection, method: *const AbiConnectionMethod)
{

}
impl AbiConnection {
    pub fn ensure_init(&self) {
        let mut dummy = ();
        (self.entry)(self as *const _, &mut dummy, AbiSignallingAction::Interrogate)

    }

}

