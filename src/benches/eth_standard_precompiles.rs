use crate::parameters::SubmitResult;
use crate::prelude::{Address, U256};
use crate::transaction::EthTransaction;
use borsh::BorshDeserialize;
use criterion::{BatchSize, BenchmarkId, Criterion};
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};

use crate::test_utils::solidity;
use crate::test_utils::{address_from_secret_key, deploy_evm, sign_transaction, SUBMIT};

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;

pub(crate) fn eth_standard_precompiles_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE.into(),
        INITIAL_NONCE.into(),
    );
    let calling_account_id = "some-account.near".to_string();

    // deploy StandardPrecompiles contract
    let constructor = ContractConstructor::load();
    let tx = constructor.deploy(INITIAL_NONCE.into());
    let signed_tx = sign_transaction(tx, Some(runner.chain_id), &source_account);
    let (output, maybe_err) = runner.call(
        SUBMIT,
        calling_account_id.clone(),
        rlp::encode(&signed_tx).to_vec(),
    );
    assert!(maybe_err.is_none());
    let submit_result =
        SubmitResult::try_from_slice(&output.unwrap().return_data.as_value().unwrap()).unwrap();
    let contract_address = Address::from_slice(&submit_result.result);
    let contract = Contract {
        abi: constructor.abi,
        address: contract_address,
    };

    let test_names = Contract::all_method_names();
    let bench_ids: Vec<_> = test_names.iter().map(BenchmarkId::from_parameter).collect();

    // create testing transactions
    let transactions: Vec<_> = test_names
        .iter()
        .map(|method_name| {
            let tx = contract.call_method(method_name, U256::from(INITIAL_NONCE + 1));
            let signed_tx = sign_transaction(tx, Some(runner.chain_id), &source_account);
            rlp::encode(&signed_tx).to_vec()
        })
        .collect();

    // measure gas usage
    for (tx_bytes, name) in transactions.iter().zip(test_names.iter()) {
        let (output, maybe_err) =
            runner
                .one_shot()
                .call(SUBMIT, calling_account_id.clone(), tx_bytes.clone());
        assert!(maybe_err.is_none());
        let output = output.unwrap();
        let gas = output.burnt_gas;
        let eth_gas = crate::test_utils::parse_eth_gas(&output);
        // TODO(#45): capture this in a file
        println!("ETH_STANDARD_PRECOMPILES_{} NEAR GAS: {:?}", name, gas);
        println!("ETH_STANDARD_PRECOMPILES_{} ETH GAS: {:?}", name, eth_gas);
    }

    let mut group = c.benchmark_group("standard_precompiles");

    // measure wall-clock time
    for (tx_bytes, id) in transactions.iter().zip(bench_ids.into_iter()) {
        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    (
                        runner.one_shot(),
                        calling_account_id.clone(),
                        tx_bytes.clone(),
                    )
                },
                |(r, c, i)| r.call(SUBMIT, c, i),
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

struct ContractConstructor {
    abi: ethabi::Contract,
    code: Vec<u8>,
}

struct Contract {
    abi: ethabi::Contract,
    address: Address,
}

impl ContractConstructor {
    fn load() -> Self {
        let artifacts_base_path = Self::solidity_artifacts_path();
        let hex_path = artifacts_base_path.join("StandardPrecompiles.bin");
        let hex_rep = match std::fs::read_to_string(&hex_path) {
            Ok(hex) => hex,
            Err(_) => {
                // An error occurred opening the file, maybe the contract hasn't been compiled?
                let sources_root = Path::new("src").join("benches").join("res");
                solidity::compile(
                    sources_root,
                    "StandardPrecompiles.sol",
                    &artifacts_base_path,
                );
                // If another error occurs, then we can't handle it so we just unwrap.
                std::fs::read_to_string(hex_path).unwrap()
            }
        };
        let code = hex::decode(&hex_rep).unwrap();
        let abi_path = artifacts_base_path.join("StandardPrecompiles.abi");
        let reader = std::fs::File::open(abi_path).unwrap();
        let abi = ethabi::Contract::load(reader).unwrap();

        Self { abi, code }
    }

    fn deploy(&self, nonce: U256) -> EthTransaction {
        let data = self
            .abi
            .constructor()
            .unwrap()
            .encode_input(self.code.clone(), &[])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: None,
            value: Default::default(),
            data,
        }
    }

    fn solidity_artifacts_path() -> PathBuf {
        Path::new("target").join("solidity_build")
    }
}

impl Contract {
    fn call_method(&self, method_name: &str, nonce: U256) -> EthTransaction {
        let data = self
            .abi
            .function(method_name)
            .unwrap()
            .encode_input(&[])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: Some(self.address),
            value: Default::default(),
            data,
        }
    }

    fn all_method_names() -> &'static [&'static str] {
        &[
            "test_ecrecover",
            "test_sha256",
            "test_ripemd160",
            "test_identity",
            "test_modexp",
            "test_ecadd",
            "test_ecmul",
            // TODO(#46): ecpair uses up all the gas (by itself) for some reason, need to look into this.
            // "test_ecpair",
            "test_blake2f",
            "test_all",
        ]
    }
}
