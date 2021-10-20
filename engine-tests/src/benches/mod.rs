use crate::tests::uniswap::UniswapTestContext;
use criterion::Criterion;

mod eth_deploy_code;
mod eth_erc20;
mod eth_standard_precompiles;
mod eth_transfer;
mod nft_pagination;
mod uniswap;

// We don't want to run in CI, so ignore. To run locally use `cargo test --release -- --ignored`
#[test]
#[ignore]
fn benches() {
    let mut c = Criterion::default();

    eth_deploy_code::eth_deploy_code_benchmark(&mut c);
    eth_erc20::eth_erc20_benchmark(&mut c);
    eth_standard_precompiles::eth_standard_precompiles_benchmark(&mut c);
    eth_transfer::eth_transfer_benchmark(&mut c);

    c.final_summary();
}

#[test]
#[ignore]
fn measure_nft_pagination_gas_usage() {
    const TOTAL_TOKENS: usize = 500;
    let data_sizes = [100, 500, 1000, 2000];
    let tokens_per_page = [1, 2, 4, 8, 16, 32];

    for size in data_sizes {
        for n_tokens in tokens_per_page {
            let gas_used = nft_pagination::measure_gas_usage(TOTAL_TOKENS, size, n_tokens);
            println!("{},{},{}", size, n_tokens, gas_used);
        }
    }
}

#[test]
#[ignore]
fn uniswap_benches() {
    let mut c = Criterion::default();

    let mut context = UniswapTestContext::new("uniswap-wasmer2");
    uniswap::uniswap_benchmark(&mut c, &mut context);

    let mut context = UniswapTestContext::new("uniswap-wasmer2-no-gas");
    context.no_gas();
    uniswap::uniswap_benchmark(&mut c, &mut context);

    c.final_summary();
}
