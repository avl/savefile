extern crate rustc_version;
use rustc_version::{version_meta, Channel, Version};
fn main() {
    let version = version_meta().unwrap();
    if version.channel == Channel::Nightly {
        println!("cargo:rustc-cfg=feature=\"nightly\"");
    }
    if version.semver >= Version::new(1, 78, 0) {
        println!("cargo:rustc-cfg=feature=\"rust1_78\"");
    }
}
