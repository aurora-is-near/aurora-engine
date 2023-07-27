use crate::utils::{self};
use aurora_engine_precompiles::prepaid_gas;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::{types::Wei, U256};

#[test]
fn test_prepaid_gas_precompile() {
    const EXPECTED_VALUE: u64 = 157_277_246_352_223;
    let mut signer = utils::Signer::random();
    let mut runner = utils::deploy_runner();
    runner.cancel_hashchain();

    let transaction = TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(prepaid_gas::ADDRESS),
        value: Wei::zero(),
        data: Vec::new(),
    };

    runner.context.prepaid_gas = EXPECTED_VALUE;
    let result = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap();

    assert_eq!(
        U256::from(EXPECTED_VALUE),
        U256::from_big_endian(utils::unwrap_success_slice(&result)),
    );

    // confirm the precompile works in view calls too
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let result = runner
        .view_call(&utils::as_view_call(transaction, sender))
        .unwrap();
    assert!(result.is_ok());
}
