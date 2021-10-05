use criterion::{BatchSize, BenchmarkId, Criterion};

use crate::prelude::U256;
use crate::test_utils::{self, SUBMIT};
use crate::tests::uniswap::UniswapTestContext;

const MINT_AMOUNT: u64 = 1_000_000_000;
const LIQUIDITY_AMOUNT: u64 = MINT_AMOUNT / 2;
const OUTPUT_AMOUNT: u64 = LIQUIDITY_AMOUNT / 100;

pub(crate) fn uniswap_benchmark(c: &mut Criterion, context: &mut UniswapTestContext) {
    let calling_account_id = "some-account.near";
    let chain_id = Some(context.runner.chain_id);
    let (token_a, token_b) = context.create_token_pair(MINT_AMOUNT.into());
    context.create_pool(&token_a, &token_b);

    // Approve spending the tokens
    context.approve_erc20(&token_a, context.manager.0.address, U256::MAX);
    context.approve_erc20(&token_b, context.manager.0.address, U256::MAX);
    context.approve_erc20(&token_a, context.swap_router.0.address, U256::MAX);
    context.approve_erc20(&token_b, context.swap_router.0.address, U256::MAX);

    // create transaction for adding liquidity
    let nonce = context.signer.use_nonce();
    let liquidity_params = context.mint_params(LIQUIDITY_AMOUNT.into(), &token_a, &token_b);
    let tx = context.manager.mint(liquidity_params, nonce.into());
    let signed_tx = test_utils::sign_transaction(tx, chain_id, &context.signer.secret_key);
    let liquidity_tx_bytes = rlp::encode(&signed_tx).to_vec();

    // create transaction for swapping
    let nonce = context.signer.use_nonce();
    let swap_params = context.exact_output_single_params(OUTPUT_AMOUNT.into(), &token_a, &token_b);
    let tx = context
        .swap_router
        .exact_output_single(swap_params, nonce.into());
    let signed_tx = test_utils::sign_transaction(tx, chain_id, &context.signer.secret_key);
    let swap_tx_bytes = rlp::encode(&signed_tx).to_vec();

    let mut group = c.benchmark_group(&context.name);
    let liquidity_id = BenchmarkId::from_parameter("add_liquidity");
    let swap_id = BenchmarkId::from_parameter("swap");

    // measure add_liquidity wall-clock time
    group.bench_function(liquidity_id, |b| {
        b.iter_batched(
            || {
                (
                    context.runner.one_shot(),
                    calling_account_id,
                    liquidity_tx_bytes.clone(),
                )
            },
            |(r, c, i)| r.call(SUBMIT, c, i),
            BatchSize::SmallInput,
        )
    });

    // Measure add_liquidity gas usage; don't use `one_shot` because we want to keep
    // this state change for the next benchmark where we swap some tokens in the pool.
    let (output, maybe_error) =
        context
            .runner
            .call(SUBMIT, calling_account_id, liquidity_tx_bytes.clone());
    assert!(maybe_error.is_none());
    let output = output.unwrap();
    let gas = output.burnt_gas;
    let eth_gas = crate::test_utils::parse_eth_gas(&output);
    // TODO(#45): capture this in a file
    println!("UNISWAP_ADD_LIQUIDITY NEAR GAS: {:?}", gas);
    println!("UNISWAP_ADD_LIQUIDITY ETH GAS: {:?}", eth_gas);

    // Measure swap gas usage
    let (output, maybe_error) =
        context
            .runner
            .one_shot()
            .call(SUBMIT, calling_account_id, swap_tx_bytes.clone());
    assert!(maybe_error.is_none());
    let output = output.unwrap();
    let gas = output.burnt_gas;
    let eth_gas = crate::test_utils::parse_eth_gas(&output);
    // TODO(#45): capture this in a file
    println!("UNISWAP_SWAP NEAR GAS: {:?}", gas);
    println!("UNISWAP_SWAP ETH GAS: {:?}", eth_gas);

    // measure add_liquidity wall-clock time
    group.bench_function(swap_id, |b| {
        b.iter_batched(
            || {
                (
                    context.runner.one_shot(),
                    calling_account_id,
                    swap_tx_bytes.clone(),
                )
            },
            |(r, c, i)| r.call(SUBMIT, c, i),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}
