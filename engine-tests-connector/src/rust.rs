use std::path::{Path, PathBuf};

pub fn compile<P: AsRef<Path>>(manifest_path: P) -> PathBuf {
    let opts = cargo_near_build::BuildOpts {
        no_locked: false,
        no_abi: true,
        no_embed_abi: true,
        no_doc: true,
        manifest_path: Some(
            cargo_near_build::camino::Utf8PathBuf::from_path_buf(
                manifest_path.as_ref().join("Cargo.toml"),
            )
            .unwrap(),
        ),
        // nearcore VM rejects wasm produced by rustc >= 1.87, so pin the
        // contract build to the older toolchain.
        override_toolchain: Some("1.86".to_string()),
        ..Default::default()
    };

    cargo_near_build::build(opts)
        .map(|a| a.path.into_std_path_buf())
        .unwrap()
}
