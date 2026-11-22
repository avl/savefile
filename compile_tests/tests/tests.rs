extern crate compiletest_rs as compiletest;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Crates that the test programs reference with `extern crate`.
const EXTERN_CRATES: &[&str] = &["savefile", "savefile_abi", "savefile_derive", "async_trait"];

/// Build the dependencies (normally a no-op, since `cargo test` already did) and ask cargo
/// for the exact artifact paths.
///
/// This avoids hard-coding `target/debug/deps`, which newer cargo no longer uses (the new
/// build-dir layout puts artifacts in `target/debug/build/<pkg>/<hash>/out`). It also avoids
/// picking up stale artifacts built by other rustc versions (E0514), or ambiguity between
/// several builds of the same crate (E0464).
fn rustc_flags() -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new(cargo)
        .args(["build", "--message-format=json", "--manifest-path"])
        .arg(&manifest)
        .output()
        .expect("failed to run cargo build");
    assert!(
        output.status.success(),
        "cargo build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut lib_dirs = BTreeSet::new();
    let mut externs = vec![];
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg["reason"] != "compiler-artifact" {
            continue;
        }
        let name = msg["target"]["name"].as_str().unwrap_or_default().replace('-', "_");
        for file in msg["filenames"].as_array().into_iter().flatten() {
            let Some(file) = file.as_str() else { continue };
            let path = PathBuf::from(file);
            let is_lib = ["rlib", "rmeta", "so", "dylib", "dll"]
                .iter()
                .any(|ext| path.extension().map(|x| x == *ext).unwrap_or(false));
            if !is_lib {
                continue;
            }
            if let Some(dir) = path.parent() {
                lib_dirs.insert(dir.to_path_buf());
            }
            // Newer cargo builds with `-Zembed-metadata=no`, in which case the rlib is only a
            // stub and the sibling .rmeta must also be passed. So pass all files.
            if EXTERN_CRATES.contains(&name.as_str()) {
                externs.push((name.clone(), path));
            }
        }
    }

    let mut flags = String::new();
    for dir in &lib_dirs {
        flags += &format!(" -L dependency={}", dir.display());
    }
    for crate_name in EXTERN_CRATES {
        assert!(
            externs.iter().any(|(name, _)| name == crate_name),
            "Could not find build artifact for crate {}",
            crate_name
        );
    }
    for (name, path) in &externs {
        flags += &format!(" --extern {}={}", name, path.display());
    }
    flags += " --edition 2021";
    flags
}

fn run_mode(mode: &'static str, custom_dir: Option<&'static str>, rustc_flags: &str) {
    let mut config = compiletest::Config::default().tempdir();
    let cfg_mode = mode.parse().expect("Invalid mode");

    config.mode = cfg_mode;

    let dir = custom_dir.unwrap_or(mode);
    config.src_base = PathBuf::from(format!("tests/{}", dir));
    config.target_rustcflags = Some(rustc_flags.to_string());
    config.llvm_filecheck = Some("FileCheck".to_string().into());

    config.strict_headers = true;

    compiletest::run_tests(&config);
}

#[test]
fn compile_test() {
    let flags = rustc_flags();
    run_mode("compile-fail", None, &flags);
    run_mode("run-pass", None, &flags);
}
