use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Condvar, Mutex, Once};

pub mod liquidity_protocol;

const HASH_COMMIT: &str = "c8be9c67247880bd6ec88cf7ad2e040a16a483f2"; // tag 4.0.0

static READY: Mutex<bool> = Mutex::new(false);
static CV: Condvar = Condvar::new();

pub fn download_and_compile_solidity_sources(
    repo_name: &str,
    download_compile_once: &'static Once,
) -> PathBuf {
    let sources_dir = Path::new("target").join(repo_name);

    // Contracts not already present, so download and compile them (but only once, even
    // if multiple tests running in parallel saw `contracts_dir` does not exist).
    download_compile_once.call_once(|| {
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
            .args(["yarn", "install"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Unsuccessful exit status while install hardhat dependencies: {}",
            String::from_utf8_lossy(&output.stderr),
        );

        let hardhat = |command: &str| {
            let output = Command::new("/usr/bin/env")
                .current_dir(&sources_dir)
                .args(["node", "node_modules/hardhat/internal/cli/cli.js", command])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "Unsuccessful exit status while install while executing `{command}`: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };

        // clean and compile
        hardhat("clean");
        hardhat("compile");

        *READY.lock().unwrap() = true;
        CV.notify_all();
    });

    // Wait for finished compilation.
    let mut ready = READY.lock().unwrap();

    while !*ready {
        ready = CV.wait(ready).unwrap();
    }
    drop(ready);

    sources_dir.join("artifacts/contracts")
}
