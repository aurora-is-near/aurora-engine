use std::path::{Path, PathBuf};
use std::process::Command;

pub fn compile<P: AsRef<Path>>(manifest_path: P) -> PathBuf {
    build_contract_1_93(&manifest_path.as_ref().join("Cargo.toml"), None)
}

/// Build a NEAR helper contract with the workspace toolchain (rust 1.93) for
/// wasm32-unknown-unknown, then lower bulk-memory opcodes (emitted by rustc>=1.87)
/// to MVP via wasm-opt --llvm-memory-copy-fill-lowering, so near-vm singlepass
/// (`reftypes_bulk_memory`=false) can deserialize the wasm.
pub fn build_contract_1_93(manifest_path: &PathBuf, features: Option<&str>) -> PathBuf {
    let dir = std::fs::canonicalize(manifest_path.parent().unwrap())
        .unwrap_or_else(|e| panic!("cannot canonicalize {manifest_path:?}: {e}"));

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&dir)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("RUSTFLAGS", "-C link-arg=-s")
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"]);
    if let Some(f) = features {
        cmd.args(["--features", f]);
    }
    assert!(
        cmd.status().expect("spawn cargo").success(),
        "build failed: {manifest_path:?}"
    );

    let release_dir = dir.join("target/wasm32-unknown-unknown/release");
    let lowered = release_dir.join("contract-mvp.wasm");
    let raw = std::fs::read_dir(&release_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "wasm") && p.file_name() != lowered.file_name()
        })
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .expect("no .wasm produced");

    // near-vm accepts sign-ext + nontrapping-fptoint natively, but NOT bulk-memory.
    // Enable the first two so wasm-opt validates the 1.93 input; lower ONLY bulk-memory.
    let st = Command::new("wasm-opt")
        .args([
            "-O4",
            "--enable-sign-ext",
            "--enable-nontrapping-float-to-int",
            "--enable-bulk-memory",
            "--llvm-memory-copy-fill-lowering",
            raw.to_str().unwrap(),
            "-o",
            lowered.to_str().unwrap(),
        ])
        .status()
        .expect("spawn wasm-opt");
    assert!(st.success(), "wasm-opt lowering failed: {raw:?}");
    lowered
}
