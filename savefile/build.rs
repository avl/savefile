extern crate rustc_version;
use rustc_version::{version, version_meta, Channel, Version};
fn main() {
    println!("cargo:rustc-check-cfg=cfg(has_new_range)");
    
    // std::range::Range was stabilized in rust 1.96
    if let Ok(version) = version() {
        if version >= Version::new(1, 96, 0) {
            println!("cargo:rustc-cfg=has_new_range");
        }
    }
    
    let version = version_meta().unwrap();
    if version.channel == Channel::Nightly {
        println!("cargo:rustc-cfg=feature=\"nightly\"");
    }
}
