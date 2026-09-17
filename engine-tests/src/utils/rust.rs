use std::path::{Path, PathBuf};

pub fn compile<P: AsRef<Path>>(manifest_path: P) -> PathBuf {
    let opts = cargo_near_build::BuildOpts {
        no_locked: true,
        no_abi: true,
        no_embed_abi: true,
        no_doc: true,
        manifest_path: Some(
            cargo_near_build::camino::Utf8PathBuf::from_path_buf(
                manifest_path.as_ref().join("Cargo.toml"),
            )
            .unwrap(),
        ),
        skip_rust_version_check: true,
        // The newer wasm linker (rust-lld) no longer auto-imports the undefined NEAR host
        // functions, so we must pass `--import-undefined` explicitly. This overrides
        // cargo-near-build's default rustflags (`-C link-arg=-s`), which we keep, and
        // `--cfg near` is still force-appended by the builder afterwards.
        env: vec![(
            "RUSTFLAGS".to_string(),
            "-C link-arg=-s -C link-arg=--import-undefined".to_string(),
        )],
        ..Default::default()
    };

    cargo_near_build::build(opts)
        .map(|a| a.path.into_std_path_buf())
        .unwrap()
}
