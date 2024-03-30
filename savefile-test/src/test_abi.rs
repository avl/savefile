/*
pub trait ExampleExternalAbi {
    fn add(x: u32, y: u32) -> u32;
}

pub struct ExternalAbi {
}

impl ExampleExternalAbi for ExternalAbi {
    fn add(x: u32, y: u32) -> u32 {
        x + y
    }
}

#[Derive(savefile_abi_export)]
pub static TEST_ABI: ExternalAbi = ExternalAbi {};

*/