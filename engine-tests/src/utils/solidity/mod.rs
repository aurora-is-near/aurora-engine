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
        let artifacts_base_path = artifacts_base_path.as_ref();
        let lock = artifact_lock(artifacts_base_path, contract_name);
        let guard = lock.lock().unwrap_or_else(PoisonError::into_inner);

        compile(&sources_root, &contract_file, artifacts_base_path);
        let constructor = Self::load_artifacts(artifacts_base_path, contract_name);

        drop(guard);
        constructor
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
        let artifacts_base_path = artifacts_base_path.as_ref();
        // Tests run in parallel and many of them want the same contract, so several
        // threads reach this point at once with the artifacts not yet built. Deciding
        // whether to compile and then reading the result has to be atomic with respect
        // to those other threads: `solc` writes the `.bin` before the `.abi`, so a
        // thread that checked the artifacts while another thread's compile was
        // in-flight could find the bytecode present, skip compiling, and then fail to
        // open an `.abi` that does not exist yet. Reading a half-written `.bin` is the
        // quieter version of the same bug, since a truncated hex string still decodes
        // and only shows up later as an inexplicable EVM failure.
        let lock = artifact_lock(artifacts_base_path, contract_name);
        let guard = lock.lock().unwrap_or_else(PoisonError::into_inner);

        // Both artifacts are needed, so the presence of one says nothing useful about
        // whether this contract can be loaded without compiling it first.
        let (bin_path, abi_path) = artifact_paths(artifacts_base_path, contract_name);
        if !bin_path.exists() || !abi_path.exists() {
            compile(sources_root, contract_file, artifacts_base_path);
        }
        let constructor = Self::load_artifacts(artifacts_base_path, contract_name);

        drop(guard);
        constructor
    }

    /// Reads an already compiled contract's artifacts. Callers must hold the
    /// contract's `artifact_lock` so that no compile can be writing them.
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

/// One lock per compiled contract, so that two threads wanting different contracts do
/// not wait on each other. Keyed by artifact path rather than by contract name because
/// the same name can be built into more than one directory.
static ARTIFACT_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns the lock guarding `contract_name`'s artifacts in `artifacts_base_path`.
/// The lock must be held across the whole compile-then-read sequence, not just the
/// compile, so that no thread reads artifacts another thread is part way through
/// writing.
fn artifact_lock(artifacts_base_path: &Path, contract_name: &str) -> Arc<Mutex<()>> {
    let key = artifacts_base_path.join(contract_name);
    // These locks guard files rather than data, so a panicking test leaves nothing here
    // to be corrupted. Recovering from the poison keeps that original panic visible
    // instead of burying it under a `PoisonError` in every test that follows.
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
    fs::create_dir_all(&output_path).unwrap();
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
    use super::{artifact_lock, artifact_paths};
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn one_lock_per_contract() {
        let first = artifact_lock(Path::new("target/solidity_build"), "Foo");
        let second = artifact_lock(Path::new("target/solidity_build"), "Foo");

        assert!(
            Arc::ptr_eq(&first, &second),
            "threads wanting the same contract must wait on the same lock"
        );
    }

    #[test]
    fn unrelated_contracts_do_not_share_a_lock() {
        let foo = artifact_lock(Path::new("target/solidity_build"), "Foo");
        let bar = artifact_lock(Path::new("target/solidity_build"), "Bar");
        let foo_elsewhere = artifact_lock(Path::new("src/tests/res"), "Foo");

        assert!(
            !Arc::ptr_eq(&foo, &bar),
            "different contracts should compile concurrently"
        );
        assert!(
            !Arc::ptr_eq(&foo, &foo_elsewhere),
            "the same name in another directory is another artifact"
        );
    }

    #[test]
    fn artifacts_are_named_after_the_contract() {
        let (bin, abi) = artifact_paths(Path::new("target/solidity_build"), "Foo");

        assert_eq!(bin, Path::new("target/solidity_build/Foo.bin"));
        assert_eq!(abi, Path::new("target/solidity_build/Foo.abi"));
    }
}
