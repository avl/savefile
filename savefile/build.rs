extern crate rustc_version;
use rustc_version::{version_meta, Channel};
fn main() {
    let version = version_meta().unwrap();
    if version.channel == Channel::Nightly {
        println!("cargo:rustc-cfg=feature=\"nightly\"");
    }
}
