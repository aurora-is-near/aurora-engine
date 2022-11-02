use crate::prelude::U256;
use criterion::{BatchSize, BenchmarkId, Criterion};
use libsecp256k1::SecretKey;

use crate::prelude::Wei;
use crate::test_utils::standard_precompiles::{PrecompilesConstructor, PrecompilesContract};
use crate::test_utils::{address_from_secret_key, deploy_evm, sign_transaction, SUBMIT};

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

pub(crate) fn eth_standard_precompiles_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    let calling_account_id = "some-account.near";

    // deploy StandardPrecompiles contract
    let constructor = PrecompilesConstructor::load();
    let contract = PrecompilesContract(runner.deploy_contract(
        &source_account,
        |c| c.deploy(INITIAL_NONCE.into()),
        constructor,
    ));

    let test_names = PrecompilesContract::all_method_names();
    let bench_ids = test_names.iter().map(BenchmarkId::from_parameter);

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
                .call(SUBMIT, calling_account_id, tx_bytes.clone());
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
                || (runner.one_shot(), calling_account_id, tx_bytes.clone()),
                |(r, c, i)| r.call(SUBMIT, c, i),
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}
