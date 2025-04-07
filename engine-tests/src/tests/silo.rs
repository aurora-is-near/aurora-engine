use aurora_engine::engine::EngineErrorKind;
use aurora_engine_sdk as sdk;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh::BorshSerialize;
use aurora_engine_types::parameters::engine::TransactionStatus;
use aurora_engine_types::parameters::silo::{
    Erc20FallbackAddressArgs, FixedGasArgs, SiloParamsArgs, WhitelistAccountArgs,
    WhitelistAddressArgs, WhitelistArgs, WhitelistKind, WhitelistStatusArgs,
};
use aurora_engine_types::types::EthGas;
use libsecp256k1::SecretKey;
use rand::{rngs::ThreadRng, Rng, RngCore};
use std::fmt::Debug;

use crate::{
    prelude::{Address, Wei},
    utils::{self, validate_address_balance_and_nonce, AuroraRunner},
};

const INITIAL_BALANCE: Wei = Wei::new_u64(10u64.pow(18) * 10);
const ZERO_BALANCE: Wei = Wei::zero();
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(10u64.pow(18) * 4);
const FIXED_GAS: EthGas = EthGas::new(10u64.pow(18));
const ONE_GAS_PRICE: Wei = Wei::new_u64(1);
const TWO_GAS_PRICE: Wei = Wei::new_u64(2);

const ERC20_FALLBACK_ADDRESS: Address = Address::zero();
const SILO_PARAMS_ARGS: SiloParamsArgs = SiloParamsArgs {
    fixed_gas: FIXED_GAS,
    erc20_fallback_address: ERC20_FALLBACK_ADDRESS,
};
// https://github.com/aurora-is-near/aurora-engine/blob/master/engine-tests/src/test_utils/mod.rs#L393
const CALLER_ACCOUNT_ID: &str = "some-account.near";

#[test]
fn test_address_transfer_success() {
    // set up Aurora runner and accounts
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));

    // Allow to submit transactions
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // perform transfer
    runner
        .submit_with_signer(&mut source_account, |nonce| {
            utils::transfer_with_price(receiver, TRANSFER_AMOUNT, nonce, TWO_GAS_PRICE.raw())
        })
        .unwrap();

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FIXED_GAS * TWO_GAS_PRICE - TRANSFER_AMOUNT,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_transfer_insufficient_balance() {
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            utils::transfer_with_price(
                receiver,
                INITIAL_BALANCE + INITIAL_BALANCE,
                nonce,
                ONE_GAS_PRICE.raw(),
            )
        })
        .unwrap();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FIXED_GAS * ONE_GAS_PRICE,
        // the nonce is still incremented even though the transfer failed
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_transfer_insufficient_balance_fee() {
    const HALF_FIXED_GAS: EthGas = EthGas::new(10u64.pow(18) / 2);

    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // We want to leave TRANSFER_AMOUNT + HALF_FIXED_GAS on the balance.
    let amount = INITIAL_BALANCE
        - FIXED_GAS * ONE_GAS_PRICE
        - TRANSFER_AMOUNT
        - HALF_FIXED_GAS * ONE_GAS_PRICE;
    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            utils::transfer_with_price(receiver, amount, nonce, ONE_GAS_PRICE.raw())
        })
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        TRANSFER_AMOUNT + HALF_FIXED_GAS * ONE_GAS_PRICE,
        // the nonce is still incremented even though the transfer failed
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, amount, INITIAL_NONCE.into()).unwrap();

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            utils::transfer_with_price(receiver, TRANSFER_AMOUNT, nonce, ONE_GAS_PRICE.raw())
        })
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::OutOfFund));
}

#[test]
fn test_eth_transfer_incorrect_nonce() {
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // attempt transfer
    let err = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // creating transaction with incorrect nonce
            utils::transfer(receiver, TRANSFER_AMOUNT, nonce + 1)
        })
        .unwrap_err();
    assert!(
        matches!(err.kind, EngineErrorKind::IncorrectNonce(msg) if &msg == "ERR_INCORRECT_NONCE: ac: 0, tx: 1")
    );

    // validate post-state (which is the same as pre-state in this case)
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_transfer_with_low_gas_limit() {
    let (mut runner, mut signer, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    let transaction = |nonce| {
        let mut tx = utils::transfer(receiver, TRANSFER_AMOUNT, nonce);
        tx.gas_limit = 10_000.into();
        tx.gas_price = ONE_GAS_PRICE.raw();
        tx
    };

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // make transfer. should be error FixedGasOverflow because too low gas_limit.
    let error = runner
        .submit_with_signer(&mut signer, transaction)
        .err()
        .unwrap();
    assert!(matches!(error.kind, EngineErrorKind::FixedGasOverflow));

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_relayer_balance_after_transfer() {
    let (mut runner, mut source_account, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&source_account.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction =
        |nonce| utils::transfer_with_price(receiver, TRANSFER_AMOUNT, nonce, ONE_GAS_PRICE.raw());

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // do transfer
    runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap();

    let relayer =
        sdk::types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(
        &runner,
        relayer,
        FIXED_GAS * ONE_GAS_PRICE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
}

#[test]
fn test_admin_access_right() {
    let (mut runner, signer, _) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_whitelist(&mut runner, WhitelistKind::Admin);

    // Allow to submit transactions.

    let account = borsh::to_vec(&WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        account_id: caller.clone(),
        kind: WhitelistKind::Account,
    }))
    .unwrap();
    let address = borsh::to_vec(&WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
        address: sender,
        kind: WhitelistKind::Address,
    }))
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
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = utils::transfer_with_price(
        receiver,
        TRANSFER_AMOUNT,
        INITIAL_NONCE.into(),
        ONE_GAS_PRICE.raw(),
    );

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_all_whitelists(&mut runner);

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // Allow to submit transactions.

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

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
        INITIAL_BALANCE - TRANSFER_AMOUNT - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_submit_access_right_via_batch() {
    let (mut runner, signer, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = utils::transfer_with_price(
        receiver,
        TRANSFER_AMOUNT,
        INITIAL_NONCE.into(),
        ONE_GAS_PRICE.raw(),
    );

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_all_whitelists(&mut runner);

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // Allow to submit transactions.

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

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
        INITIAL_BALANCE - TRANSFER_AMOUNT - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_submit_with_disabled_whitelist() {
    let (mut runner, signer, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let transaction = utils::transfer_with_price(
        receiver,
        TRANSFER_AMOUNT,
        INITIAL_NONCE.into(),
        ONE_GAS_PRICE.raw(),
    );

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_all_whitelists(&mut runner);

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // Allow to submit transactions.

    // perform transfer
    let err = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

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
        INITIAL_BALANCE - TRANSFER_AMOUNT - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();

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
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let transaction = utils::transfer_with_price(
        receiver,
        TRANSFER_AMOUNT,
        INITIAL_NONCE.into(),
        ONE_GAS_PRICE.raw(),
    );

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_all_whitelists(&mut runner);

    // Allow to submit transactions.
    add_account_to_whitelist(&mut runner, caller.clone());
    add_address_to_whitelist(&mut runner, sender);

    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // perform transfer
    let result = runner
        .submit_transaction(&signer.secret_key, transaction.clone())
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER_AMOUNT - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();

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
        INITIAL_BALANCE - TRANSFER_AMOUNT - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER_AMOUNT, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_deploy_access_rights() {
    let (mut runner, signer, _) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let code: Vec<u8> = {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(512..=1024);
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    };
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();
    let deploy_tx = utils::create_deploy_transaction_with_price(
        code.clone(),
        INITIAL_NONCE.into(),
        ONE_GAS_PRICE.raw(),
    );
    // Check that caller's balance is enough.
    let balance = runner.get_balance(sender);
    assert_eq!(balance, INITIAL_BALANCE);

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_all_whitelists(&mut runner);

    // Try to deploy code without adding to admins white list.
    let err = runner
        .submit_transaction(&signer.secret_key, deploy_tx.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // Add caller's account id and sender address to admins list to allow deploying a code.
    add_admin(&mut runner, caller);
    add_evm_admin(&mut runner, sender);

    // Deploy that code
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let address = Address::try_from_slice(utils::unwrap_success_slice(&result)).unwrap();

    // Confirm the code stored at that address is equal to the input code.
    let stored_code = runner.get_code(address);
    assert_eq!(code, stored_code);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
}

#[test]
fn test_deploy_with_disabled_whitelist() {
    let (mut runner, signer, _) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let code: Vec<u8> = {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(512..=1024);
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    };
    let deploy_tx = utils::create_deploy_transaction_with_price(
        code.clone(),
        INITIAL_NONCE.into(),
        ONE_GAS_PRICE.raw(),
    );
    // Check that caller's balance is enough.
    let balance = runner.get_balance(sender);
    assert_eq!(balance, INITIAL_BALANCE);

    set_silo_params(&mut runner, Some(SILO_PARAMS_ARGS));
    enable_all_whitelists(&mut runner);

    // Try to deploy code without adding to admins white list.
    let err = runner
        .submit_transaction(&signer.secret_key, deploy_tx.clone())
        .unwrap_err();
    assert_eq!(err.kind, EngineErrorKind::NotAllowed);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // Disable whitelists.
    disable_whitelist(&mut runner, WhitelistKind::Admin);
    disable_whitelist(&mut runner, WhitelistKind::EvmAdmin);

    // Deploy that code
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let address = Address::try_from_slice(utils::unwrap_success_slice(&result)).unwrap();

    // Confirm the code stored at that address is equal to the input code.
    let stored_code = runner.get_code(address);
    assert_eq!(code, stored_code);

    // Check that the balance and the nonce haven't been changed.
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - FIXED_GAS * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
}

#[test]
fn test_switch_between_fix_gas() {
    const TRANSFER: Wei = Wei::new_u64(10_000_000);
    let (mut runner, mut signer, receiver) = initialize_transfer();
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let caller: AccountId = CALLER_ACCOUNT_ID.parse().unwrap();

    add_account_to_whitelist(&mut runner, caller);
    add_address_to_whitelist(&mut runner, sender);

    // validate pre-state
    validate_address_balance_and_nonce(&runner, sender, INITIAL_BALANCE, INITIAL_NONCE.into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, ZERO_BALANCE, INITIAL_NONCE.into())
        .unwrap();

    // Defining gas cost in transaction
    // do transfer
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            let mut tx = utils::transfer(receiver, TRANSFER, nonce);
            tx.gas_limit = 30_0000.into();
            tx.gas_price = 1.into();
            tx
        })
        .unwrap();

    // validate post-state
    validate_address_balance_and_nonce(
        &runner,
        sender,
        INITIAL_BALANCE - TRANSFER - EthGas::new(result.gas_used) * ONE_GAS_PRICE,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, TRANSFER, 0.into()).unwrap();

    // Set fixed gas
    let fixed_gas = EthGas::new(1_000_000);
    set_silo_params(
        &mut runner,
        Some(SiloParamsArgs {
            fixed_gas,
            erc20_fallback_address: ERC20_FALLBACK_ADDRESS,
        }),
    );
    // Check that fixed gas cost has been set successfully.
    assert_eq!(runner.get_fixed_gas(), Some(fixed_gas));

    let balance_before_transfer = runner.get_balance(sender);
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            utils::transfer_with_price(receiver, TRANSFER, nonce, TWO_GAS_PRICE.raw())
        })
        .unwrap();
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));

    let sender_balance = balance_before_transfer - TRANSFER - fixed_gas * TWO_GAS_PRICE;
    let receiver_balance = TRANSFER + TRANSFER;

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, sender_balance, (INITIAL_NONCE + 2).into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, receiver_balance, INITIAL_NONCE.into())
        .unwrap();

    // Unset fixed gas cost. Should be used usual gas charge mechanism.
    set_silo_params(&mut runner, None);
    assert_eq!(runner.get_fixed_gas(), None);
    let balance_before_transfer = runner.get_balance(sender);

    // do transfer
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            let mut tx = utils::transfer(receiver, TRANSFER, nonce);
            tx.gas_limit = 30_0000.into();
            tx.gas_price = 1.into();
            tx
        })
        .unwrap();

    let sender_balance = balance_before_transfer - TRANSFER - Wei::new_u64(result.gas_used);
    let receiver_balance = TRANSFER + TRANSFER + TRANSFER;

    // validate post-state
    validate_address_balance_and_nonce(&runner, sender, sender_balance, (INITIAL_NONCE + 3).into())
        .unwrap();
    validate_address_balance_and_nonce(&runner, receiver, receiver_balance, INITIAL_NONCE.into())
        .unwrap();
}

#[test]
fn test_set_erc20_fallback_address() {
    let mut runner = utils::deploy_runner();
    set_erc20_fallback_address(&mut runner, Some(Address::from_array([1; 20])));
}

#[test]
fn test_set_fixed_gas() {
    let mut runner = utils::deploy_runner();
    set_fixed_gas(&mut runner, Some(FIXED_GAS));
}

fn initialize_transfer() -> (AuroraRunner, utils::Signer, Address) {
    // set up Aurora runner and accounts
    let mut runner = utils::deploy_runner();
    let mut rng = rand::thread_rng();
    let (source_address, source_account) = keys(&mut rng);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let (dest_address, _) = keys(&mut rng);
    let mut signer = utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    (runner, signer, dest_address)
}

fn keys(rng: &mut ThreadRng) -> (Address, SecretKey) {
    let sk = SecretKey::random(rng);
    let address = utils::address_from_secret_key(&sk);
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
fn enable_all_whitelists(runner: &mut AuroraRunner) {
    let args = vec![
        WhitelistStatusArgs {
            kind: WhitelistKind::Admin,
            active: true,
        },
        WhitelistStatusArgs {
            kind: WhitelistKind::EvmAdmin,
            active: true,
        },
        WhitelistStatusArgs {
            kind: WhitelistKind::Account,
            active: true,
        },
        WhitelistStatusArgs {
            kind: WhitelistKind::Address,
            active: true,
        },
    ];
    call_function(runner, "set_whitelists_statuses", args);
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

fn set_fixed_gas(runner: &mut AuroraRunner, fixed_gas: Option<EthGas>) {
    let args = FixedGasArgs { fixed_gas };
    call_function(runner, "set_fixed_gas", args);
}

fn set_erc20_fallback_address(runner: &mut AuroraRunner, address: Option<Address>) {
    let args = Erc20FallbackAddressArgs { address };
    call_function(runner, "set_erc20_fallback_address", args);
}

fn set_silo_params(runner: &mut AuroraRunner, silo_params: Option<SiloParamsArgs>) {
    call_function(runner, "set_silo_params", silo_params);
}

fn call_function<T: BorshSerialize + Debug>(runner: &mut AuroraRunner, func: &str, args: T) {
    let input = borsh::to_vec(&args).unwrap();
    let result = runner.call(func, &runner.aurora_account_id.clone(), input);
    assert!(
        result.is_ok(),
        "{}: {:?}, args: {:?}",
        func,
        result.unwrap_err(),
        args
    );
}

pub mod workspace {
    use super::FIXED_GAS;
    use crate::tests::erc20_connector::workspace::{erc20_balance, exit_to_near};
    use crate::utils::solidity::erc20::ERC20;
    use crate::utils::workspace::{
        deploy_engine, deploy_erc20_from_nep_141, deploy_nep_141, nep_141_balance_of,
    };
    use aurora_engine_sdk::types::near_account_to_evm_address;
    use aurora_engine_types::parameters::silo::{
        Erc20FallbackAddressArgs, SiloParamsArgs, WhitelistAddressArgs, WhitelistArgs,
        WhitelistKind, WhitelistStatusArgs,
    };
    use aurora_engine_types::types::Address;
    use aurora_engine_workspace::types::NearToken;
    use aurora_engine_workspace::{account::Account, EngineContract, RawContract};

    const FT_ACCOUNT: &str = "test_token";
    const FT_TOTAL_SUPPLY: u128 = 1_000_000;
    const FT_TRANSFER_AMOUNT: u128 = 300_000;

    #[tokio::test]
    async fn test_transfer_nep141_to_non_whitelisted_address() {
        let SiloTestContext {
            aurora,
            fallback_account,
            fallback_address,
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
        } = init_silo().await;

        // Transfer tokens from `ft_owner` to non-whitelisted address `ft_owner_address`
        transfer_nep_141_to_erc_20(
            &nep_141,
            &ft_owner,
            ft_owner_address,
            FT_TRANSFER_AMOUNT,
            &aurora,
        )
        .await;

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            0
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            0.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            FT_TRANSFER_AMOUNT.into()
        );

        // Transfer tokens from fallback address to fallback near account
        let result = exit_to_near(
            &fallback_account,
            fallback_account.id().as_ref(),
            FT_TRANSFER_AMOUNT,
            &erc20,
            &aurora,
        )
        .await;
        assert!(result.is_success());

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            0.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            0.into()
        );
    }

    #[tokio::test]
    async fn test_transfer_nep141_to_non_whitelisted_address_with_another_fallback_address() {
        let SiloTestContext {
            aurora,
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            ..
        } = init_silo().await;

        // Set another EVM fallback address
        let fallback_account = aurora
            .root()
            .create_subaccount("fallback2", NearToken::from_near(10))
            .await
            .unwrap();
        // Call storage deposit for fallback account
        let result = aurora
            .root()
            .call(&nep_141.id(), "storage_deposit")
            .args_json(serde_json::json!({
                "account_id": fallback_account.id(),
                "registration_only": None::<bool>
            }))
            .deposit(NearToken::from_near(50))
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());
        let fallback_address = near_account_to_evm_address(fallback_account.id().as_bytes());
        // Setting a new fallback address.
        let result = aurora
            .set_erc20_fallback_address(Erc20FallbackAddressArgs {
                address: Some(fallback_address),
            })
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // Transfer tokens from `ft_owner` to non-whitelisted address `ft_owner_address`
        transfer_nep_141_to_erc_20(
            &nep_141,
            &ft_owner,
            ft_owner_address,
            FT_TRANSFER_AMOUNT,
            &aurora,
        )
        .await;

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            0
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            0.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            FT_TRANSFER_AMOUNT.into()
        );

        // Transfer tokens from fallback address to fallback near account
        let result = exit_to_near(
            &fallback_account,
            fallback_account.id().as_ref(),
            FT_TRANSFER_AMOUNT,
            &erc20,
            &aurora,
        )
        .await;
        assert!(result.is_success());

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            0.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            0.into()
        );
    }

    #[tokio::test]
    async fn test_transfer_nep141_to_whitelisted_address() {
        let SiloTestContext {
            aurora,
            fallback_account,
            fallback_address,
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
        } = init_silo().await;

        add_address_to_whitelist(&aurora, ft_owner_address).await;

        // Transfer tokens from `ft_owner` to whitelisted address `ft_owner_address`
        transfer_nep_141_to_erc_20(
            &nep_141,
            &ft_owner,
            ft_owner_address,
            FT_TRANSFER_AMOUNT,
            &aurora,
        )
        .await;

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            0
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            FT_TRANSFER_AMOUNT.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            0.into()
        );

        // Transfer tokens from ft_owner evm address to ft_owner near account
        let result = exit_to_near(
            &ft_owner,
            ft_owner.id().as_ref(),
            FT_TRANSFER_AMOUNT,
            &erc20,
            &aurora,
        )
        .await;
        assert!(result.is_success());

        // Verify the nep141 and erc20 tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            0
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            0.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            0.into()
        );
    }

    struct SiloTestContext {
        pub aurora: EngineContract,
        pub fallback_account: Account,
        pub fallback_address: Address,
        pub ft_owner: Account,
        pub ft_owner_address: Address,
        pub nep_141: RawContract,
        pub erc20: ERC20,
    }

    async fn add_address_to_whitelist(aurora: &EngineContract, address: Address) {
        let entry = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
            kind: WhitelistKind::Address,
            address,
        });
        let result = aurora
            .add_entry_to_whitelist(entry)
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());
    }

    async fn transfer_nep_141_to_erc_20(
        nep_141: &RawContract,
        source: &Account,
        dest: Address,
        amount: u128,
        aurora: &EngineContract,
    ) {
        let transfer_args = serde_json::json!({
            "receiver_id": aurora.id(),
            "amount": format!("{amount}"),
            "msg": dest.encode(),
        });
        let result = source
            .call(&nep_141.id(), "ft_transfer_call")
            .args_json(transfer_args)
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");
    }

    /// Deploys the EVM, deploys nep141 contract, and calls `set_silo_params`
    async fn init_silo() -> SiloTestContext {
        // Deploy Aurora Engine
        let aurora = deploy_engine().await;
        // Create fallback account and evm address
        let fallback_account = aurora
            .root()
            .create_subaccount("fallback", NearToken::from_near(10))
            .await
            .unwrap();
        let fallback_address =
            aurora_engine_sdk::types::near_account_to_evm_address(fallback_account.id().as_bytes());

        // Set silo mode
        let params = Some(SiloParamsArgs {
            fixed_gas: FIXED_GAS,
            erc20_fallback_address: fallback_address,
        });

        let result = aurora.set_silo_params(params).transact().await.unwrap();
        assert!(result.is_success());

        // We have to enable the `Address` whitelist.
        let result = aurora
            .set_whitelist_status(WhitelistStatusArgs {
                kind: WhitelistKind::Address,
                active: true,
            })
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // Create `ft_owner` account and evm address
        let ft_owner = aurora
            .root()
            .create_subaccount("ft_owner", NearToken::from_near(10))
            .await
            .unwrap();
        let ft_owner_address =
            aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.id().as_bytes());

        let nep_141_account = aurora
            .root()
            .create_subaccount(FT_ACCOUNT, NearToken::from_near(10))
            .await
            .unwrap();

        // Deploy nep141 token
        let nep_141 = deploy_nep_141(&nep_141_account, &ft_owner, FT_TOTAL_SUPPLY, &aurora)
            .await
            .unwrap();

        // Call storage deposit for fallback account
        let result = aurora
            .root()
            .call(&nep_141.id(), "storage_deposit")
            .args_json(serde_json::json!({
                "account_id": fallback_account.id(),
                "registration_only": None::<bool>
            }))
            .deposit(NearToken::from_near(50))
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // Deploy erc20 token
        let erc20 = deploy_erc20_from_nep_141(nep_141_account.id().as_ref(), &aurora, None)
            .await
            .unwrap();

        // Verify tokens balances
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &fallback_account.id()).await,
            0
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            0.into()
        );
        assert_eq!(
            erc20_balance(&erc20, fallback_address, &aurora).await,
            0.into()
        );

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
