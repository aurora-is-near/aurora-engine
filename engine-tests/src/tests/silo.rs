use aurora_engine::engine::EngineErrorKind;
use aurora_engine::silo::parameters::{
    FixedGasCostArgs, WhitelistAccountArgs, WhitelistAddressArgs, WhitelistArgs,
    WhitelistStatusArgs,
};
use aurora_engine::silo::WhitelistKind;
use aurora_engine_sdk as sdk;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::engine::TransactionStatus;
use borsh::BorshSerialize;
use libsecp256k1::SecretKey;
use rand::{rngs::ThreadRng, Rng, RngCore};
use std::fmt::Debug;

use crate::{
    prelude::{Address, Wei},
    test_utils::{self, validate_address_balance_and_nonce, AuroraRunner},
};

const INITIAL_BALANCE: Wei = Wei::new_u64(10u64.pow(18) * 10);
const ZERO_BALANCE: Wei = Wei::zero();
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(10u64.pow(18) * 4);
const FEE: Wei = Wei::new_u64(10u64.pow(18));
// https://github.com/aurora-is-near/aurora-engine/blob/master/engine-tests/src/test_utils/mod.rs#L393
const CALLER_ACCOUNT_ID: &str = "some-account.near";

#[test]
fn test_address_transfer_success() {
    // set up Aurora runner and accounts
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_fixed_gas_cost(&mut runner, Some(FEE));

    // Allow to submit transactions
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // perform transfer
    runner
        .submit_with_signer(&mut source_account, |nonce| {
            test_utils::transfer(receiver, TRANSFER_AMOUNT, nonce)
        })
        .unwrap();

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FEE - TRANSFER_AMOUNT,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());
}

#[test]
fn test_transfer_insufficient_balance() {
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_fixed_gas_cost(&mut runner, Some(FEE));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            test_utils::transfer(receiver, INITIAL_BALANCE + INITIAL_BALANCE, nonce)
        })
        .unwrap();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FEE,
        // the nonce is still incremented even though the transfer failed
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());
}

#[test]
fn test_transfer_insufficient_balance_fee() {
    const HALF_FEE: Wei = Wei::new_u64(10u64.pow(18) / 2);

    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_fixed_gas_cost(&mut runner, Some(FEE));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            test_utils::transfer(
                receiver,
                // We want to leave TRANSFER_AMOUNT + HALF_FEE on the balance.
                INITIAL_BALANCE - TRANSFER_AMOUNT - FEE - HALF_FEE,
                nonce,
            )
        })
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        TRANSFER_AMOUNT + HALF_FEE,
        // the nonce is still incremented even though the transfer failed
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(
        &runner,
        receiver,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE - HALF_FEE,
        INITIAL_NONCE.into(),
    );

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            test_utils::transfer(receiver, TRANSFER_AMOUNT, nonce)
        })
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::OutOfFund));
}

#[test]
fn test_eth_transfer_incorrect_nonce() {
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_fixed_gas_cost(&mut runner, Some(FEE));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // attempt transfer
    let err = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // creating transaction with incorrect nonce
            test_utils::transfer(receiver, TRANSFER_AMOUNT, nonce + 1)
        })
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::IncorrectNonce);

    // validate post-state (which is the same as pre-state in this case)
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());
}

#[test]
fn test_transfer_with_low_gas_limit() {
    let (mut runner, mut signer, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_fixed_gas_cost(&mut runner, Some(FEE));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    let transaction = |nonce| {
        let mut tx = test_utils::transfer(receiver, TRANSFER_AMOUNT, nonce);
        // it's not enough gas for common tx, but it doesn't matter if fixed cost is set
        tx.gas_limit = 10_000.into();
        tx
    };

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // make transfer
    let result = runner.submit_with_signer(&mut signer, transaction).unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FEE - TRANSFER_AMOUNT,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());
}

#[test]
fn test_relayer_balance_after_transfer() {
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = |nonce| test_utils::transfer(receiver, TRANSFER_AMOUNT, nonce);

    set_fixed_gas_cost(&mut runner, Some(FEE));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // do transfer
    runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap();

    let relayer = sdk::types::near_account_to_evm_address(
        runner.context.predecessor_account_id.as_ref().as_bytes(),
    );

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, relayer, FEE, INITIAL_NONCE.into());
}

#[test]
fn test_admin_access_right() {
    let (mut runner, signer, _) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_fixed_gas_cost(&mut runner, Some(FEE));
    // Allow to submit transactions.

    let account = WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        account_id: caller.clone(),
        kind: WhitelistKind::Account,
    })
    .try_to_vec()
    .unwrap();
    let address = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
        address: sender,
        kind: WhitelistKind::Address,
    })
    .try_to_vec()
    .unwrap();

    let err = runner
        .call("add_entry_to_whitelist", caller.as_ref(), account.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);
    let err = runner
        .call("add_entry_to_whitelist", caller.as_ref(), address.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    add_admin(&mut runner, caller.clone());

    let result = runner.call("add_entry_to_whitelist", caller.as_ref(), account);
    assert!(result.is_ok());
    let result = runner.call("add_entry_to_whitelist", caller.as_ref(), address);
    assert!(result.is_ok());
}

#[test]
fn test_submit_access_right() {
    let (mut runner, signer, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = test_utils::transfer(receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());

    set_fixed_gas_cost(&mut runner, Some(FEE));

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Allow to submit transactions.

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Add caller and signer to whitelists.
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // perform transfer
    let result = runner
        .submit_transaction(&signer.secret_key, transaction)
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());
}

#[test]
fn test_submit_access_right_via_batch() {
    let (mut runner, signer, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = test_utils::transfer(receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());

    set_fixed_gas_cost(&mut runner, Some(FEE));

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Allow to submit transactions.

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Add caller and signer to whitelists via batch.
    let args = vec![
        WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
            kind: WhitelistKind::Account,
            account_id: caller,
        }),
        WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
            kind: WhitelistKind::Address,
            address: sender,
        }),
    ];

    call_function(&mut runner, "add_entry_to_whitelist_batch", args);

    // perform transfer
    let result = runner
        .submit_transaction(&signer.secret_key, transaction)
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());
}

#[test]
fn test_submit_with_disabled_whitelist() {
    let (mut runner, signer, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let transaction = test_utils::transfer(receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());

    set_fixed_gas_cost(&mut runner, Some(FEE));

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Allow to submit transactions.

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Disable whitelists.
    disable_whitelist(&mut runner, WhitelistKind::Account);
    disable_whitelist(&mut runner, WhitelistKind::Address);

    // perform transfer
    let result = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());

    // Enable whitelist.
    enable_whitelist(&mut runner, WhitelistKind::Account);
    enable_whitelist(&mut runner, WhitelistKind::Address);

    let err = runner
        .submit_transaction(&signer.secret_key, transaction)
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);
}

#[test]
fn test_submit_with_removing_entries() {
    let (mut runner, signer, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = test_utils::transfer(receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());

    set_fixed_gas_cost(&mut runner, Some(FEE));

    // Allow to submit transactions.
    add_account_to_whitelist(&mut runner, caller.clone());
    add_address_to_whitelist(&mut runner, sender);

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // perform transfer
    let result = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());

    // Remove account id and address from white lists.
    remove_account_from_whitelist(&mut runner, caller);
    remove_address_from_whitelist(&mut runner, sender);

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction)
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FEE,
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into());
}

#[test]
fn test_deploy_access_rights() {
    let (mut runner, signer, _) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let code: Vec<u8> = {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(512..=1024);
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    };
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let deploy_tx = test_utils::create_deploy_transaction(code.clone(), INITIAL_NONCE.into());
    // Check that caller's balance is enough.
    let balance = runner.get_balance(sender);
    assert_eq!(balance, INITIAL_BALANCE);

    set_fixed_gas_cost(&mut runner, Some(FEE));

    // Try to deploy code without adding to admins white list.
    let err = runner
        .submit_transaction(&signer.secret_key, deploy_tx.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());

    // Add caller's account id and sender address to admins list to allow deploying a code.
    add_admin(&mut runner, caller);
    add_evm_admin(&mut runner, sender);

    // Deploy that code
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let address = Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();

    // Confirm the code stored at that address is equal to the input code.
    let stored_code = runner.get_code(address);
    assert_eq!(code, stored_code);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FEE,
        (INITIAL_NONCE + 1).into(),
    );
}

#[test]
fn test_deploy_with_disabled_whitelist() {
    let (mut runner, signer, _) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let code: Vec<u8> = {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(512..=1024);
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    };
    let deploy_tx = test_utils::create_deploy_transaction(code.clone(), INITIAL_NONCE.into());
    // Check that caller's balance is enough.
    let balance = runner.get_balance(sender);
    assert_eq!(balance, INITIAL_BALANCE);

    set_fixed_gas_cost(&mut runner, Some(FEE));

    // Try to deploy code without adding to admins white list.
    let err = runner
        .submit_transaction(&signer.secret_key, deploy_tx.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());

    // Disable whitelists.
    disable_whitelist(&mut runner, WhitelistKind::Admin);
    disable_whitelist(&mut runner, WhitelistKind::EvmAdmin);

    // Deploy that code
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let address = Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();

    // Confirm the code stored at that address is equal to the input code.
    let stored_code = runner.get_code(address);
    assert_eq!(code, stored_code);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FEE,
        (INITIAL_NONCE + 1).into(),
    );
}

#[test]
fn test_switch_between_fix_gas_cost() {
    const TRANSFER: Wei = Wei::new_u64(10_000_000);
    let (mut runner, mut signer, receiver) = initialize_transfer();
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into());
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into());

    // Defining gas cost in transaction
    // do transfer
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            let mut tx = test_utils::transfer(receiver, TRANSFER, nonce);
            tx.gas_limit = 30_0000.into();
            tx.gas_price = 1.into();
            tx
        })
        .unwrap();

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER - Wei::new_u64(result.gas_used),
        (INITIAL_NONCE + 1).into(),
    );
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER, 0.into());

    // Set fixed gas cost
    let fixed_gas_cost = Wei::new_u64(1_000_000);
    set_fixed_gas_cost(&mut runner, Some(fixed_gas_cost));
    // Check that fixed gas cost has been set successfully.
    assert_eq!(runner.get_fixed_gas_cost(), Some(fixed_gas_cost));

    let balance_before_transfer = runner.get_balance(sender);
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            test_utils::transfer(receiver, TRANSFER, nonce)
        })
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    let sender_balance = balance_before_transfer - TRANSFER - fixed_gas_cost;
    let receiver_balance = TRANSFER + TRANSFER;

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, sender_balance, (INITIAL_NONCE + 2).into());
    validate_address_balance_and_nonce(&runner, receiver, receiver_balance, INITIAL_NONCE.into());

    // Unset fixed gas cost. Should be used usual gas charge mechanism.
    set_fixed_gas_cost(&mut runner, None);
    assert_eq!(runner.get_fixed_gas_cost(), None);
    let balance_before_transfer = runner.get_balance(sender);

    // do transfer
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            let mut tx = test_utils::transfer(receiver, TRANSFER, nonce);
            tx.gas_limit = 30_0000.into();
            tx.gas_price = 1.into();
            tx
        })
        .unwrap();

    let sender_balance = balance_before_transfer - TRANSFER - Wei::new_u64(result.gas_used);
    let receiver_balance = TRANSFER + TRANSFER + TRANSFER;

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, sender_balance, (INITIAL_NONCE + 3).into());
    validate_address_balance_and_nonce(&runner, receiver, receiver_balance, INITIAL_NONCE.into());
}

fn initialize_transfer() -> (AuroraRunner, test_utils::Signer, Address) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let (source_address, source_account) = keys(&mut rng);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let (dest_address, _) = keys(&mut rng);
    let mut signer = test_utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    (runner, signer, dest_address)
}

fn keys(rng: &mut ThreadRng) -> (Address, SecretKey) {
    let sk = SecretKey::random(rng);
    let address = test_utils::address_from_secret_key(&sk);
    (address, sk)
}

fn add_admin(runner: &mut AuroraRunner, account_id: AccountId) {
    let args = WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        kind: WhitelistKind::Admin,
        account_id,
    });
    call_function(runner, "add_entry_to_whitelist", args);
}

fn add_evm_admin(runner: &mut AuroraRunner, address: Address) {
    let args = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
        kind: WhitelistKind::EvmAdmin,
        address,
    });
    call_function(runner, "add_entry_to_whitelist", args);
}

#[allow(dead_code)]
fn enable_whitelist(runner: &mut AuroraRunner, kind: WhitelistKind) {
    let args = WhitelistStatusArgs { kind, active: true };
    call_function(runner, "set_whitelist_status", args);
}

#[allow(dead_code)]
fn disable_whitelist(runner: &mut AuroraRunner, kind: WhitelistKind) {
    let args = WhitelistStatusArgs {
        kind,
        active: false,
    };
    call_function(runner, "set_whitelist_status", args);
}

fn add_account_to_whitelist(runner: &mut AuroraRunner, account_id: AccountId) {
    let args = WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        kind: WhitelistKind::Account,
        account_id,
    });
    call_function(runner, "add_entry_to_whitelist", args);
}

fn add_address_to_whitelist(runner: &mut AuroraRunner, address: Address) {
    let args = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
        kind: WhitelistKind::Address,
        address,
    });
    call_function(runner, "add_entry_to_whitelist", args);
}

fn remove_account_from_whitelist(runner: &mut AuroraRunner, account_id: AccountId) {
    let args = WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        kind: WhitelistKind::Account,
        account_id,
    });
    call_function(runner, "remove_entry_from_whitelist", args);
}

fn remove_address_from_whitelist(runner: &mut AuroraRunner, address: Address) {
    let args = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
        kind: WhitelistKind::Address,
        address,
    });
    call_function(runner, "remove_entry_from_whitelist", args);
}

fn set_fixed_gas_cost(runner: &mut AuroraRunner, cost: Option<Wei>) {
    let args = FixedGasCostArgs { cost };
    call_function(runner, "set_fixed_gas_cost", args);
}

fn call_function<T: BorshSerialize + Debug>(runner: &mut AuroraRunner, func: &str, args: T) {
    let input = args.try_to_vec().unwrap();
    let result = runner.call(func, &runner.aurora_account_id.clone(), input);
    assert!(
        result.is_ok(),
        "{}: {:?}, args: {:?}",
        func,
        result.unwrap_err(),
        args
    );
}

pub mod sim_tests {
    use super::FEE;
    use crate::test_utils::erc20::ERC20;
    use crate::tests::erc20_connector::sim_tests::{
        self, deploy_nep_141, erc20_balance, exit_to_near, nep_141_balance_of,
    };
    use crate::tests::state_migration::{deploy_evm, AuroraAccount};
    use aurora_engine::silo::parameters::{SiloParamsArgs, WhitelistAddressArgs, WhitelistArgs};
    use aurora_engine::silo::WhitelistKind;
    use aurora_engine_types::types::Address;
    use borsh::BorshSerialize;
    use near_sdk_sim::UserAccount;
    use serde_json::json;

    const FT_ACCOUNT: &str = "test_token.root";
    const FT_TOTAL_SUPPLY: u128 = 1_000_000;

    #[test]
    fn test_transfer_nep141_to_non_whitelisted_address() {
        let SiloTestContext {
            aurora,
            fallback_account,
            fallback_address,
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
        } = init_silo();

        let ft_transfer_amount = 300_000;

        // Transfer tokens from `ft_owner` to non-whitelisted address `ft_owner_address`
        transfer_nep_141_to_erc_20(
            &nep_141,
            &ft_owner,
            ft_owner_address,
            ft_transfer_amount,
            &aurora,
        );

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY - ft_transfer_amount
        );
        assert_eq!(
            nep_141_balance_of(fallback_account.account_id.as_str(), &nep_141, &aurora),
            0
        );
        assert_eq!(erc20_balance(&erc20, ft_owner_address, &aurora), 0.into());
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora),
            ft_transfer_amount.into()
        );

        // Transfer tokens from fallback address to fallback near account
        exit_to_near(
            &fallback_account,
            fallback_account.account_id.as_str(),
            ft_transfer_amount,
            &erc20,
            &aurora,
        );

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY - ft_transfer_amount
        );
        assert_eq!(
            nep_141_balance_of(fallback_account.account_id.as_str(), &nep_141, &aurora),
            ft_transfer_amount
        );
        assert_eq!(erc20_balance(&erc20, ft_owner_address, &aurora), 0.into());
        assert_eq!(erc20_balance(&erc20, fallback_address, &aurora), 0.into());
    }

    #[test]
    fn test_transfer_nep141_to_whitelisted_address() {
        let SiloTestContext {
            aurora,
            fallback_account,
            fallback_address,
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
        } = init_silo();

        add_address_to_whitelist(&aurora, ft_owner_address);

        let ft_transfer_amount = 300_000;

        // Transfer tokens from `ft_owner` to whitelisted address `ft_owner_address`
        transfer_nep_141_to_erc_20(
            &nep_141,
            &ft_owner,
            ft_owner_address,
            ft_transfer_amount,
            &aurora,
        );

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY - ft_transfer_amount
        );
        assert_eq!(
            nep_141_balance_of(fallback_account.account_id.as_str(), &nep_141, &aurora),
            0
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            ft_transfer_amount.into()
        );
        assert_eq!(erc20_balance(&erc20, fallback_address, &aurora), 0.into());

        // Transfer tokens from ft_owner evm address to ft_owner near account
        exit_to_near(
            &ft_owner,
            ft_owner.account_id.as_str(),
            ft_transfer_amount,
            &erc20,
            &aurora,
        );

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY
        );
        assert_eq!(
            nep_141_balance_of(fallback_account.account_id.as_str(), &nep_141, &aurora),
            0
        );
        assert_eq!(erc20_balance(&erc20, ft_owner_address, &aurora), 0.into());
        assert_eq!(erc20_balance(&erc20, fallback_address, &aurora), 0.into());
    }

    struct SiloTestContext {
        pub aurora: AuroraAccount,
        pub fallback_account: UserAccount,
        pub fallback_address: Address,
        pub ft_owner: UserAccount,
        pub ft_owner_address: Address,
        pub nep_141: UserAccount,
        pub erc20: ERC20,
    }

    fn add_address_to_whitelist(aurora: &AuroraAccount, address: Address) {
        let args = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
            kind: WhitelistKind::Address,
            address,
        });
        aurora
            .user
            .call(
                aurora.contract.account_id(),
                "add_entry_to_whitelist",
                &args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    pub fn transfer_nep_141_to_erc_20(
        nep_141: &UserAccount,
        source: &UserAccount,
        dest: Address,
        amount: u128,
        aurora: &AuroraAccount,
    ) {
        let transfer_args = json!({
            "receiver_id": aurora.contract.account_id.as_str(),
            "amount": format!("{amount}"),
            "msg": dest.encode(),
        });
        source
            .call(
                nep_141.account_id(),
                "ft_transfer_call",
                transfer_args.to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                1,
            )
            .assert_success();
    }

    /// Deploys the EVM, deploys nep141 contract, and calls `set_silo_params`
    fn init_silo() -> SiloTestContext {
        // Deploy Aurora
        let aurora: AuroraAccount = deploy_evm();

        // Create fallback account and evm address
        let fallback_account = aurora.user.create_user(
            "fallback.root".parse().unwrap(),
            near_sdk_sim::STORAGE_AMOUNT,
        );
        let fallback_address = aurora_engine_sdk::types::near_account_to_evm_address(
            fallback_account.account_id.as_bytes(),
        );

        // Set silo mode
        let args = Some(SiloParamsArgs {
            fixed_gas_cost: FEE,
            erc20_fallback_address: fallback_address,
        });

        aurora
            .user
            .call(
                aurora.contract.account_id(),
                "set_silo_params",
                &args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();

        // Create `ft_owner` account and evm address
        let ft_owner = aurora.user.create_user(
            "ft_owner.root".parse().unwrap(),
            near_sdk_sim::STORAGE_AMOUNT,
        );
        let ft_owner_address =
            aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.account_id.as_bytes());

        // Deploy nep141 token
        let nep_141 = deploy_nep_141(
            FT_ACCOUNT,
            ft_owner.account_id.as_ref(),
            FT_TOTAL_SUPPLY,
            &aurora,
        );

        // Call storage deposit for fallback account
        aurora
            .user
            .call(
                nep_141.account_id(),
                "storage_deposit",
                json!({
                    "account_id": fallback_account.account_id.as_str(),
                })
                .to_string()
                .as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                near_sdk_sim::STORAGE_AMOUNT,
            )
            .assert_success();

        // Deploy erc20 token
        let erc20 = sim_tests::deploy_erc20_from_nep_141(&nep_141, &aurora);

        // Verify tokens balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY
        );
        assert_eq!(
            nep_141_balance_of(fallback_account.account_id.as_str(), &nep_141, &aurora),
            0
        );
        assert_eq!(erc20_balance(&erc20, ft_owner_address, &aurora), 0.into());
        assert_eq!(erc20_balance(&erc20, fallback_address, &aurora), 0.into());

        SiloTestContext {
            aurora,
            fallback_account,
            fallback_address,
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
        }
    }
}
