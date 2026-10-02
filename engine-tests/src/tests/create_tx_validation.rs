//! Creation transactions that the EVM would fail before consuming the sender nonce (oversized init
//! code, insufficient balance for `value`) must be rejected as invalid instead of being charged:
//! otherwise the same signed transaction could be submitted and paid for again and again.

use aurora_engine::engine::{CREATE_TX_VALIDATION_HEIGHT, EngineErrorKind, GasPaymentError};
use aurora_engine::parameters::TransactionStatus;
use aurora_engine_sdk::types::near_account_to_evm_address;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::U256;
use aurora_engine_types::types::{Address, Wei};
use aurora_evm::ExitError;
use engine_standalone_storage::sync;

use crate::utils::{self, AuroraRunner, CALLER_ACCOUNT_ID, Signer, standalone::StandaloneRunner};

/// EIP-3860 init code size limit.
const MAX_INITCODE_SIZE: usize = 49_152;
const GAS_PRICE: u64 = 1;
const GAS_LIMIT: u64 = 1_000_000;
const PREPAID_GAS: u64 = GAS_LIMIT * GAS_PRICE;

fn deploy_tx(value: Wei, init_code: Vec<u8>) -> TransactionLegacy {
    TransactionLegacy {
        nonce: U256::zero(),
        gas_price: U256::from(GAS_PRICE),
        gas_limit: U256::from(GAS_LIMIT),
        to: None,
        value,
        data: init_code,
    }
}

fn setup(balance: Wei) -> (AuroraRunner, Signer, Address) {
    let mut runner = utils::deploy_runner();
    let signer = Signer::random();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    runner.create_address(sender, balance, U256::zero());
    (runner, signer, sender)
}

fn assert_untouched(runner: &AuroraRunner, sender: Address, balance: Wei) {
    assert_eq!(runner.get_nonce(sender), U256::zero());
    assert_eq!(runner.get_balance(sender), balance);
    // The relayer (the account calling `submit`) must not be paid for a rejected transaction.
    let relayer = near_account_to_evm_address(CALLER_ACCOUNT_ID.as_bytes());
    assert_eq!(runner.get_balance(relayer), Wei::zero());
}

#[test]
fn test_oversized_init_code_deployment_is_rejected_without_charge() {
    let balance = Wei::new_u64(10 * PREPAID_GAS);
    let (mut runner, signer, sender) = setup(balance);
    let tx = deploy_tx(Wei::zero(), vec![0u8; MAX_INITCODE_SIZE + 1]);

    // The same signed transaction is rejected every time instead of being charged every time.
    for _ in 0..2 {
        let error = runner
            .submit_transaction(&signer.secret_key, tx.clone())
            .unwrap_err();
        assert!(matches!(
            error.kind,
            EngineErrorKind::EvmError(ExitError::CreateContractLimit)
        ));
        assert_untouched(&runner, sender, balance);
    }

    // Exactly at the limit the deployment is executed as usual.
    let result = runner
        .submit_transaction(
            &signer.secret_key,
            deploy_tx(Wei::zero(), vec![0u8; MAX_INITCODE_SIZE]),
        )
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));
    assert_eq!(runner.get_nonce(sender), U256::one());
}

#[test]
fn test_deployment_with_uncovered_value_is_rejected_without_charge() {
    let balance = Wei::new_u64(10 * PREPAID_GAS);
    let (mut runner, signer, sender) = setup(balance);
    // The value exceeds what is left after the gas prepayment, but not the whole balance.
    let tx = deploy_tx(Wei::new_u64(9 * PREPAID_GAS + 1), vec![0x00]);

    for _ in 0..2 {
        let error = runner
            .submit_transaction(&signer.secret_key, tx.clone())
            .unwrap_err();
        assert!(matches!(
            error.kind,
            EngineErrorKind::GasPayment(GasPaymentError::OutOfFund)
        ));
        assert_untouched(&runner, sender, balance);
    }

    // A value that is exactly covered after the prepayment is deployed as usual.
    let result = runner
        .submit_transaction(
            &signer.secret_key,
            deploy_tx(Wei::new_u64(9 * PREPAID_GAS), vec![0x00]),
        )
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));
    assert_eq!(runner.get_nonce(sender), U256::one());
}

#[test]
fn test_creation_tx_checks_are_height_gated_for_standalone_replay() {
    let mut runner = StandaloneRunner::default();
    runner.init_evm();
    let signer = Signer::random();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    runner.mint_account(sender, Wei::new_u64(10 * PREPAID_GAS), U256::zero(), None);
    let tx = deploy_tx(Wei::zero(), vec![0u8; MAX_INITCODE_SIZE + 1]);

    // Before the fix height the historical behavior is reproduced: executed, failed with the whole
    // gas limit charged, nonce untouched. `submit_transaction` increments the height first.
    runner.env.block_height = CREATE_TX_VALIDATION_HEIGHT - 2;
    let result = runner
        .submit_transaction(&signer.secret_key, tx.clone())
        .unwrap();
    assert_eq!(result.status, TransactionStatus::CreateContractLimit);
    assert_eq!(result.gas_used, GAS_LIMIT);
    assert_eq!(runner.get_nonce(&sender), U256::zero());

    // From the fix height on the transaction is rejected before execution.
    runner.env.block_height = CREATE_TX_VALIDATION_HEIGHT - 1;
    let error = runner
        .submit_transaction(&signer.secret_key, tx)
        .unwrap_err();
    // The standalone engine reports the contract-method error, i.e. the on-chain panic message.
    assert!(
        matches!(
            &error,
            sync::error::Error::ContractError(e)
                if e.message.as_ref().as_ref() == b"CREATE_CONTRACT_LIMIT"
        ),
        "unexpected error: {error:?}"
    );
}
