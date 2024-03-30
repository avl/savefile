use savefile::abi::{AbiConnection, AbiConnectionMethod, AbiExportable, AbiMethod, AbiMethodInfo, Method};
use savefile::{Schema, SchemaPrimitive};

pub trait ExampleExternalAbi {
    fn add(&self, x: u32, y: u32) -> u32;
}

impl AbiExportable for dyn ExampleExternalAbi {
    fn methods(&self) -> Vec<AbiMethod> {
        vec![
            AbiMethod {
                name: "add",
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
pub extern fn test_abi_entry(connection: *mut AbiConnection, arguments: *mut (), flag: u8){

}


impl ExampleExternalAbi for AbiConnection {
    fn add(&self, x: u32, y: u32) -> u32 {
        self.ensure_init();
    }
}


fn main() {

    let connection = AbiConnection {
        caller_version: 0,
        callee_version: 0,
        methods: vec![
            AbiConnectionMethod {
                method_name: "add",
                caller_info: AbiMethodInfo { return_value: Schema::u32, arguments: vec![Schema::Primitive(SchemaPrimitive::schema_u32), Schema::Primitive(SchemaPrimitive::schema_u32)] },
                callee_info: None,
                compatibility_mask: None,
            }
        ],
        entry: test_abi_entry,
    };

    //check(Default::default());
    println!("Hello, world!");
}
