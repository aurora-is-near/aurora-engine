use crate::test_utils::{self, standalone};
use aurora_engine_precompiles::promise_result::{self, costs};
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::{
    types::{Address, EthGas, NearGas, PromiseResult, Wei},
    U256,
};
use borsh::BorshSerialize;

const NEAR_GAS_PER_EVM: u64 = 175_000_000;

#[test]
fn test_promise_results_precompile() {
    let mut signer = test_utils::Signer::random();
    let mut runner = test_utils::deploy_evm();

    let mut standalone = standalone::StandaloneRunner::default();
    standalone.init_evm();

    let promise_results = vec![
        PromiseResult::Successful(hex::decode("deadbeef").unwrap()),
        PromiseResult::Failed,
    ];

    let transaction = TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(promise_result::ADDRESS),
        value: Wei::zero(),
        data: Vec::new(),
    };

    runner.promise_results = promise_results.clone();
    let result = runner
        .submit_transaction(&signer.secret_key, transaction)
        .unwrap();

    let standalone_result = standalone
        .submit_raw("submit", &runner.context, &promise_results)
        .unwrap();

    assert_eq!(result, standalone_result);

    assert_eq!(
        test_utils::unwrap_success(result),
        promise_results.try_to_vec().unwrap(),
    );
}

#[test]
fn test_promise_result_gas_cost() {
    let mut runner = test_utils::deploy_evm();
    let mut standalone = standalone::StandaloneRunner::default();
    standalone.init_evm();
    runner.standalone_runner = Some(standalone);
    let mut signer = test_utils::Signer::random();
    runner.context.block_height = aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT + 1;

    // Baseline transaction that does essentially nothing.
    let (_, baseline) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| TransactionLegacy {
            nonce,
            gas_price: U256::zero(),
            gas_limit: u64::MAX.into(),
            to: Some(Address::from_array([0; 20])),
            value: Wei::zero(),
            data: Vec::new(),
        })
        .unwrap();

    let mut profile_for_promises = |promise_data: Vec<PromiseResult>| -> (u64, u64, u64) {
        let input_length: usize = promise_data.iter().map(PromiseResult::size).sum();
        runner.promise_results = promise_data;
        let (submit_result, profile) = runner
            .submit_with_signer_profiled(&mut signer, |nonce| TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(promise_result::ADDRESS),
                value: Wei::zero(),
                data: Vec::new(),
            })
            .unwrap();
        assert!(submit_result.status.is_ok());
        // Subtract off baseline transaction to isolate just precompile things
        (
            u64::try_from(input_length).unwrap(),
            profile.all_gas() - baseline.all_gas(),
            submit_result.gas_used,
        )
    };

    let promise_results = vec![
        PromiseResult::Successful(hex::decode("deadbeef").unwrap()),
        PromiseResult::Failed,
        PromiseResult::Successful(vec![1u8; 100]),
    ];

    let (x1, y1, evm1) = profile_for_promises(Vec::new());
    let (x2, y2, evm2) = profile_for_promises(promise_results);

    let cost_per_byte = (y2 - y1) / (x2 - x1);
    let base_cost = NearGas::new(y1 - cost_per_byte * x1);

    let base_cost = EthGas::new(base_cost.as_u64() / NEAR_GAS_PER_EVM);
    let cost_per_byte = cost_per_byte / NEAR_GAS_PER_EVM;

    assert!(
        test_utils::within_x_percent(
            5,
            base_cost.as_u64(),
            costs::PROMISE_RESULT_BASE_COST.as_u64()
        ),
        "Incorrect promise_result base cost. Expected: {} Actual: {}",
        base_cost,
        costs::PROMISE_RESULT_BASE_COST
    );

    assert!(
        test_utils::within_x_percent(5, cost_per_byte, costs::PROMISE_RESULT_BYTE_COST.as_u64()),
        "Incorrect promise_result per byte cost. Expected: {} Actual: {}",
        cost_per_byte,
        costs::PROMISE_RESULT_BYTE_COST
    );

    let total_gas1 = y1 + baseline.all_gas();
    let total_gas2 = y2 + baseline.all_gas();
    assert!(
        test_utils::within_x_percent(6, evm1, total_gas1 / NEAR_GAS_PER_EVM),
        "Incorrect EVM gas used. Expected: {} Actual: {}",
        evm1,
        total_gas1 / NEAR_GAS_PER_EVM
    );
    assert!(
        test_utils::within_x_percent(6, evm2, total_gas2 / NEAR_GAS_PER_EVM),
        "Incorrect EVM gas used. Expected: {} Actual: {}",
        evm2,
        total_gas2 / NEAR_GAS_PER_EVM
    );
}
