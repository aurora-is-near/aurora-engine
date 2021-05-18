use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use secp256k1::SecretKey;

use crate::test_utils::{address_from_secret_key, create_eth_transaction, deploy_evm, SUBMIT};
use crate::types::Wei;

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::zero();

pub(crate) fn eth_deploy_code_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    let inputs: Vec<_> = [1, 4, 8, 12, 16]
        .iter()
        .copied()
        .map(|n| {
            let code_size = 2usize.pow(n);
            let code: Vec<u8> = vec![0; code_size];
            let transaction = create_eth_transaction(
                None,
                TRANSFER_AMOUNT.into(),
                code,
                Some(runner.chain_id),
                &source_account,
            );
            rlp::encode(&transaction).to_vec()
        })
        .collect();
    let calling_account_id = "some-account.near".to_string();

    // measure gas usage
    for input in inputs.iter() {
        let input_size = input.len();
        let (output, maybe_err) =
            runner
                .one_shot()
                .call(SUBMIT, calling_account_id.clone(), input.clone());
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
        let input_size = input.len() as u64;
        let id = BenchmarkId::from_parameter(input_size);
        group.throughput(Throughput::Bytes(input_size));
        group.bench_function(id, |b| {
            b.iter_batched(
                || (runner.one_shot(), calling_account_id.clone(), input.clone()),
                |(r, c, i)| r.call(SUBMIT, c, i),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}
