use crate::prelude::{Address, U256};
use crate::test_utils::{
    self,
    erc20::{ERC20Constructor, ERC20},
};
use bstr::ByteSlice;
use secp256k1::SecretKey;

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 67;

#[test]
fn erc20_mint() {
    let (mut runner, source_account, dest_address, contract) = initialize_erc20();

    // Validate pre-state
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 1).into(),
            dest_address,
            &contract
        )
    );

    // Do mint transaction
    let mint_amount: u64 = rand::random();
    let mint_tx = contract.mint(dest_address, mint_amount.into(), (INITIAL_NONCE + 2).into());
    let outcome = runner.submit_transaction(&source_account, mint_tx);
    assert!(outcome.is_ok());

    // Validate post-state
    assert_eq!(
        U256::from(mint_amount),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 3).into(),
            dest_address,
            &contract
        )
    );
}

#[test]
fn erc20_transfer_success() {
    let (mut runner, source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account);

    let mint_tx = contract.mint(
        source_address,
        INITIAL_BALANCE.into(),
        (INITIAL_NONCE + 1).into(),
    );
    let outcome = runner.submit_transaction(&source_account, mint_tx);
    assert!(outcome.is_ok());

    // Validate pre-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 2).into(),
            source_address,
            &contract
        )
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 3).into(),
            dest_address,
            &contract
        )
    );

    // Do transfer
    let transfer_tx = contract.transfer(
        dest_address,
        TRANSFER_AMOUNT.into(),
        (INITIAL_NONCE + 4).into(),
    );
    let outcome = runner
        .submit_transaction(&source_account, transfer_tx)
        .unwrap();
    assert!(outcome.status);

    // Validate post-state
    assert_eq!(
        U256::from(INITIAL_BALANCE - TRANSFER_AMOUNT),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 5).into(),
            source_address,
            &contract
        )
    );
    assert_eq!(
        U256::from(TRANSFER_AMOUNT),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 6).into(),
            dest_address,
            &contract
        )
    );
}

#[test]
fn erc20_transfer_insufficient_balance() {
    let (mut runner, source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account);

    let mint_tx = contract.mint(
        source_address,
        INITIAL_BALANCE.into(),
        (INITIAL_NONCE + 1).into(),
    );
    let outcome = runner.submit_transaction(&source_account, mint_tx);
    assert!(outcome.is_ok());

    // Validate pre-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 2).into(),
            source_address,
            &contract
        )
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 3).into(),
            dest_address,
            &contract
        )
    );

    // Do transfer
    let transfer_tx = contract.transfer(
        dest_address,
        (2 * INITIAL_BALANCE).into(),
        (INITIAL_NONCE + 4).into(),
    );
    let outcome = runner
        .submit_transaction(&source_account, transfer_tx)
        .unwrap();
    assert!(!outcome.status); // status == false means execution error
    let message = parse_erc20_error_message(&outcome.result);
    assert_eq!(&message, "&ERC20: transfer amount exceeds balance");

    // Validate post-state
    assert_eq!(
        U256::from(INITIAL_BALANCE),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 5).into(),
            source_address,
            &contract
        )
    );
    assert_eq!(
        U256::zero(),
        get_address_erc20_balance(
            &mut runner,
            &source_account,
            (INITIAL_NONCE + 6).into(),
            dest_address,
            &contract
        )
    );
}

fn get_address_erc20_balance(
    runner: &mut test_utils::AuroraRunner,
    signing_account: &SecretKey,
    nonce: U256,
    address: Address,
    contract: &ERC20,
) -> U256 {
    let balance_tx = contract.balance_of(address, nonce);
    let outcome = runner.submit_transaction(signing_account, balance_tx);
    assert!(outcome.is_ok());
    U256::from_big_endian(&outcome.unwrap().result)
}

fn parse_erc20_error_message(result: &[u8]) -> String {
    let start_index = result.find_char('&').unwrap();
    let end_index = result[start_index..].find_byte(0).unwrap() + start_index;

    String::from_utf8(result[start_index..end_index].to_vec()).unwrap()
}

fn initialize_erc20() -> (test_utils::AuroraRunner, SecretKey, Address, ERC20) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE.into(), INITIAL_NONCE.into());
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));

    let constructor = ERC20Constructor::load();
    let contract = ERC20(runner.deploy_contract(
        &source_account,
        |c| c.deploy("TestToken", "TEST", INITIAL_NONCE.into()),
        constructor,
    ));

    (runner, source_account, dest_address, contract)
}
