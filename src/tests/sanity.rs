use crate::prelude::Address;
use crate::test_utils;
use crate::transaction::LegacyEthTransaction;
use crate::types::{Wei, ERC20_MINT_SELECTOR};
use secp256k1::SecretKey;

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(123);

/// Tests we can transfer Eth from one account to another and that the balances are correctly
/// updated.
#[test]
fn test_eth_transfer_success() {
    // set up Aurora runner and accounts
    let (mut runner, source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account);
    let transaction = test_utils::create_eth_transaction(
        Some(dest_address),
        TRANSFER_AMOUNT.into(),
        vec![],
        Some(runner.chain_id),
        &source_account,
    );
    let input = rlp::encode(&transaction).to_vec();
    let calling_account_id = "some-account.near".to_string();

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // perform transfer
    let (_, maybe_err) = runner.call(test_utils::SUBMIT, calling_account_id, input);
    assert!(maybe_err.is_none());

    // validate post-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE - TRANSFER_AMOUNT,
        (INITIAL_NONCE + 1).into(),
    );
    test_utils::validate_address_balance_and_nonce(
        &runner,
        dest_address,
        TRANSFER_AMOUNT,
        0.into(),
    );
}

/// Tests the case where the transfer amount is larger than the address balance
#[test]
fn test_eth_transfer_insufficient_balance() {
    let (mut runner, source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account);
    let transaction = test_utils::create_eth_transaction(
        Some(dest_address),
        INITIAL_BALANCE + INITIAL_BALANCE, // trying to transfer more than we have
        vec![],
        Some(runner.chain_id),
        &source_account,
    );
    let input = rlp::encode(&transaction).to_vec();
    let calling_account_id = "some-account.near".to_string();

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let (_, maybe_err) = runner.call(test_utils::SUBMIT, calling_account_id, input);
    let error_message = format!("{:?}", maybe_err);
    assert!(error_message.contains("ERR_OUT_OF_FUND"));

    // validate post-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        // the nonce is still incremented even though the transfer failed
        (INITIAL_NONCE + 1).into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());
}

/// Tests the case where the nonce on the transaction does not match the address
#[test]
fn test_eth_transfer_incorrect_nonce() {
    let (mut runner, source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account);
    let transaction = LegacyEthTransaction {
        nonce: (INITIAL_NONCE + 1).into(),
        gas_price: Default::default(),
        gas: Default::default(),
        to: Some(dest_address),
        value: TRANSFER_AMOUNT.into(),
        data: vec![],
    };
    let transaction =
        test_utils::sign_transaction(transaction, Some(runner.chain_id), &source_account);
    let input = rlp::encode(&transaction).to_vec();
    let calling_account_id = "some-account.near".to_string();

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let (_, maybe_err) = runner.call(test_utils::SUBMIT, calling_account_id, input);
    let error_message = format!("{:?}", maybe_err);
    assert!(error_message.contains("ERR_INCORRECT_NONCE"));

    // validate post-state (which is the same as pre-state in this case)
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());
}

#[test]
fn test_eth_transfer_not_enough_gas() {
    let (mut runner, source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account);
    let transaction = LegacyEthTransaction {
        nonce: INITIAL_NONCE.into(),
        gas_price: Default::default(),
        gas: 10_000.into(), // this is not enough gas
        to: Some(dest_address),
        value: TRANSFER_AMOUNT.into(),
        data: vec![],
    };
    let transaction =
        test_utils::sign_transaction(transaction, Some(runner.chain_id), &source_account);
    let input = rlp::encode(&transaction).to_vec();
    let calling_account_id = "some-account.near".to_string();

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let (_, maybe_err) = runner.call(test_utils::SUBMIT, calling_account_id, input);
    let error_message = format!("{:?}", maybe_err);
    assert!(error_message.contains("ERR_INTRINSIC_GAS"));

    // validate post-state (which is the same as pre-state in this case)
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());
}

fn initialize_transfer() -> (test_utils::AuroraRunner, SecretKey, Address) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));

    (runner, source_account, dest_address)
}

use sha3::Digest;

#[test]
fn check_selector() {
    // Selector to call mint function in ERC 20 contract
    //
    // keccak("mint(address,uint256)".as_bytes())[..4];
    let mut hasher = sha3::Keccak256::default();
    hasher.update(b"mint(address,uint256)");
    assert_eq!(hasher.finalize()[..4].to_vec(), ERC20_MINT_SELECTOR);
}
