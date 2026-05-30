use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

// Serialize helper-contract builds: parallel tests building the same contract
// would otherwise race on the shared `wasm-opt` output file and occasionally
// load a half-written wasm (CompilationError(PrepareError(Deserialization))).
static BUILD_LOCK: Mutex<()> = Mutex::new(());

pub fn compile<P: AsRef<Path>>(manifest_path: P) -> PathBuf {
    build_contract_1_93(&manifest_path.as_ref().join("Cargo.toml"), None)
}

/// Build a NEAR helper contract with the workspace toolchain (rust 1.93).
///
/// Bare `no_std` contracts (only `wee_alloc` / bare `extern "C"`) build for the
/// MVP `wasm32v1-none` target, which emits no bulk-memory opcodes, so they keep
/// the pre-bump (rust 1.86) gas profile. `near-sdk` contracts pull `std`-only
/// crates that do not compile for `wasm32v1-none`, so they fall back to
/// `wasm32-unknown-unknown` and have their bulk-memory (`memory.copy` /
/// `memory.fill`, emitted by rustc >= 1.87) lowered to MVP loops via
/// `wasm-opt --llvm-memory-copy-fill-lowering` — near-vm singlepass
/// (`reftypes_bulk_memory = false`) cannot deserialize bulk-memory.
pub fn build_contract_1_93(manifest_path: &Path, features: Option<&str>) -> PathBuf {
    let _guard = BUILD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = std::fs::canonicalize(manifest_path.parent().unwrap())
        .unwrap_or_else(|e| panic!("cannot canonicalize {manifest_path:?}: {e}"));

    try_build(&dir, "wasm32v1-none", features, false)
        .or_else(|| try_build(&dir, "wasm32-unknown-unknown", features, true))
        .unwrap_or_else(|| panic!("contract build failed for {manifest_path:?}"))
}

fn try_build(dir: &Path, target: &str, features: Option<&str>, lower: bool) -> Option<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("RUSTFLAGS", "-C link-arg=-s")
        .args(["build", "--release", "--target", target]);
    if let Some(f) = features {
        cmd.args(["--features", f]);
    }
    if target == "wasm32v1-none" {
        // wasm32v1-none is a probe: `near-sdk` contracts fail here (they pull
        // `std`-only crates) before falling back to wasm32-unknown-unknown.
        // Silence the thousands of expected `can't find crate for std` errors.
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    if !cmd.status().ok()?.success() {
        return None;
    }

    let release_dir = dir.join(format!("target/{target}/release"));
    let lowered = release_dir.join("contract-mvp.wasm");
    let raw = std::fs::read_dir(&release_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "wasm")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("contract-mvp"))
        })
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())?;

    // near-vm accepts sign-extension and nontrapping-float-to-int natively, but
    // NOT bulk-memory. Enable the first two so wasm-opt validates the rustc-1.93
    // input; lower bulk-memory only on the wasm32-unknown-unknown fallback.
    let mut args = vec![
        "--enable-sign-ext",
        "--enable-nontrapping-float-to-int",
        "-O4",
    ];
    if lower {
        args.push("--enable-bulk-memory");
        args.push("--llvm-memory-copy-fill-lowering");
    }
    args.extend([raw.to_str().unwrap(), "-o", lowered.to_str().unwrap()]);
    Command::new("wasm-opt")
        .args(&args)
        .status()
        .ok()?
        .success()
        .then_some(lowered)
}
