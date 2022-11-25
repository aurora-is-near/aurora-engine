use std::path::Path;
use std::process::Command;

pub fn compile<P: AsRef<Path>>(source_path: P) {
    let output = Command::new("cargo")
        .current_dir(source_path)
        .env("RUSTFLAGS", "-C link-arg=-s")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!("{}", String::from_utf8(output.stderr).unwrap());
    }
}
