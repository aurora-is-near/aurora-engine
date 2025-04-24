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
        ..Default::default()
    };

    cargo_near_build::build(opts)
        .map(|a| a.path.into_std_path_buf())
        .unwrap()
}
