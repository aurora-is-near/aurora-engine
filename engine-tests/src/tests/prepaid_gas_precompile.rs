use crate::test_utils::{self, standalone};
use aurora_engine_precompiles::prepaid_gas;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::{types::Wei, U256};

#[test]
fn test_prepaid_gas_precompile() {
    let mut signer = test_utils::Signer::random();
    let mut runner = test_utils::deploy_evm();
    let mut standalone = standalone::StandaloneRunner::default();

    standalone.init_evm();
    runner.standalone_runner = Some(standalone);

    let transaction = TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(prepaid_gas::ADDRESS),
        value: Wei::zero(),
        data: Vec::new(),
    };

    const EXPECTED_VALUE: u64 = 157_277_246_352_223;
    runner.context.prepaid_gas = EXPECTED_VALUE;
    let result = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap();

    assert_eq!(
        U256::from(EXPECTED_VALUE),
        U256::from_big_endian(test_utils::unwrap_success_slice(&result)),
    );

    // confirm the precompile works in view calls too
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let result = runner
        .view_call(test_utils::as_view_call(transaction, sender))
        .unwrap();
    assert!(result.is_ok());
}
