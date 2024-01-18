use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Condvar, Mutex, Once};

pub mod liquidity_protocol;

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
                // We need to checkout a specific commit because the code in the current `master`
                // cannot be used with our version of `ethereum-types`, it gives the following error:
                // Error("unknown variant `error`, expected one of `constructor`, `function`, `event`, `fallback`, `receive`", line: 9, column: 21)
                let commit_hash =
                    git2::Oid::from_str("49ab85b3c39d916711495596a1bf811848437896").unwrap();
                repo.set_head_detached(commit_hash).unwrap();
                let mut opts = git2::build::CheckoutBuilder::new();
                repo.checkout_head(Some(opts.force())).unwrap();
            }
        }

        // install packages
        let status = Command::new("/usr/bin/env")
            .current_dir(&sources_dir)
            .args(["yarn", "install"])
            .status()
            .unwrap();
        assert!(
            status.success(),
            "Unsuccessful exit status while install hardhat dependencies: {status}"
        );

        let hardhat = |command: &str| {
            let status = Command::new("/usr/bin/env")
                .current_dir(&sources_dir)
                .args(["node_modules/hardhat/internal/cli/cli.js", command])
                .status()
                .unwrap();
            assert!(
                status.success(),
                "Unsuccessful exit status while install while executing `{command}`: {status}",
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
