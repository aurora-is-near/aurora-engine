use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use libsecp256k1::SecretKey;

use crate::prelude::Wei;
use crate::test_utils::{
    address_from_secret_key, create_deploy_transaction, deploy_evm, sign_transaction, SUBMIT,
};

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

pub(crate) fn eth_deploy_code_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    let inputs: Vec<_> = [1, 4, 8, 10, 13, 14]
        .iter()
        .copied()
        .map(|n| {
            let code_size = 2usize.pow(n);
            let code: Vec<u8> = vec![0; code_size];
            let transaction = create_deploy_transaction(code, INITIAL_NONCE.into());
            let signed_transaction =
                sign_transaction(transaction, Some(runner.chain_id), &source_account);
            rlp::encode(&signed_transaction).to_vec()
        })
        .collect();
    let calling_account_id = "some-account.near";

    // measure gas usage
    for input in inputs.iter() {
        let input_size = input.len();
        let (output, maybe_err) = runner
            .one_shot()
            .call(SUBMIT, calling_account_id, input.clone());
        assert!(maybe_err.is_none());
        let output = output.unwrap();
        let gas = output.burnt_gas;
        let eth_gas = crate::test_utils::parse_eth_gas(&output);
        // TODO(#45): capture this in a file
        println!("ETH_DEPLOY_CODE_{:?} NEAR GAS: {:?}", input_size, gas);
        println!("ETH_DEPLOY_CODE_{:?} ETH GAS: {:?}", input_size, eth_gas);
    }

    // measure wall-clock time
    let mut group = c.benchmark_group("deploy_code");
    for input in inputs {
        let input_size = u64::try_from(input.len()).unwrap();
        let id = BenchmarkId::from_parameter(input_size);
        group.throughput(Throughput::Bytes(input_size));
        group.bench_function(id, |b| {
            b.iter_batched(
                || (runner.one_shot(), calling_account_id, input.clone()),
                |(r, c, i)| r.call(SUBMIT, c, i),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}
