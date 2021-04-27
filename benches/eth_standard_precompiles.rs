use aurora_engine::prelude::{Address, U256};
use aurora_engine::transaction::EthTransaction;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};

// We don't use everything in `common`, but that's ok, other benchmarks do
#[allow(dead_code)]
mod common;
mod solidity;

use common::{address_from_secret_key, deploy_evm, sign_transaction, RAW_CALL};

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;

fn eth_standard_precompiles_benchmark(c: &mut Criterion) {
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
        RAW_CALL,
        calling_account_id.clone(),
        rlp::encode(&signed_tx).to_vec(),
    );
    assert!(maybe_err.is_none());
    let contract_address = output.unwrap().return_data.as_value().unwrap();
    let contract = Contract {
        abi: constructor.abi,
        address: Address::from_slice(&contract_address),
    };

    // create testing transaction
    let tx = contract.test_all(U256::from(INITIAL_NONCE + 1));
    let signed_tx = sign_transaction(tx, Some(runner.chain_id), &source_account);
    let tx_bytes = rlp::encode(&signed_tx).to_vec();

    // measure gas usage
    let (output, maybe_err) =
        runner
            .one_shot()
            .call(RAW_CALL, calling_account_id.clone(), tx_bytes.clone());
    println!("{:?}", maybe_err);
    assert!(maybe_err.is_none());
    let gas = output.unwrap().burnt_gas;
    println!("ETH_STANDARD_PRECOMPILES GAS: {:?}", gas); // TODO: capture this in a file

    // measure wall-clock time
    c.bench_function("eth_standard_precompiles", |b| {
        b.iter_batched(
            || {
                (
                    runner.one_shot(),
                    calling_account_id.clone(),
                    tx_bytes.clone(),
                )
            },
            |(r, c, i)| r.call(RAW_CALL, c, i),
            BatchSize::SmallInput,
        )
    });
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
                let sources_root = Path::new("benches").join("res");
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
    fn test_all(&self, nonce: U256) -> EthTransaction {
        let data = self
            .abi
            .function("test_all")
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
}

criterion_group!(benches, eth_standard_precompiles_benchmark);
criterion_main!(benches);
