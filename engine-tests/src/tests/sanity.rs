use crate::prelude::{Address, U256};
use crate::prelude::{Wei, ERC20_MINT_SELECTOR};
use crate::test_utils;
use crate::tests::state_migration;
use aurora_engine::fungible_token::FungibleTokenMetadata;
use aurora_engine::parameters::{SubmitResult, TransactionStatus};
use aurora_engine_sdk as sdk;
use borsh::BorshSerialize;
use rand::RngCore;
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000_000);
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(123);
const GAS_PRICE: u64 = 10;

#[test]
fn test_transaction_to_zero_address() {
    // Transactions that explicit list `0x0000...` as the `to` field in the transaction
    // should not be interpreted as contract creation. Previously this was the case
    // and it caused the Engine to incorrectly derive the sender's address.
    // See the mismatch between the sender address reported by the Aurora explorer
    // and the sender logged by the engine:
    //   - https://testnet.aurorascan.dev/tx/0x51846313113e13ff87ccbd153f1b339b857bf7729fe16af7d351ff06943c4c20
    //   - https://explorer.testnet.near.org/transactions/5URFuet378c6zokikG62uK4YH31AnZb99pDPRnVJBAy2
    // This is a test to show the bug is now fixed.
    let tx_hex = "f8648080836691b79400000000000000000000000000000000000000008080849c8a82caa0464cada9d6a907f5537dcc0f95274a30ddaeff33276e9b3993815586293a2010a07626bd794381ba59f30e26ec6f3448d19f63bb12dcda19acda429b2fb7d3dfba";
    let tx_bytes = hex::decode(tx_hex).unwrap();
    let tx = aurora_engine_transactions::EthTransactionKind::try_from(tx_bytes.as_slice()).unwrap();
    let normalized_tx = aurora_engine_transactions::NormalizedEthTransaction::from(tx);
    let address = normalized_tx.address.as_ref().unwrap();
    let sender = hex::encode(address.as_bytes());
    assert_eq!(sender.as_str(), "63eafba871e0bda44be3cde19df5aa1c0f078142");

    // We want the standalone engine to still reproduce the old behaviour for blocks before the bug fix, and
    // to use the correct parsing for blocks after the fix.
    let mut runner = test_utils::standalone::StandaloneRunner::default();
    runner.init_evm_with_chain_id(normalized_tx.chain_id.unwrap());
    let mut context = test_utils::AuroraRunner::default().context;
    context.input = tx_bytes;
    // Prior to the fix the zero address is interpreted as None, causing a contract deployment.
    // It also incorrectly derives the sender address, so does not increment the right nonce.
    context.block_index = aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT - 1;
    let result = runner.submit_raw(test_utils::SUBMIT, &context).unwrap();
    assert_eq!(result.gas_used, 53_000);
    runner.env.block_height = aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT;
    assert_eq!(runner.get_nonce(address), U256::zero());

    // After the fix this transaction is simply a transfer of 0 ETH to the zero address
    context.block_index = aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT;
    let result = runner.submit_raw(test_utils::SUBMIT, &context).unwrap();
    assert_eq!(result.gas_used, 21_000);
    runner.env.block_height = aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT + 1;
    assert_eq!(runner.get_nonce(address), U256::one());
}

#[test]
fn test_state_format() {
    // The purpose of this test is to make sure that if we accidentally
    // change the binary format of the `EngineState` then we will know
    // about it. This is important because changing the state format will
    // break the contract unless we do a state migration.
    let args = aurora_engine::parameters::NewCallArgs {
        chain_id: aurora_engine_types::types::u256_to_arr(&666.into()),
        owner_id: "boss".parse().unwrap(),
        bridge_prover_id: "prover_mcprovy_face".parse().unwrap(),
        upgrade_delay_blocks: 3,
    };
    let state: aurora_engine::engine::EngineState = args.into();
    let expected_hex: String = [
        "000000000000000000000000000000000000000000000000000000000000029a",
        "04000000626f7373",
        "1300000070726f7665725f6d6370726f76795f66616365",
        "0300000000000000",
    ]
    .concat();
    assert_eq!(hex::encode(state.try_to_vec().unwrap()), expected_hex);
}

#[test]
fn test_deploy_contract() {
    let (mut runner, mut signer, _) = initialize_transfer();

    // Randomly generate some "contract code"
    const LEN: usize = 567;
    let code: Vec<u8> = {
        let mut rng = rand::thread_rng();
        let mut buf = vec![0u8; LEN];
        rng.fill_bytes(&mut buf);
        buf
    };

    // Deploy that code
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            test_utils::create_deploy_transaction(code.clone(), nonce)
        })
        .unwrap();
    let address = Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();

    // Confirm the code stored at that address is equal to the input code.
    let stored_code = runner.get_code(address);
    assert_eq!(code, stored_code);
}

#[test]
fn test_deploy_largest_contract() {
    // Check to see we can deploy the largest allowed contract size within the
    // NEAR gas limit of 200 Tgas.
    let (mut runner, mut signer, _) = initialize_transfer();

    let len = evm::Config::berlin().create_contract_limit.unwrap();
    let code: Vec<u8> = {
        let mut rng = rand::thread_rng();
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    };

    // Deploy that code
    let (result, profile) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            test_utils::create_deploy_transaction(code.clone(), nonce)
        })
        .unwrap();

    // At least 5 million EVM gas
    assert!(
        result.gas_used >= 5_000_000,
        "{:?} not greater than 5 million",
        result.gas_used,
    );

    // Less than 12 NEAR Tgas
    test_utils::assert_gas_bound(profile.all_gas(), 12);
}

#[test]
fn test_log_address() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let mut deploy_contract = |name: &str, signer: &mut test_utils::Signer| {
        let constructor = test_utils::solidity::ContractConstructor::compile_from_source(
            "src/tests/res",
            "target/solidity_build",
            "caller.sol",
            name,
        );

        let nonce = signer.use_nonce();
        runner.deploy_contract(
            &signer.secret_key,
            |c| c.deploy_without_constructor(nonce.into()),
            constructor,
        )
    };

    let greet_contract = deploy_contract("Greeter", &mut signer);
    let caller_contract = deploy_contract("Caller", &mut signer);

    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            caller_contract.call_method_with_args(
                "greet",
                &[ethabi::Token::Address(greet_contract.address.raw())],
                nonce,
            )
        })
        .unwrap();

    // Address included in the log should come from the contract emitting the log,
    // not the contract that invoked the call.
    let log_address = result.logs.first().unwrap().address;
    assert_eq!(log_address, greet_contract.address);
}

#[test]
fn test_is_contract() {
    let (mut runner, mut signer, _) = initialize_transfer();
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);

    let constructor = test_utils::solidity::ContractConstructor::force_compile(
        "src/tests/res",
        "target/solidity_build",
        "is_contract.sol",
        "IsContract",
    );

    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    let call_contract = |account: Address,
                         runner: &mut test_utils::AuroraRunner,
                         signer: &mut test_utils::Signer|
     -> bool {
        let result = runner
            .submit_with_signer(signer, |nonce| {
                contract.call_method_with_args(
                    "isContract",
                    &[ethabi::Token::Address(account.raw())],
                    nonce,
                )
            })
            .unwrap();
        let bytes = test_utils::unwrap_success_slice(&result);
        ethabi::decode(&[ethabi::ParamType::Bool], bytes)
            .unwrap()
            .pop()
            .unwrap()
            .into_bool()
            .unwrap()
    };

    // Should return false for accounts that don't exist
    assert_eq!(
        call_contract(Address::from_array([1; 20]), &mut runner, &mut signer),
        false,
    );

    // Should return false for accounts that don't have contract code
    assert_eq!(
        call_contract(signer_address, &mut runner, &mut signer),
        false,
    );

    // Should return true for contracts
    assert_eq!(
        call_contract(contract.address, &mut runner, &mut signer),
        true,
    );
}

#[test]
fn test_solidity_pure_bench() {
    let (mut runner, mut signer, _) = initialize_transfer();
    runner.wasm_config.limit_config.max_gas_burnt = u64::MAX;

    let constructor = test_utils::solidity::ContractConstructor::force_compile(
        "src/tests/res",
        "target/solidity_build",
        "bench.sol",
        "Bencher",
    );

    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    // Number of iterations to do
    let loop_limit: u32 = 10_000;
    let (result, profile) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            contract.call_method_with_args(
                "cpu_ram_soak_test",
                &[ethabi::Token::Uint(loop_limit.into())],
                nonce,
            )
        })
        .unwrap();

    assert!(
        result.gas_used > 38_000_000,
        "Over 38 million EVM gas is used"
    );
    let near_gas = profile.all_gas();
    assert!(
        near_gas > 1500 * 1_000_000_000_000,
        "Expected 1500 NEAR Tgas to be used, but only consumed {}",
        near_gas / 1_000_000_000_000,
    );

    // Pure rust version of the same contract
    let base_path = std::path::Path::new("../etc").join("benchmark-contract");
    let output_path =
        base_path.join("target/wasm32-unknown-unknown/release/benchmark_contract.wasm");
    test_utils::rust::compile(base_path);
    let contract_bytes = std::fs::read(output_path).unwrap();
    let code = near_primitives_core::contract::ContractCode::new(contract_bytes, None);
    let mut context = runner.context.clone();
    context.input = loop_limit.to_le_bytes().to_vec();
    let (outcome, error) = near_vm_runner::run(
        &code,
        "cpu_ram_soak_test",
        &mut runner.ext,
        context,
        &runner.wasm_config,
        &runner.fees_config,
        &[],
        runner.current_protocol_version,
        Some(&runner.cache),
    );
    if let Some(e) = error {
        panic!("{:?}", e);
    }
    let outcome = outcome.unwrap();
    let profile = test_utils::ExecutionProfile::new(&outcome);
    // Check the contract actually did the work.
    assert_eq!(&outcome.logs, &[format!("Done {} iterations!", loop_limit)]);
    assert!(profile.all_gas() < 1_000_000_000_000); // Less than 1 Tgas used!
}

#[test]
fn test_revert_during_contract_deploy() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let constructor = test_utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "reverter.sol",
        "ReverterByDefault",
    );

    let nonce = signer.use_nonce();
    let deploy_tx =
        constructor.deploy_with_args(nonce.into(), &[ethabi::Token::Uint(U256::zero())]);
    let submit_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();

    let revert_bytes = crate::test_utils::unwrap_revert(submit_result);
    // First 4 bytes is a function selector with signature `Error(string)`
    assert_eq!(&revert_bytes[0..4], &[8, 195, 121, 160]);
    // Remaining data is an ABI-encoded string
    let revert_message = ethabi::decode(&[ethabi::ParamType::String], &revert_bytes[4..])
        .unwrap()
        .pop()
        .unwrap()
        .into_string()
        .unwrap();

    assert_eq!(revert_message.as_str(), "Revert message");
}

#[test]
fn test_timestamp() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let constructor = test_utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "timestamp.sol",
        "Timestamp",
    );

    // deploy contract
    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    // set timestamp
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let t_ns = t.as_nanos();
    let t_s = U256::from(t.as_secs());
    runner.context.block_timestamp = t_ns as u64;

    // call contract
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_without_args("getCurrentBlockTimestamp", nonce)
        })
        .unwrap();
    let timestamp = U256::from_big_endian(&test_utils::unwrap_success(result));

    // Check time is correct.
    // The `+1`  is needed here because the runner increments the context
    // timestamp by 1 second automatically before each transaction.
    assert_eq!(t_s + 1, timestamp);
}

#[test]
fn test_override_state() {
    let (mut runner, mut account1, viewer_address) = initialize_transfer();
    let account1_address = test_utils::address_from_secret_key(&account1.secret_key);
    let mut account2 = test_utils::Signer::random();
    let account2_address = test_utils::address_from_secret_key(&account2.secret_key);
    runner.create_address(account2_address, INITIAL_BALANCE, INITIAL_NONCE.into());

    let contract = test_utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "poster.sol",
        "Poster",
    );

    // deploy contract
    let result = runner
        .submit_with_signer(&mut account1, |nonce| {
            crate::prelude::transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: Default::default(),
                gas_limit: u64::MAX.into(),
                to: None,
                value: Default::default(),
                data: contract.code.clone(),
            }
        })
        .unwrap();
    let address = Address::try_from_slice(&test_utils::unwrap_success(result)).unwrap();
    let contract = contract.deployed_at(address);

    // define functions to interact with the contract
    let get_address = |runner: &test_utils::AuroraRunner| {
        let result = runner
            .view_call(test_utils::as_view_call(
                contract.call_method_without_args("get", U256::zero()),
                viewer_address,
            ))
            .unwrap();
        match result {
            crate::prelude::parameters::TransactionStatus::Succeed(bytes) => {
                Address::try_from_slice(&bytes[12..32]).unwrap()
            }
            _ => panic!("tx failed"),
        }
    };

    let post_address = |runner: &mut test_utils::AuroraRunner, signer: &mut test_utils::Signer| {
        let result = runner
            .submit_with_signer(signer, |nonce| {
                contract.call_method_with_args(
                    "post",
                    &[ethabi::Token::String("Hello, world!".to_string())],
                    nonce,
                )
            })
            .unwrap();
        assert!(result.status.is_ok());
    };

    // Assert the initial state is 0
    assert_eq!(get_address(&runner), Address::new(H160([0; 20])));
    post_address(&mut runner, &mut account1);
    // Assert the address matches the first caller
    assert_eq!(get_address(&runner), account1_address);
    post_address(&mut runner, &mut account2);
    // Assert the address matches the second caller
    assert_eq!(get_address(&runner), account2_address);
}

#[test]
fn test_num_wasm_functions() {
    // Counts the number of functions in our wasm output.
    // See https://github.com/near/nearcore/issues/4814 for context
    let runner = test_utils::deploy_evm();
    let module = walrus::ModuleConfig::default()
        .parse(runner.code.code())
        .unwrap();
    let num_functions = module.funcs.iter().count();
    assert!(
        num_functions <= 1440,
        "{} is not less than 1440",
        num_functions
    );
}

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
        tx.gas_limit = 10_000.into(); // this is not enough gas
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
        tx.gas_limit = 30_000.into();
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
    let relayer_address = sdk::types::near_account_to_evm_address(
        runner.context.predecessor_account_id.as_ref().as_bytes(),
    );

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
        tx.gas_limit = 3_000_000.into();
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
    let relayer = sdk::types::near_account_to_evm_address(
        runner.context.predecessor_account_id.as_ref().as_bytes(),
    );

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

use aurora_engine_types::H160;
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
    let block_hash = aurora_engine::engine::compute_block_hash(chain_id, 10, account_id.as_bytes());

    assert_eq!(
        hex::encode(block_hash.0).as_str(),
        "c4a46f076b64877cbd8c5dbfd7bfbbea21a5653b79e3b6d06b6dfb5c88f1c384",
    );
}

#[test]
fn test_block_hash_api() {
    let mut runner = test_utils::deploy_evm();

    let block_height: u64 = 10;
    let (maybe_outcome, maybe_error) = runner.call(
        "get_block_hash",
        "any.near",
        block_height.try_to_vec().unwrap(),
    );
    if let Some(error) = maybe_error {
        panic!("Call failed: {:?}", error);
    }
    let outcome = maybe_outcome.unwrap();
    let block_hash = outcome.return_data.as_value().unwrap();

    assert_eq!(
        hex::encode(&block_hash).as_str(),
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

    let account_id: String = runner.context.signer_account_id.clone().into();
    let (maybe_outcome, maybe_error) = runner.call("ft_metadata", &account_id, Vec::new());
    assert!(maybe_error.is_none());
    let outcome = maybe_outcome.unwrap();
    let json_value =
        aurora_engine::json::parse_json(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(
        json_value,
        aurora_engine::json::JsonValue::from(FungibleTokenMetadata::default())
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
        Address::new(H160([1; 20])),
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
    let mut tx = test_utils::transfer(Address::new(H160([1; 20])), TRANSFER_AMOUNT, nonce.into());
    tx.gas_limit = 3_000_000.into();
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

    let args = (address, INITIAL_NONCE, INITIAL_BALANCE.raw().low_u64());
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
    let x = aurora.call(method, address.as_bytes());
    match &x.outcome().status {
        near_sdk_sim::transaction::ExecutionStatus::SuccessValue(b) => U256::from_big_endian(&b),
        other => panic!("Unexpected outcome: {:?}", other),
    }
}
