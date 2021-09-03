use crate::fungible_token::FungibleTokenMetadata;
use crate::parameters::{SubmitResult, TransactionStatus};
use crate::prelude::sdk;
use crate::prelude::{Address, U256};
use crate::prelude::{Wei, ERC20_MINT_SELECTOR};
use crate::test_utils;
use crate::tests::state_migration;
use borsh::BorshSerialize;
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000_000);
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(123);
const GAS_PRICE: u64 = 10;

/// Tests we can transfer Eth from one account to another and that the balances are correctly
/// updated.
#[test]
fn test_eth_transfer_success() {
    // set up Aurora runner and accounts
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // perform transfer
    runner
        .submit_with_signer(&mut source_account, |nonce| {
            test_utils::transfer(dest_address, TRANSFER_AMOUNT, nonce)
        })
        .unwrap();

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
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            test_utils::transfer(dest_address, INITIAL_BALANCE + INITIAL_BALANCE, nonce)
        })
        .unwrap();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

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
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let err = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // creating transaction with incorrect nonce
            test_utils::transfer(dest_address, TRANSFER_AMOUNT, nonce + 1)
        })
        .unwrap_err();
    let error_message = format!("{:?}", err);
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
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);
    let transaction = |nonce| {
        let mut tx = test_utils::transfer(dest_address, TRANSFER_AMOUNT, nonce);
        tx.gas = 10_000.into(); // this is not enough gas
        tx
    };

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let err = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap_err();
    let error_message = format!("{:?}", err);
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

#[test]
fn test_transfer_charging_gas_success() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);
    let transaction = |nonce| {
        let mut tx = test_utils::transfer(dest_address, TRANSFER_AMOUNT, nonce);
        tx.gas = 30_000.into();
        tx.gas_price = GAS_PRICE.into();
        tx
    };

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // do transfer
    let result = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap();
    let spent_amount = Wei::new_u64(GAS_PRICE * result.gas_used);
    let expected_source_balance = INITIAL_BALANCE - TRANSFER_AMOUNT - spent_amount;
    let expected_dest_balance = TRANSFER_AMOUNT;
    let expected_relayer_balance = spent_amount;
    let relayer_address =
        sdk::types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());

    // validate post-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        expected_source_balance,
        (INITIAL_NONCE + 1).into(),
    );
    test_utils::validate_address_balance_and_nonce(
        &runner,
        dest_address,
        expected_dest_balance,
        0.into(),
    );
    test_utils::validate_address_balance_and_nonce(
        &runner,
        relayer_address,
        expected_relayer_balance,
        0.into(),
    );
}

#[test]
fn test_eth_transfer_charging_gas_not_enough_balance() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);
    let transaction = |nonce| {
        let mut tx = test_utils::transfer(dest_address, TRANSFER_AMOUNT, nonce);
        // With this gas limit and price the account does not
        // have enough balance to cover the gas cost
        tx.gas = 3_000_000.into();
        tx.gas_price = GAS_PRICE.into();
        tx
    };

    // validate pre-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

    // validate post-state
    let relayer =
        sdk::types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());
    test_utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        // nonce is still incremented since the transaction was otherwise valid
        (INITIAL_NONCE + 1).into(),
    );
    test_utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into());
    test_utils::validate_address_balance_and_nonce(&runner, relayer, Wei::zero(), 0.into());
}

fn initialize_transfer() -> (test_utils::AuroraRunner, test_utils::Signer, Address) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));
    let mut signer = test_utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    (runner, signer, dest_address)
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

#[test]
fn test_block_hash() {
    let runner = test_utils::AuroraRunner::default();
    let chain_id = {
        let number = crate::prelude::U256::from(runner.chain_id);
        crate::prelude::u256_to_arr(&number)
    };
    let account_id = runner.aurora_account_id;
    let block_hash = crate::engine::Engine::compute_block_hash(chain_id, 10, account_id.as_bytes());

    assert_eq!(
        hex::encode(block_hash.0).as_str(),
        "c4a46f076b64877cbd8c5dbfd7bfbbea21a5653b79e3b6d06b6dfb5c88f1c384",
    );
}

#[test]
fn test_block_hash_contract() {
    let (mut runner, mut source_account, _) = initialize_transfer();
    let test_constructor = test_utils::solidity::ContractConstructor::compile_from_source(
        ["src", "tests", "res"].iter().collect::<PathBuf>(),
        Path::new("target").join("solidity_build"),
        "blockhash.sol",
        "BlockHash",
    );
    let nonce = source_account.use_nonce();
    let test_contract = runner.deploy_contract(
        &source_account.secret_key,
        |c| c.deploy_without_args(nonce.into()),
        test_constructor,
    );

    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            test_contract.call_method_without_args("test", nonce)
        })
        .unwrap();

    test_utils::panic_on_fail(result.status);
}

#[test]
fn test_ft_metadata() {
    let mut runner = test_utils::deploy_evm();

    let (maybe_outcome, maybe_error) = runner.call(
        "ft_metadata",
        runner.context.signer_account_id.clone(),
        Vec::new(),
    );
    assert!(maybe_error.is_none());
    let outcome = maybe_outcome.unwrap();
    let json_value = crate::json::parse_json(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(
        json_value,
        crate::json::JsonValue::from(FungibleTokenMetadata::default())
    );
}

// Same as `test_eth_transfer_insufficient_balance` above, except runs through
// `near-sdk-sim` instead of `near-vm-runner`. This is important because `near-sdk-sim`
// has more production logic, in particular, state revert on contract panic.
// TODO: should be able to generalize the `call` backend of `AuroraRunner` so that this
//       test does not need to be written twice.
#[test]
fn test_eth_transfer_insufficient_balance_sim() {
    let (aurora, mut signer, address) = initialize_evm_sim();

    // Run transaction which will fail (transfer more than current balance)
    let nonce = signer.use_nonce();
    let tx = test_utils::transfer(
        Address([1; 20]),
        INITIAL_BALANCE + INITIAL_BALANCE,
        nonce.into(),
    );
    let signed_tx = test_utils::sign_transaction(
        tx,
        Some(test_utils::AuroraRunner::default().chain_id),
        &signer.secret_key,
    );
    let call_result = aurora.call("submit", rlp::encode(&signed_tx).as_ref());
    let result: SubmitResult = call_result.unwrap_borsh();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

    // validate post-state
    assert_eq!(
        query_address_sim(&address, "get_nonce", &aurora),
        U256::from(INITIAL_NONCE + 1),
    );
    assert_eq!(
        query_address_sim(&address, "get_balance", &aurora),
        INITIAL_BALANCE.raw(),
    );
}

// Same as `test_eth_transfer_charging_gas_not_enough_balance` but run through `near-sdk-sim`.
#[test]
fn test_eth_transfer_charging_gas_not_enough_balance_sim() {
    let (aurora, mut signer, address) = initialize_evm_sim();

    // Run transaction which will fail (not enough balance to cover gas)
    let nonce = signer.use_nonce();
    let mut tx = test_utils::transfer(Address([1; 20]), TRANSFER_AMOUNT, nonce.into());
    tx.gas = 3_000_000.into();
    tx.gas_price = GAS_PRICE.into();
    let signed_tx = test_utils::sign_transaction(
        tx,
        Some(test_utils::AuroraRunner::default().chain_id),
        &signer.secret_key,
    );
    let call_result = aurora.call("submit", rlp::encode(&signed_tx).as_ref());
    let result: SubmitResult = call_result.unwrap_borsh();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

    // validate post-state
    assert_eq!(
        query_address_sim(&address, "get_nonce", &aurora),
        U256::from(INITIAL_NONCE + 1),
    );
    assert_eq!(
        query_address_sim(&address, "get_balance", &aurora),
        INITIAL_BALANCE.raw(),
    );
}

fn initialize_evm_sim() -> (state_migration::AuroraAccount, test_utils::Signer, Address) {
    let aurora = state_migration::deploy_evm();
    let signer = test_utils::Signer::random();
    let address = test_utils::address_from_secret_key(&signer.secret_key);

    let args = (address.0, INITIAL_NONCE, INITIAL_BALANCE.raw().low_u64());
    aurora
        .call("mint_account", &args.try_to_vec().unwrap())
        .assert_success();

    // validate pre-state
    assert_eq!(
        query_address_sim(&address, "get_nonce", &aurora),
        U256::from(INITIAL_NONCE),
    );
    assert_eq!(
        query_address_sim(&address, "get_balance", &aurora),
        INITIAL_BALANCE.raw(),
    );

    (aurora, signer, address)
}

fn query_address_sim(
    address: &Address,
    method: &str,
    aurora: &state_migration::AuroraAccount,
) -> U256 {
    let x = aurora.call(method, &address.0);
    match &x.outcome().status {
        near_sdk_sim::transaction::ExecutionStatus::SuccessValue(b) => U256::from_big_endian(&b),
        other => panic!("Unexpected outcome: {:?}", other),
    }
}
