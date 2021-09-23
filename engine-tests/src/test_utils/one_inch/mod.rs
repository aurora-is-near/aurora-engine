use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

pub(crate) mod liquidity_protocol;

pub(crate) fn download_and_compile_solidity_sources(
    repo_name: &str,
    download_once: &'static Once,
    compile_once: &'static Once,
) -> PathBuf {
    let sources_dir = Path::new("target").join(repo_name);
    if !sources_dir.exists() {
        // Contracts not already present, so download them (but only once, even
        // if multiple tests running in parallel saw `contracts_dir` does not exist).
        download_once.call_once(|| {
            let url = format!("https://github.com/1inch/{}", repo_name);
            git2::Repository::clone(&url, &sources_dir).unwrap();
        });
    }

    compile_once.call_once(|| {
        // install packages
        let status = Command::new("/usr/bin/env")
            .current_dir(&sources_dir)
            .args(["yarn", "install"])
            .status()
            .unwrap();
        assert!(status.success());

        let hardhat = |command: &str| {
            let status = Command::new("/usr/bin/env")
                .current_dir(&sources_dir)
                .args(["node_modules/hardhat/internal/cli/cli.js", command])
                .status()
                .unwrap();
            assert!(status.success());
        };

        // clean and compile
        hardhat("clean");
        hardhat("compile");
    });

    sources_dir.join("artifacts/contracts")
}
