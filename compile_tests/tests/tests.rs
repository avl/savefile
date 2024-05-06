extern crate compiletest_rs as compiletest;

use std::path::PathBuf;

fn run_mode(mode: &'static str, custom_dir: Option<&'static str>) {
    let mut config = compiletest::Config::default().tempdir();
    let cfg_mode = mode.parse().expect("Invalid mode");

    config.mode = cfg_mode;

    let dir = custom_dir.unwrap_or(mode);
    config.src_base = PathBuf::from(format!("tests/{}", dir));
    config.target_rustcflags = Some("-L target/debug -L target/debug/deps --edition 2021".to_string());
    config.llvm_filecheck = Some("FileCheck".to_string().into());

    config.strict_headers = true;

    compiletest::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("compile-fail", None);
    run_mode("run-pass", None);
}
