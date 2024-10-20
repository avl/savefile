extern crate rustc_version;
use rustc_version::{version_meta, Version};
fn main() {
    let version = version_meta().unwrap();
    if version.semver >= Version::new(1, 78, 0) {
        println!("cargo:rustc-cfg=feature=\"rust1_78\"");
    }
}
