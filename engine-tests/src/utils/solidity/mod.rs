use crate::prelude::{transactions::legacy::TransactionLegacy, Address, U256};
use aurora_engine_types::types::Wei;
use serde::Deserialize;
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

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
        compile(&sources_root, &contract_file, &artifacts_base_path);
        Self::compile_from_source(
            sources_root,
            artifacts_base_path,
            contract_file,
            contract_name,
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
        let bin_file = format!("{contract_name}.bin");
        let abi_file = format!("{contract_name}.abi");
        let hex_path = artifacts_base_path.as_ref().join(bin_file);
        let hex_rep = fs::read_to_string(&hex_path).map_or_else(
            |_| {
                // An error occurred opening the file, maybe the contract hasn't been compiled?
                compile(sources_root, contract_file, &artifacts_base_path);
                // If another error occurs, then we can't handle it so we just unwrap.
                fs::read_to_string(hex_path).unwrap()
            },
            |hex| hex,
        );
        let code = hex::decode(hex_rep).unwrap();
        let abi_path = artifacts_base_path.as_ref().join(abi_file);
        let file = fs::File::open(abi_path).unwrap();
        let reader = BufReader::new(file);
        let abi = ethabi::Contract::load(reader).unwrap();

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
            "-v",
            &source_mount_arg,
            "-v",
            &output_mount_arg,
            "ethereum/solc:0.8.24", // TODO: 0.8.25 introduces support of the Dencun hard fork.
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
