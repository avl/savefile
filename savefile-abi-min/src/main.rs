use std::ptr::null;
use std::sync::atomic::AtomicU64;
use savefile_abi::{AbiCallResult, AbiConnection, AbiConnectionMethod, AbiErrMsg, AbiExportable, AbiMethod, AbiMethodInfo, AbiSignallingAction, Method};
use savefile::{Schema, SchemaPrimitive};

pub trait ExampleExternalAbi {
    fn add(&self, x: u32, y: u32) -> u32;
}

impl AbiExportable for dyn ExampleExternalAbi {
    fn methods(&self) -> Vec<AbiMethod> {
        vec![
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

pub struct ExternalAbi {
}

impl ExampleExternalAbi for ExternalAbi {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

pub static TEST_ABI: ExternalAbi = ExternalAbi {};

#[no_mangle]
pub extern fn test_abi_entry(flag: AbiSignallingAction){
    todo!()

}




impl ExampleExternalAbi for AbiConnection {
    fn add(&self, x: u32, y: u32) -> u32 {
        let info: &AbiConnectionMethod = &self.methods[0];

        compile_error!("Test this, then try to minimize how much code we need to generate per method")
        extern "C" fn result_receiver(outcome: AbiCallResult, err_receiver: *mut AbiErrMsg, result_receiver: *mut (), retval_data: *const u8, retval_length: usize) {
            if retval_length != 4 {
                return;
            }
            let retval_val = unsafe { &*(retval_data as *const u32) };
            let result_receiver_val = unsafe { &mut *(result_receiver as *mut u32) };
            *result_receiver_val = *retval_val
        }
        let mut result_buffer: u32 = 0;
        let mut err_receiver = AbiErrMsg{
            msg: null(),
            len: 0,
        };
        if (info.compatibility_mask & 1 == 1) {
            let data = [x,y];
            (self.entry)(AbiSignallingAction::RegularCall(data.as_ptr() as *const u32 as * const u8, 8, &mut err_receiver as *mut AbiErrMsg, &mut result_buffer as *mut u32 as *mut (), result_receiver ));
        } else {

        }


        todo!()
    }
}


fn main() {

    let connection = AbiConnection::new_internal("test",test_abi_entry, 0,
    &ExternalAbi{}
    );

    assert_eq!(connection.add(1,2),3);
}

