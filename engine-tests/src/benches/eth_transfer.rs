use criterion::{BatchSize, Criterion};
use libsecp256k1::SecretKey;

use crate::prelude::Wei;
use crate::test_utils::{address_from_secret_key, create_eth_transaction, deploy_evm, SUBMIT};

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(123);

pub(crate) fn eth_transfer_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    let dest_account = address_from_secret_key(&SecretKey::random(&mut rng));
    let transaction = create_eth_transaction(
        Some(dest_account),
        TRANSFER_AMOUNT,
        vec![],
        Some(runner.chain_id),
        &source_account,
    );
    let input = rlp::encode(&transaction).to_vec();
    let calling_account_id = "some-account.near";

    // measure gas usage
    let (output, maybe_err) = runner
        .one_shot()
        .call(SUBMIT, calling_account_id, input.clone());
    assert!(maybe_err.is_none());
    let gas = output.unwrap().burnt_gas;
    // TODO(#45): capture this in a file
    println!("ETH_TRANSFER NEAR GAS: {:?}", gas);
    println!("ETH_TRANSFER ETH GAS: {:?}", 21_000);

    // measure wall-clock time
    c.bench_function("eth_transfer", |b| {
        b.iter_batched(
            || (runner.one_shot(), calling_account_id, input.clone()),
            |(r, c, i)| r.call(SUBMIT, c, i),
            BatchSize::SmallInput,
        )
    });
}
