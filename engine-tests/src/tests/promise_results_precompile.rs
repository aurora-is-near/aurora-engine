use crate::test_utils::{self, standalone};
use aurora_engine_precompiles::promise_result;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::{
    types::{PromiseResult, Wei},
    U256,
};
use borsh::BorshSerialize;

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
        .submit_transaction(&signer.secret_key, transaction.clone())
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
