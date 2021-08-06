use crate::prelude::Address;
use crate::test_utils;
use crate::types::{self, Wei, ERC20_MINT_SELECTOR};
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
    let err = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            test_utils::transfer(dest_address, INITIAL_BALANCE + INITIAL_BALANCE, nonce)
        })
        .unwrap_err();
    let error_message = format!("{:?}", err);
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
        types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());

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
    let err = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap_err();
    let error_message = format!("{:?}", err);
    assert!(error_message.contains("ERR_OUT_OF_FUND"));

    // validate post-state
    let relayer =
        types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());
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
        crate::types::u256_to_arr(&number)
    };
    let account_id = runner.aurora_account_id;
    let block_hash = crate::engine::Engine::compute_block_hash(chain_id, 10, account_id.as_bytes());

    assert_eq!(
        hex::encode(block_hash.0).as_str(),
        "4c8a60b32b74f184438a5e450951570bc1bda37caa7b6a3f178b80395845fb80"
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

    if !result.status {
        panic!("{}", String::from_utf8_lossy(&result.result));
    }
}
