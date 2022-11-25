use crate::prelude::Wei;
use crate::prelude::{Address, U256};
use crate::test_utils::{
    self,
    erc20::{ERC20Constructor, ERC20},
    Signer,
};
use aurora_engine::parameters::TransactionStatus;
use aurora_engine_sdk as sdk;
use bstr::ByteSlice;
use libsecp256k1::SecretKey;

const INITIAL_BALANCE: u64 = 1_000_000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 67;

#[test]
fn erc20_mint() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
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
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
    );
}

#[test]
fn erc20_mint_out_of_gas() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
    );

    // Try mint transaction
    let mint_amount: u64 = rand::random();
    let nonce = source_account.use_nonce();
    let mut mint_tx = contract.mint(dest_address, mint_amount.into(), nonce.into());

    // not enough gas to cover intrinsic cost
    let intrinsic_gas = test_utils::erc20::legacy_into_normalized_tx(mint_tx.clone())
        .intrinsic_gas(&evm::Config::istanbul())
        .unwrap();
    mint_tx.gas_limit = (intrinsic_gas - 1).into();
    let outcome = runner.submit_transaction(&source_account.secret_key, mint_tx.clone());
    let error = outcome.unwrap_err();
    let error_message = format!("{:?}", error);
    assert!(error_message.contains("ERR_INTRINSIC_GAS"));

    // not enough gas to complete transaction
    const GAS_LIMIT: u64 = 67_000;
    const GAS_PRICE: u64 = 10;
    mint_tx.gas_limit = U256::from(GAS_LIMIT);
    mint_tx.gas_price = U256::from(GAS_PRICE); // also set non-zero gas price to check gas still charged.
    let outcome = runner.submit_transaction(&source_account.secret_key, mint_tx);
    let error = outcome.unwrap();
    assert_eq!(error.status, TransactionStatus::OutOfGas);

    // Validate post-state

    test_utils::validate_address_balance_and_nonce(
        &runner,
        test_utils::address_from_secret_key(&source_account.secret_key),
        Wei::new_u64(INITIAL_BALANCE - GAS_LIMIT * GAS_PRICE),
        (INITIAL_NONCE + 2).into(),
    );
    test_utils::validate_address_balance_and_nonce(
        &runner,
        sdk::types::near_account_to_evm_address(
            runner.context.predecessor_account_id.as_ref().as_bytes(),
        ),
        Wei::new_u64(GAS_LIMIT * GAS_PRICE),
        U256::zero(),
    );
}

#[test]
fn profile_erc20_get_balance() {
    let (mut runner, mut source_account, _, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    let balance_tx = contract.balance_of(source_address, U256::zero());
    let (result, profile) =
        runner.profiled_view_call(test_utils::as_view_call(balance_tx, source_address));
    assert!(result.is_ok());

    // call costs less than 2 Tgas
    test_utils::assert_gas_bound(profile.all_gas(), 2);
    // at least 70% of the cost is spent on wasm computation (as opposed to host functions)
    let wasm_fraction = (100 * profile.wasm_gas()) / profile.all_gas();
    assert!(
        (20..=30).contains(&wasm_fraction),
        "{}% is not between 20% and 30%",
        wasm_fraction
    );
}

#[test]
fn erc20_transfer_success() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    // Validate pre-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(&mut runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
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
        get_address_erc20_balance(&mut runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::from(TRANSFER_AMOUNT),
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
    );
}

#[test]
fn erc20_transfer_insufficient_balance() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    // Validate pre-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(&mut runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
    );

    // Do transfer
    let outcome = runner
        .submit_with_signer(&mut source_account, |nonce| {
            contract.transfer(dest_address, (2 * INITIAL_BALANCE).into(), nonce)
        })
        .unwrap();
    let message = parse_erc20_error_message(&test_utils::unwrap_revert(outcome));
    assert_eq!(&message, "&ERC20: transfer amount exceeds balance");

    // Validate post-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(&mut runner, &source_account, source_address, &contract)
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(&mut runner, &source_account, dest_address, &contract)
    );
}

#[test]
fn deploy_erc_20_out_of_gas() {
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );

    let constructor = ERC20Constructor::load();
    let mut deploy_transaction = constructor.deploy("OutOfGas", "OOG", INITIAL_NONCE.into());

    // not enough gas to cover intrinsic cost
    let intrinsic_gas = test_utils::erc20::legacy_into_normalized_tx(deploy_transaction.clone())
        .intrinsic_gas(&evm::Config::istanbul())
        .unwrap();
    deploy_transaction.gas_limit = (intrinsic_gas - 1).into();
    let outcome = runner.submit_transaction(&source_account, deploy_transaction.clone());
    let error = outcome.unwrap_err();
    let error_message = format!("{:?}", error);
    assert!(error_message.contains("ERR_INTRINSIC_GAS"));

    // not enough gas to complete transaction
    deploy_transaction.gas_limit = U256::from(intrinsic_gas + 1);
    let outcome = runner.submit_transaction(&source_account, deploy_transaction);
    let error = outcome.unwrap();
    assert_eq!(error.status, TransactionStatus::OutOfGas);

    // Validate post-state
    test_utils::validate_address_balance_and_nonce(
        &runner,
        test_utils::address_from_secret_key(&source_account),
        Wei::new_u64(INITIAL_BALANCE),
        (INITIAL_NONCE + 1).into(),
    );
}

fn get_address_erc20_balance(
    runner: &mut test_utils::AuroraRunner,
    signer: &Signer,
    address: Address,
    contract: &ERC20,
) -> U256 {
    let balance_tx = contract.balance_of(address, signer.nonce.into());
    let result = runner
        .view_call(test_utils::as_view_call(
            balance_tx,
            test_utils::address_from_secret_key(&signer.secret_key),
        ))
        .unwrap();
    let bytes = match result {
        aurora_engine::parameters::TransactionStatus::Succeed(bytes) => bytes,
        err => panic!("Unexpected view call status {:?}", err),
    };
    U256::from_big_endian(&bytes)
}

fn parse_erc20_error_message(result: &[u8]) -> String {
    let start_index = result.find_char('&').unwrap();
    let end_index = result[start_index..].find_byte(0).unwrap() + start_index;

    String::from_utf8(result[start_index..end_index].to_vec()).unwrap()
}

fn initialize_erc20() -> (test_utils::AuroraRunner, Signer, Address, ERC20) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));

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
