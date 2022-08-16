use crate::prelude::{transactions::legacy::TransactionLegacy, Address, U256};
use near_sdk::serde_json;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::Command;

pub(crate) struct ContractConstructor {
    pub abi: ethabi::Contract,
    pub code: Vec<u8>,
}

pub(crate) struct DeployedContract {
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
        let bin_file = format!("{}.bin", contract_name);
        let abi_file = format!("{}.abi", contract_name);
        let hex_path = artifacts_base_path.as_ref().join(&bin_file);
        let hex_rep = match std::fs::read_to_string(&hex_path) {
            Ok(hex) => hex,
            Err(_) => {
                // An error occurred opening the file, maybe the contract hasn't been compiled?
                compile(sources_root, contract_file, &artifacts_base_path);
                // If another error occurs, then we can't handle it so we just unwrap.
                std::fs::read_to_string(hex_path).unwrap()
            }
        };
        let code = hex::decode(&hex_rep).unwrap();
        let abi_path = artifacts_base_path.as_ref().join(&abi_file);
        let reader = std::fs::File::open(abi_path).unwrap();
        let abi = ethabi::Contract::load(reader).unwrap();

        Self { abi, code }
    }

    pub fn compile_from_extended_json<P>(contract_path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let reader = std::fs::File::open(contract_path).unwrap();
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
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: None,
            value: Default::default(),
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
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: None,
            value: Default::default(),
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
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.address),
            value: Default::default(),
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
        .args(&[
            "docker",
            "run",
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
    println!("{}", String::from_utf8(output.stdout).unwrap());
    if !output.status.success() {
        panic!("{}", String::from_utf8(output.stderr).unwrap());
    }
}
