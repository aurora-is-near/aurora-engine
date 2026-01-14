use crate::prelude::Wei;
use crate::prelude::{Address, U256};
use crate::utils::{
    self, Signer,
    solidity::erc20::{self, ERC20, ERC20Constructor},
    str_to_account_id,
};
use aurora_engine::engine::EngineErrorKind;
use aurora_engine::parameters::TransactionStatus;
use aurora_engine_sdk as sdk;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::{
    Erc20Identifier, Erc20Metadata, SetErc20MetadataArgs,
};
use aurora_engine_types::parameters::engine::SetOwnerArgs;
use bstr::ByteSlice;
use std::str::FromStr;

const INITIAL_BALANCE: u64 = 1_000_000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 67;

#[test]
fn erc20_mint() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );

    // Do mint transaction
    let mint_amount: u64 = 10;
    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(dest_address, mint_amount.into(), nonce)
    });
    assert!(outcome.is_ok());

    // Validate post-state
    assert_eq!(
        U256::from(mint_amount),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );
}

#[test]
fn erc20_mint_out_of_gas() {
    const GAS_LIMIT: u64 = 67_000;
    const GAS_PRICE: u64 = 10;

    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );

    // Try mint transaction
    let mint_amount: u64 = rand::random();
    let nonce = source_account.use_nonce();
    let mut mint_tx = contract.mint(dest_address, mint_amount.into(), nonce.into());

    // not enough gas to cover intrinsic cost
    let intrinsic_gas = erc20::legacy_into_normalized_tx(mint_tx.clone())
        .intrinsic_gas(&aurora_evm::Config::shanghai())
        .unwrap();
    mint_tx.gas_limit = (intrinsic_gas - 1).into();
    let error = runner
        .submit_transaction(&source_account.secret_key, mint_tx.clone())
        .unwrap_err();
    assert_eq!(error.kind, EngineErrorKind::IntrinsicGasNotMet);

    // not enough gas to complete transaction
    mint_tx.gas_limit = U256::from(GAS_LIMIT);
    mint_tx.gas_price = U256::from(GAS_PRICE); // also set non-zero gas price to check gas still charged.
    let outcome = runner.submit_transaction(&source_account.secret_key, mint_tx);
    let error = outcome.unwrap();
    assert_eq!(error.status, TransactionStatus::OutOfGas);

    // Validate post-state

    utils::validate_address_balance_and_nonce(
        &runner,
        utils::address_from_secret_key(&source_account.secret_key),
        Wei::new_u64(INITIAL_BALANCE - GAS_LIMIT * GAS_PRICE),
        (INITIAL_NONCE + 2).into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(
        &runner,
        sdk::types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes()),
        Wei::new_u64(GAS_LIMIT * GAS_PRICE),
        U256::zero(),
    )
    .unwrap();
}

#[test]
fn profile_erc20_get_balance() {
    let (mut runner, mut source_account, _, contract) = initialize_erc20();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    let balance_tx = contract.balance_of(source_address, U256::zero());
    let (status, profile) = runner
        .profiled_view_call(&utils::as_view_call(balance_tx, source_address))
        .unwrap();
    assert!(status.is_ok());

    // call costs less than 3 Tgas
    utils::assert_gas_bound(profile.all_gas(), 3);
    // at least 80% of the cost is spent on wasm computation (as opposed to host functions)
    let wasm_fraction = (100 * profile.wasm_gas()) / profile.all_gas();
    assert!(
        (10..=20).contains(&wasm_fraction),
        "{wasm_fraction}% is not between 10% and 20%",
    );
}

#[test]
fn erc20_transfer_success() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    // Validate pre-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(&runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );

    // Do transfer
    let outcome = runner
        .submit_with_signer(&mut source_account, |nonce| {
            contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
        })
        .unwrap();
    assert!(outcome.status.is_ok());

    // Validate post-state
    assert_eq!(
        U256::from(INITIAL_BALANCE - TRANSFER_AMOUNT),
        get_address_erc20_balance(&runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::from(TRANSFER_AMOUNT),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );
}

#[test]
fn erc20_transfer_insufficient_balance() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    // Validate pre-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(&runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );

    // Do transfer
    let outcome = runner
        .submit_with_signer(&mut source_account, |nonce| {
            contract.transfer(dest_address, (2 * INITIAL_BALANCE).into(), nonce)
        })
        .unwrap();
    let message = parse_erc20_error_message(utils::unwrap_revert_slice(&outcome));
    assert_eq!(message, "&ERC20: transfer amount exceeds balance");

    // Validate post-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(&runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&runner, &source_account, dest_address, &contract)
    );
}

#[test]
fn deploy_erc_20_out_of_gas() {
    let mut runner = utils::deploy_runner();
    let mut rng = rand::rng();
    let source_account = utils::random_sk(&mut rng);
    let source_address = utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );

    let constructor = ERC20Constructor::load();
    let mut deploy_transaction = constructor.deploy("OutOfGas", "OOG", INITIAL_NONCE.into());

    // not enough gas to cover intrinsic cost
    let intrinsic_gas = erc20::legacy_into_normalized_tx(deploy_transaction.clone())
        .intrinsic_gas(&aurora_evm::Config::shanghai())
        .unwrap();
    deploy_transaction.gas_limit = (intrinsic_gas - 1).into();
    let error = runner
        .submit_transaction(&source_account, deploy_transaction.clone())
        .unwrap_err();
    assert_eq!(error.kind, EngineErrorKind::IntrinsicGasNotMet);

    // not enough gas to complete transaction
    deploy_transaction.gas_limit = U256::from(intrinsic_gas + 1);
    let outcome = runner.submit_transaction(&source_account, deploy_transaction);
    let error = outcome.unwrap();
    assert_eq!(error.status, TransactionStatus::OutOfGas);

    // Validate post-state
    utils::validate_address_balance_and_nonce(
        &runner,
        utils::address_from_secret_key(&source_account),
        Wei::new_u64(INITIAL_BALANCE),
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
}

#[test]
fn test_erc20_get_and_set_metadata() {
    let mut runner = utils::deploy_runner();
    let token_account_id = "token";
    let erc20_address = runner.deploy_erc20_token(token_account_id);
    let caller = runner.aurora_account_id.clone();
    // Getting ERC-20 metadata by Address.
    let result = runner.one_shot().call(
        "get_erc20_metadata",
        &caller,
        serde_json::to_vec::<Erc20Identifier>(&erc20_address.into()).unwrap(),
    );

    assert!(result.is_ok());

    let metadata: Erc20Metadata =
        serde_json::from_slice(&result.unwrap().return_data.as_value().unwrap()).unwrap();
    assert_eq!(metadata, Erc20Metadata::default());

    let new_metadata = Erc20Metadata {
        name: "USD Token".to_string(),
        symbol: "USDT".to_string(),
        decimals: 20,
    };

    let result = runner.call(
        "set_erc20_metadata",
        &caller,
        serde_json::to_vec(&SetErc20MetadataArgs {
            erc20_identifier: erc20_address.into(),
            metadata: new_metadata.clone(),
        })
        .unwrap(),
    );
    assert!(result.is_ok());

    // Getting ERC-20 metadata by NEP-141 account id.
    let result = runner.one_shot().call(
        "get_erc20_metadata",
        &caller,
        serde_json::to_vec::<Erc20Identifier>(
            &AccountId::from_str(token_account_id).unwrap().into(),
        )
        .unwrap(),
    );
    assert!(result.is_ok());

    let metadata: Erc20Metadata =
        serde_json::from_slice(&result.unwrap().return_data.as_value().unwrap()).unwrap();
    assert_eq!(metadata, new_metadata);
}

#[test]
fn test_erc20_get_and_set_metadata_by_owner() {
    let mut runner = utils::deploy_runner();
    let token_account_id = "token";
    let erc20_address = runner.deploy_erc20_token(token_account_id);
    let caller = runner.aurora_account_id.clone();

    // Change the owner of the aurora contract
    let new_owner = "new_owner";
    let set_owner_args = SetOwnerArgs {
        new_owner: str_to_account_id(new_owner),
    };

    let result = runner.call(
        "set_owner",
        &caller,
        borsh::to_vec(&set_owner_args).unwrap(),
    );
    assert!(result.is_ok());

    let caller = new_owner;

    // Getting ERC-20 metadata by Address.
    let result = runner.one_shot().call(
        "get_erc20_metadata",
        caller,
        serde_json::to_vec::<Erc20Identifier>(&erc20_address.into()).unwrap(),
    );

    assert!(result.is_ok());

    let metadata: Erc20Metadata =
        serde_json::from_slice(&result.unwrap().return_data.as_value().unwrap()).unwrap();
    assert_eq!(metadata, Erc20Metadata::default());

    let new_metadata = Erc20Metadata {
        name: "USD Token".to_string(),
        symbol: "USDT".to_string(),
        decimals: 20,
    };

    let result = runner.call(
        "set_erc20_metadata",
        caller,
        serde_json::to_vec(&SetErc20MetadataArgs {
            erc20_identifier: erc20_address.into(),
            metadata: new_metadata.clone(),
        })
        .unwrap(),
    );
    assert!(result.is_ok());

    // Getting ERC-20 metadata by NEP-141 account id.
    let result = runner.one_shot().call(
        "get_erc20_metadata",
        caller,
        serde_json::to_vec::<Erc20Identifier>(
            &AccountId::from_str(token_account_id).unwrap().into(),
        )
        .unwrap(),
    );
    assert!(result.is_ok());

    let metadata: Erc20Metadata =
        serde_json::from_slice(&result.unwrap().return_data.as_value().unwrap()).unwrap();
    assert_eq!(metadata, new_metadata);
}

fn get_address_erc20_balance(
    runner: &utils::AuroraRunner,
    signer: &Signer,
    address: Address,
    contract: &ERC20,
) -> U256 {
    let balance_tx = contract.balance_of(address, signer.nonce.into());
    let result = runner
        .view_call(&utils::as_view_call(
            balance_tx,
            utils::address_from_secret_key(&signer.secret_key),
        ))
        .unwrap();
    let bytes = match result {
        TransactionStatus::Succeed(bytes) => bytes,
        err => panic!("Unexpected view call status {err:?}"),
    };
    U256::from_big_endian(&bytes)
}

fn parse_erc20_error_message(result: &[u8]) -> &str {
    let start_index = result.find_char('&').unwrap();
    let end_index = result[start_index..].find_byte(0).unwrap() + start_index;

    std::str::from_utf8(&result[start_index..end_index]).unwrap()
}

fn initialize_erc20() -> (utils::AuroraRunner, Signer, Address, ERC20) {
    // set up Aurora runner and accounts
    let mut runner = utils::deploy_runner();
    let mut rng = rand::rng();
    let source_account = utils::random_sk(&mut rng);
    let source_address = utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );
    let dest_address = utils::address_from_secret_key(&utils::random_sk(&mut rng));

    let mut signer = Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;
    let nonce = signer.use_nonce();
    let constructor = ERC20Constructor::load();
    let contract = ERC20(runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy("TestToken", "TEST", nonce.into()),
        constructor,
    ));

    (runner, signer, dest_address, contract)
}
