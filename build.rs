use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=crates/runtime-core/Cargo.toml");
    println!("cargo:rerun-if-changed=crates/runtime-core/src");
    println!("cargo:rerun-if-changed=src/html/runtime");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let repo_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"));
    let outer_target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repo_root.join(path)
            }
        })
        .unwrap_or_else(|| repo_root.join("target"));
    let wasm_target_dir = outer_target_dir.join("runtime-assets");

    gsp_runtime_assets::build_runtime_assets(&repo_root, &output_dir, &wasm_target_dir)
        .unwrap_or_else(|error| panic!("failed to build embedded runtime assets: {error}"));
}
