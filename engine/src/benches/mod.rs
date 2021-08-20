use criterion::Criterion;

mod eth_deploy_code;
mod eth_erc20;
mod eth_standard_precompiles;
mod eth_transfer;

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
