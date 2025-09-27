use interface_crate::{AdderInterface};
use savefile_derive::savefile_abi_export;

#[derive(Default)]
pub struct MyAdder { }

impl AdderInterface for MyAdder {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

// Export this implementation as the default-implementation for
// the interface 'AdderInterface', for the current library.
savefile_abi_export!(MyAdder, AdderInterface);
