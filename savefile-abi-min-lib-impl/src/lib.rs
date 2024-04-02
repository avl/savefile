use savefile_abi_min_lib::AdderInterface;
use savefile_derive::savefile_abi_export;

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
savefile_abi_export!(AdderImplementation, AdderInterface);


