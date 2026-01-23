//! # alt-bn-256 precompiles tests
//!
//! Tests based on parse data from:
//! <https://github.com/ethereum/execution-spec-tests/releases/tag/pectra-devnet-5%40v1.2.0>
//! for Prague hard fork.
//!
//! Tests based on parsed `execution-spec-tests`
//! data for `alt-bn-256` precompiles but distilled to input/output data only.
//!
//! Full EVM state tests has only limited count. As we can't send big bunch of test
//! cases to NEAR VM, as it's extremely expensive operation from time
//! consumption point of view.
//!
//! JSON test data set fully represents all tests from `execution-spec-tests` for
//! `alt-bn-128` precompiles. We run this test in standalone manner.

use aurora_engine_precompiles::Istanbul;
use aurora_engine_precompiles::Precompile;
use aurora_engine_precompiles::alt_bn256::{Bn256Add, Bn256Mul, Bn256Pair};
use near_primitives_core::gas::Gas;

use crate::prelude::{Address, H160, U256, Wei};
use crate::tests::sanity::initialize_transfer;
use crate::utils;

/// Precompile input and output data struct
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PrecompileStandaloneData {
    pub input: String,
    pub output: String,
}

/// JSON distilled data for precompile tests.
/// It contains input data for precompile and expected
/// output after precompile execution.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PrecompileStandalone {
    pub precompile_data: Vec<PrecompileStandaloneData>,
}

impl PrecompileStandalone {
    fn new(data: &str) -> Self {
        serde_json::from_str(data).unwrap()
    }
}

/// Run precompile with specific input data from the file.
/// It executes precompile it two ways: run directly and check result with expected output
fn run_alt_bn128(precompile: &impl Precompile, address: Address, data: &str, gas_limit: u64) {
    for data in PrecompileStandalone::new(data).precompile_data {
        let input = hex::decode(data.input.clone()).unwrap();
        let output = hex::decode(data.output.clone()).unwrap();

        let ctx = aurora_evm::Context {
            address: H160::default(),
            caller: H160::default(),
            apparent_value: U256::zero(),
        };
        // Run precompile directly with specific input and validate output result
        let standalone_result = precompile.run(&input, None, &ctx, false).unwrap();
        assert_eq!(standalone_result.output, output);

        check_wasm_submit(address, input, &output, gas_limit);
    }
}

/// Submit transaction to precompile address and check result with expected output.
fn check_wasm_submit(address: Address, input: Vec<u8>, expected_output: &[u8], gas_limit: u64) {
    let (mut runner, mut signer, _) = initialize_transfer();
    runner.context.prepaid_gas = Gas::MAX;

    let (submit_res, wasm_result) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            aurora_engine_transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(address),
                value: Wei::zero(),
                data: input,
            }
        })
        .unwrap();

    assert_gas_bound(wasm_result.all_gas(), gas_limit);
    assert_eq!(expected_output, utils::unwrap_success_slice(&submit_res));
}

/// Checks if `total_gas` is within 1 `GGas` of `ggas_bound`.
fn assert_gas_bound(total_gas: u64, bound_ggas: u64) {
    const GIGA: i128 = 1_000_000_000;
    let total_gas: i128 = total_gas.into();
    let bound_gas: i128 = i128::from(bound_ggas) * GIGA;

    assert!(
        total_gas <= bound_gas,
        "total: {} > bound: {bound_ggas} GGas",
        total_gas / GIGA
    );
}

#[test]
fn test_alt_bn128_add() {
    run_alt_bn128(
        &Bn256Add::<Istanbul>::new(),
        Bn256Add::<Istanbul>::ADDRESS,
        include_str!("res/alt_bn_128/bn256_add.json"),
        3587, // 3.587 TGas
    );
}

#[test]
fn test_alt_bn128_mul() {
    run_alt_bn128(
        &Bn256Mul::<Istanbul>::new(),
        Bn256Mul::<Istanbul>::ADDRESS,
        include_str!("res/alt_bn_128/bn256_mul.json"),
        10226, // 10.226 TGas
    );
}

#[test]
fn test_alt_bn128_pairing() {
    run_alt_bn128(
        &Bn256Pair::<Istanbul>::new(),
        Bn256Pair::<Istanbul>::ADDRESS,
        include_str!("res/alt_bn_128/bn256_pairing.json"),
        44098, // 44.098 TGas
    );
}
