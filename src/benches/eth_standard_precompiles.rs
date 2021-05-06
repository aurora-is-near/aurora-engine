use crate::prelude::U256;
use crate::transaction::EthTransaction;
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
    let constructor = PrecompilesConstructor::load();
    let contract = PrecompilesContract(runner.deploy_contract(
        &source_account,
        |c| c.deploy(INITIAL_NONCE.into()),
        constructor,
    ));

    let test_names = PrecompilesContract::all_method_names();
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

struct PrecompilesConstructor(solidity::ContractConstructor);

struct PrecompilesContract(solidity::DeployedContract);

impl From<PrecompilesConstructor> for solidity::ContractConstructor {
    fn from(c: PrecompilesConstructor) -> Self {
        c.0
    }
}

impl PrecompilesConstructor {
    fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_source(
            Self::sources_root(),
            Self::solidity_artifacts_path(),
            "StandardPrecompiles.sol",
            "StandardPrecompiles",
        ))
    }

    fn deploy(&self, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
            .encode_input(self.0.code.clone(), &[])
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

    fn sources_root() -> PathBuf {
        Path::new("src").join("benches").join("res")
    }
}

impl PrecompilesContract {
    fn call_method(&self, method_name: &str, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .function(method_name)
            .unwrap()
            .encode_input(&[])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: Some(self.0.address),
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
