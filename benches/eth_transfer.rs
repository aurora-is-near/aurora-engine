use criterion::{criterion_group, BatchSize, Criterion};
use secp256k1::SecretKey;

use super::{address_from_secret_key, create_eth_transaction, deploy_evm, RAW_CALL};

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 123;

fn eth_transfer_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE.into(),
        INITIAL_NONCE.into(),
    );
    let dest_account = address_from_secret_key(&SecretKey::random(&mut rng));
    let transaction = create_eth_transaction(
        Some(dest_account),
        TRANSFER_AMOUNT.into(),
        vec![],
        Some(runner.chain_id),
        &source_account,
    );
    let input = rlp::encode(&transaction).to_vec();
    let calling_account_id = "some-account.near".to_string();

    // measure gas usage
    let (output, maybe_err) =
        runner
            .one_shot()
            .call(RAW_CALL, calling_account_id.clone(), input.clone());
    assert!(maybe_err.is_none());
    let gas = output.unwrap().burnt_gas;
    // TODO(#45): capture this in a file
    println!("ETH_TRANSFER NEAR GAS: {:?}", gas);
    #[cfg(feature = "profile_eth_gas")]
    println!("ETH_TRANSFER ETH GAS: {:?}", 21_000);

    // measure wall-clock time
    c.bench_function("eth_transfer", |b| {
        b.iter_batched(
            || (runner.one_shot(), calling_account_id.clone(), input.clone()),
            |(r, c, i)| r.call(RAW_CALL, c, i),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, eth_transfer_benchmark);
