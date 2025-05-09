use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

pub mod liquidity_protocol;

const HASH_COMMIT: &str = "c8be9c67247880bd6ec88cf7ad2e040a16a483f2"; // tag 4.0.0

pub static LIQUIDITY_PROTOCOL_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| download_and_compile_solidity_sources("liquidity-protocol"));

pub static LIMIT_ORDER_PROTOCOL_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| download_and_compile_solidity_sources("limit-order-protocol"));

fn download_and_compile_solidity_sources(repo_name: &str) -> PathBuf {
    let sources_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target")
        .join(repo_name);
    // Contracts not already present, so download and compile them (but only once, even
    // if multiple tests running in parallel saw `contracts_dir` does not exist).
    if !sources_dir.exists() {
        let url = format!("https://github.com/1inch/{repo_name}");
        let repo = git2::Repository::clone(&url, &sources_dir).unwrap();

        if repo_name == "limit-order-protocol" {
            let commit_hash = git2::Oid::from_str(HASH_COMMIT).unwrap();
            repo.set_head_detached(commit_hash).unwrap();
            let mut opts = git2::build::CheckoutBuilder::new();
            repo.checkout_head(Some(opts.force())).unwrap();
        }
    }

    // install packages
    let output = Command::new("/usr/bin/env")
        .current_dir(&sources_dir)
        // The `--cache-folder` argument should be provided because there could be a case when
        // two instances of yarn are running in parallel, and they are trying to install
        // the same dependencies.
        .args(["yarn", "install", "--cache-folder", repo_name])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Unsuccessful exit status while install hardhat dependencies: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let mut num_attempts = 5;
    // clean and compile EVM contracts
    while let Err(e) = compile_evm_contracts(&sources_dir) {
        assert_ne!(num_attempts, 0, "Failed to compile EVM contracts: {e}");
        num_attempts -= 1;
    }

    sources_dir.join("artifacts/contracts")
}

fn compile_evm_contracts(sources_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    hardhat(&sources_dir, "clean")?;
    hardhat(sources_dir, "compile")?;

    Ok(())
}

fn hardhat(sources_dir: impl AsRef<Path>, command: &str) -> anyhow::Result<()> {
    let output = Command::new("/usr/bin/env")
        .current_dir(sources_dir)
        .args(["node", "node_modules/hardhat/internal/cli/cli.js", command])
        .output()?;

    anyhow::ensure!(
        output.status.success(),
        "Unsuccessful exit status while executing: `hardhat {command}`\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(())
}
