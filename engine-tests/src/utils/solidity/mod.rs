use crate::prelude::{Address, U256, transactions::legacy::TransactionLegacy};
use aurora_engine_types::types::Wei;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

pub mod erc20;
pub mod exit_precompile;
pub mod random;
pub mod self_destruct;
pub mod standard_precompiles;
pub mod uniswap;
pub mod weth;

/// One lock per Solidity output directory. A single `solc` invocation can emit files
/// for multiple contracts, so the directory is the narrowest safe synchronization
/// boundary unless every compilation receives an isolated output directory.
static ARTIFACT_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct ContractConstructor {
    pub abi: ethabi::Contract,
    pub code: Vec<u8>,
}

pub struct DeployedContract {
    pub abi: ethabi::Contract,
    pub address: Address,
}

#[derive(Deserialize)]
struct ExtendedJsonSolidityArtifact {
    abi: ethabi::Contract,
    bytecode: String,
}

impl ContractConstructor {
    /// Same as `compile_from_source` but always recompiles instead of reusing artifacts when they exist.
    pub fn force_compile<P1, P2, P3>(
        sources_root: P1,
        artifacts_base_path: P2,
        contract_file: P3,
        contract_name: &str,
    ) -> Self
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
        P3: AsRef<Path>,
    {
        Self::compile_or_load(
            sources_root.as_ref(),
            artifacts_base_path.as_ref(),
            contract_file.as_ref(),
            contract_name,
            true,
            |source_path, contract_file, output_path| {
                compile(source_path, contract_file, output_path);
            },
        )
    }

    // Note: `contract_file` must be relative to `sources_root`
    pub fn compile_from_source<P1, P2, P3>(
        sources_root: P1,
        artifacts_base_path: P2,
        contract_file: P3,
        contract_name: &str,
    ) -> Self
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
        P3: AsRef<Path>,
    {
        Self::compile_or_load(
            sources_root.as_ref(),
            artifacts_base_path.as_ref(),
            contract_file.as_ref(),
            contract_name,
            false,
            |source_path, contract_file, output_path| {
                compile(source_path, contract_file, output_path);
            },
        )
    }

    fn compile_or_load<C>(
        sources_root: &Path,
        artifacts_base_path: &Path,
        contract_file: &Path,
        contract_name: &str,
        force_compile: bool,
        compiler: C,
    ) -> Self
    where
        C: FnOnce(&Path, &Path, &Path),
    {
        fs::create_dir_all(artifacts_base_path).unwrap_or_else(|e| {
            panic!(
                "Could not create Solidity artifact directory {}: {e}",
                artifacts_base_path.display()
            )
        });

        // `solc` writes artifacts for every contract in the source and import graph,
        // not only `contract_name`. Lock the complete output directory so concurrent
        // compilations cannot overwrite files outside a narrower per-contract lock.
        // The lock also covers the cache check and reads, preventing readers from
        // observing one artifact while another is still being written.
        let lock = artifact_lock(artifacts_base_path);
        let (_guard, recovered_from_poison) = match lock.lock() {
            Ok(guard) => (guard, false),
            Err(error) => (error.into_inner(), true),
        };

        let (bin_path, abi_path) = artifact_paths(artifacts_base_path, contract_name);
        if force_compile || recovered_from_poison || !bin_path.exists() || !abi_path.exists() {
            compiler(sources_root, contract_file, artifacts_base_path);
        }

        let constructor = Self::load_artifacts(artifacts_base_path, contract_name);
        if recovered_from_poison {
            // A successful compile/read cycle restores the invariant guarded by this
            // lock, so later cache hits do not needlessly recompile forever.
            lock.clear_poison();
        }

        constructor
    }

    /// Reads an already compiled contract's artifacts. Callers must hold the
    /// output directory's `artifact_lock` so that no compile can be writing them.
    fn load_artifacts(artifacts_base_path: &Path, contract_name: &str) -> Self {
        let (bin_path, abi_path) = artifact_paths(artifacts_base_path, contract_name);

        let hex_rep = fs::read_to_string(&bin_path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", bin_path.display()));
        let code = hex::decode(hex_rep)
            .unwrap_or_else(|e| panic!("Could not decode {}: {e}", bin_path.display()));
        let file = fs::File::open(&abi_path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", abi_path.display()));
        let reader = BufReader::new(file);
        let abi = ethabi::Contract::load(reader)
            .unwrap_or_else(|e| panic!("Could not parse {}: {e}", abi_path.display()));

        Self { abi, code }
    }

    pub fn compile_from_extended_json<P>(contract_path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let file = fs::File::open(contract_path).unwrap();
        let reader = BufReader::new(file);
        let contract: ExtendedJsonSolidityArtifact = serde_json::from_reader(reader).unwrap();

        Self {
            abi: contract.abi,
            code: hex::decode(&contract.bytecode[2..]).unwrap(),
        }
    }

    pub fn deployed_at(&self, address: Address) -> DeployedContract {
        DeployedContract {
            abi: self.abi.clone(),
            address,
        }
    }

    pub fn deploy_without_constructor(&self, nonce: U256) -> TransactionLegacy {
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: None,
            value: Wei::default(),
            data: self.code.clone(),
        }
    }

    pub fn deploy_without_args(&self, nonce: U256) -> TransactionLegacy {
        self.deploy_with_args(nonce, &[])
    }

    pub fn deploy_with_args(&self, nonce: U256, args: &[ethabi::Token]) -> TransactionLegacy {
        let data = self
            .abi
            .constructor()
            .unwrap()
            .encode_input(self.code.clone(), args)
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: None,
            value: Wei::default(),
            data,
        }
    }
}

impl DeployedContract {
    pub fn call_method_without_args(&self, method_name: &str, nonce: U256) -> TransactionLegacy {
        self.call_method_with_args(method_name, &[], nonce)
    }

    pub fn call_method_with_args(
        &self,
        method_name: &str,
        args: &[ethabi::Token],
        nonce: U256,
    ) -> TransactionLegacy {
        let data = self
            .abi
            .function(method_name)
            .unwrap()
            .encode_input(args)
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.address),
            value: Wei::default(),
            data,
        }
    }
}

/// Returns the lock guarding all files in `artifacts_base_path`.
fn artifact_lock(artifacts_base_path: &Path) -> Arc<Mutex<()>> {
    let key = fs::canonicalize(artifacts_base_path).unwrap_or_else(|e| {
        panic!(
            "Could not resolve Solidity artifact directory {}: {e}",
            artifacts_base_path.display()
        )
    });
    let mut locks = ARTIFACT_LOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    Arc::clone(locks.entry(key).or_default())
}

/// The bytecode and ABI files `solc` writes for `contract_name`.
fn artifact_paths(artifacts_base_path: &Path, contract_name: &str) -> (PathBuf, PathBuf) {
    (
        artifacts_base_path.join(format!("{contract_name}.bin")),
        artifacts_base_path.join(format!("{contract_name}.abi")),
    )
}

/// Compiles a solidity contract. `source_path` gives the directory containing all solidity
/// source files to consider (including imports). `contract_file` must be
/// given relative to `source_path`. `output_path` gives the directory where the compiled
/// artifacts are written. Requires Docker to be installed.
fn compile<P1, P2, P3>(source_path: P1, contract_file: P2, output_path: P3)
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
    P3: AsRef<Path>,
{
    let source_path = fs::canonicalize(source_path).unwrap();
    let output_path = fs::canonicalize(output_path).unwrap();
    let source_mount_arg = format!("{}:/contracts", source_path.to_str().unwrap());
    let output_mount_arg = format!("{}:/output", output_path.to_str().unwrap());
    let contract_arg = format!("/contracts/{}", contract_file.as_ref().to_str().unwrap());
    let output = Command::new("/usr/bin/env")
        .args([
            "docker",
            "run",
            "--rm",
            "-v",
            &source_mount_arg,
            "-v",
            &output_mount_arg,
            "ethereum/solc:stable",
            "--allow-paths",
            "/contracts/",
            "-o",
            "/output",
            "--abi",
            "--bin",
            "--overwrite",
            &contract_arg,
        ])
        .output()
        .unwrap();
    let cwd = std::env::current_dir();
    assert!(
        output.status.success(),
        "Could not compile solidity contracts in docker [source={source_mount_arg}, output={output_mount_arg}, contract={contract_arg}, workdir={cwd:?}]: {}",
        String::from_utf8(output.stderr).unwrap()
    );
}

#[cfg(test)]
mod tests {
    use super::{ContractConstructor, artifact_lock, artifact_paths};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn one_lock_per_output_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts = temp_dir.path().join("artifacts");
        fs::create_dir(&artifacts).unwrap();
        let first = artifact_lock(&artifacts);
        let second = artifact_lock(&artifacts);

        assert!(
            Arc::ptr_eq(&first, &second),
            "all writes to one output directory must use the same lock"
        );
    }

    #[test]
    fn separate_output_directories_do_not_share_a_lock() {
        let temp_dir = tempfile::tempdir().unwrap();
        let first_path = temp_dir.path().join("first");
        let second_path = temp_dir.path().join("second");
        fs::create_dir(&first_path).unwrap();
        fs::create_dir(&second_path).unwrap();
        let first = artifact_lock(&first_path);
        let second = artifact_lock(&second_path);

        assert!(
            !Arc::ptr_eq(&first, &second),
            "isolated output directories should compile concurrently"
        );
    }

    #[test]
    fn artifacts_are_named_after_the_contract() {
        let (bin, abi) = artifact_paths(Path::new("target/solidity_build"), "Foo");

        assert_eq!(bin, Path::new("target/solidity_build/Foo.bin"));
        assert_eq!(abi, Path::new("target/solidity_build/Foo.abi"));
    }

    #[test]
    fn concurrent_loads_compile_once_and_read_complete_artifacts() {
        const THREADS: usize = 8;

        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts = Arc::new(temp_dir.path().join("artifacts"));
        let start = Arc::new(Barrier::new(THREADS));
        let compile_count = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let artifacts = Arc::clone(&artifacts);
                let start = Arc::clone(&start);
                let compile_count = Arc::clone(&compile_count);
                thread::spawn(move || {
                    start.wait();
                    ContractConstructor::compile_or_load(
                        Path::new("unused-sources"),
                        &artifacts,
                        Path::new("Foo.sol"),
                        "Foo",
                        false,
                        |_, _, output_path| {
                            compile_count.fetch_add(1, Ordering::SeqCst);
                            fs::write(output_path.join("Foo.bin"), "6000").unwrap();
                            thread::sleep(Duration::from_millis(20));
                            fs::write(output_path.join("Foo.abi"), "[]").unwrap();
                        },
                    )
                })
            })
            .collect();

        for handle in handles {
            let constructor = handle.join().unwrap();
            assert_eq!(constructor.code, [0x60, 0x00]);
        }
        assert_eq!(compile_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn poisoned_artifacts_are_recompiled_once() {
        let temp_dir = tempfile::tempdir().unwrap();
        let artifacts = temp_dir.path().join("artifacts");

        let failed_compile = std::panic::catch_unwind(|| {
            ContractConstructor::compile_or_load(
                Path::new("unused-sources"),
                &artifacts,
                Path::new("Foo.sol"),
                "Foo",
                true,
                |_, _, output_path| {
                    fs::write(output_path.join("Foo.bin"), "60").unwrap();
                    fs::write(output_path.join("Foo.abi"), "[]").unwrap();
                    panic!("simulated compiler failure");
                },
            );
        });
        assert!(failed_compile.is_err());

        let compile_count = AtomicUsize::new(0);
        let constructor = ContractConstructor::compile_or_load(
            Path::new("unused-sources"),
            &artifacts,
            Path::new("Foo.sol"),
            "Foo",
            false,
            |_, _, output_path| {
                compile_count.fetch_add(1, Ordering::SeqCst);
                fs::write(output_path.join("Foo.bin"), "6000").unwrap();
                fs::write(output_path.join("Foo.abi"), "[]").unwrap();
            },
        );
        assert_eq!(constructor.code, [0x60, 0x00]);
        assert_eq!(compile_count.load(Ordering::SeqCst), 1);

        ContractConstructor::compile_or_load(
            Path::new("unused-sources"),
            &artifacts,
            Path::new("Foo.sol"),
            "Foo",
            false,
            |_, _, _| {
                compile_count.fetch_add(1, Ordering::SeqCst);
            },
        );
        assert_eq!(compile_count.load(Ordering::SeqCst), 1);
    }
}
