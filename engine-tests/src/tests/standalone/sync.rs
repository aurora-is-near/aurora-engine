use aurora_engine_sdk::env::{Env, Timestamp, DEFAULT_PREPAID_GAS};
use aurora_engine_types::parameters::connector;
use aurora_engine_types::types::{Address, Balance, Wei};
use aurora_engine_types::{account_id::AccountId, H160, H256, U256};
use engine_standalone_storage::sync;

use crate::utils::solidity::erc20::{ERC20Constructor, ERC20};
use crate::utils::{self, standalone::StandaloneRunner};

#[test]
fn test_consume_block_message() {
    let (runner, block_message) = initialize();

    assert_eq!(
        runner
            .storage
            .get_block_height_by_hash(block_message.hash)
            .unwrap(),
        block_message.height,
    );
    assert_eq!(
        runner
            .storage
            .get_block_hash_by_height(block_message.height)
            .unwrap(),
        block_message.hash,
    );
    assert_eq!(
        runner
            .storage
            .get_block_metadata(block_message.hash)
            .unwrap(),
        block_message.metadata,
    );

    runner.close();
}

#[test]
fn test_consume_deploy_message() {
    let (mut runner, block_message) = initialize();

    let code = b"hello_world!".to_vec();
    let input = utils::create_deploy_transaction(code.clone(), U256::zero()).data;
    let tx_kind = sync::types::TransactionKind::new_deploy(input);

    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        action_hash: H256::default(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        trace_kind: None,
    };

    let outcome = sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    let diff = runner
        .storage
        .get_transaction_diff(engine_standalone_storage::TransactionIncluded {
            block_hash: block_message.hash,
            position: 0,
        })
        .unwrap();
    let mut deployed_address = Address::zero();
    for (key, value) in &diff {
        match value.value() {
            Some(bytes) if bytes == code.as_slice() => {
                deployed_address = Address::try_from_slice(&key[2..22]).unwrap();
                break;
            }
            _ => {}
        }
    }

    assert_eq!(runner.get_code(&deployed_address), code);

    runner.close();
}

#[test]
fn test_consume_deploy_erc20_message() {
    let (mut runner, block_message) = initialize();

    let token: AccountId = "some_nep141.near".parse().unwrap();
    let mint_amount: u128 = 555_555;
    let dest_address = Address::new(H160([170u8; 20]));

    let args = aurora_engine::parameters::DeployErc20TokenArgs::Legacy(token.clone());
    let tx_kind = sync::types::TransactionKind::deploy_erc20(&args);
    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        action_hash: H256::default(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        trace_kind: None,
    };

    // Deploy ERC-20 (this would be the flow for bridging a new NEP-141 to Aurora)
    let outcome = sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    let erc20_address = runner
        .storage
        .with_engine_access(runner.env.block_height + 1, 0, &[], |io| {
            aurora_engine::engine::get_erc20_from_nep141(&io, &token)
        })
        .result
        .unwrap();

    runner.env.block_height += 1;
    runner.env.signer_account_id = "some_account.near".parse().unwrap();
    runner.env.predecessor_account_id = token;
    utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = utils::standalone::mocks::compute_block_hash(runner.env.block_height);

    let args = connector::FtOnTransferArgs {
        sender_id: "mr_money_bags.near".parse().unwrap(),
        amount: Balance::new(mint_amount),
        msg: hex::encode(dest_address.as_bytes()),
    };
    let tx_kind = sync::types::TransactionKind::new_ft_on_transfer(&args);
    let transaction_message = sync::types::TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        action_hash: H256::default(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        trace_kind: None,
    };

    // Mint new tokens (via ft_on_transfer flow, same as the bridge)
    let outcome = sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    // Check balance is correct
    let deployed_token = ERC20(ERC20Constructor::load().0.deployed_at(erc20_address));
    let signer = utils::Signer::random();
    let tx = deployed_token.balance_of(dest_address, signer.nonce.into());
    let result = runner.submit_transaction(&signer.secret_key, tx).unwrap();
    assert_eq!(
        U256::from_big_endian(&utils::unwrap_success(result)).low_u128(),
        mint_amount
    );
}

#[test]
fn test_consume_ft_on_transfer_message() {
    // Only need to check the case of aurora calling `ft_on_transfer` on itself, the other case
    // is handled in the `test_consume_deploy_erc20_message` above.
    let (mut runner, block_message) = initialize();

    let mint_amount = 8_675_309;
    let fee = Wei::zero();
    let dest_address = Address::new(H160([221u8; 20]));

    // Mint ETH on Aurora per the bridge workflow
    let args = connector::FtOnTransferArgs {
        sender_id: "mr_money_bags.near".parse().unwrap(),
        amount: Balance::new(mint_amount),
        msg: [
            "relayer.near",
            ":",
            hex::encode(fee.to_bytes()).as_str(),
            hex::encode(dest_address.as_bytes()).as_str(),
        ]
        .concat(),
    };
    let tx_kind = sync::types::TransactionKind::new_ft_on_transfer(&args);
    let caller = utils::standalone::mocks::EXT_ETH_CONNECTOR.parse().unwrap();
    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller,
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        action_hash: H256::default(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        trace_kind: None,
    };

    let outcome = sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    assert_eq!(
        runner.get_balance(&dest_address).raw().low_u128(),
        mint_amount
    );
}

#[test]
fn test_consume_call_message() {
    let (mut runner, _) = initialize();

    let caller = "some_account.near";
    let initial_balance = Wei::new_u64(800_000);
    let transfer_amount = Wei::new_u64(115_321);
    let caller_address = aurora_engine_sdk::types::near_account_to_evm_address(caller.as_bytes());
    let recipient_address = Address::new(H160([1u8; 20]));
    runner.mint_account(caller_address, initial_balance, U256::zero(), None);

    runner.env.block_height += 1;
    runner.env.signer_account_id = caller.parse().unwrap();
    runner.env.predecessor_account_id = caller.parse().unwrap();
    utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = utils::standalone::mocks::compute_block_hash(runner.env.block_height);

    let args = simple_transfer_args(recipient_address, transfer_amount);
    let tx_kind = sync::types::TransactionKind::new_call(&args);
    let transaction_message = sync::types::TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        action_hash: H256::default(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        trace_kind: None,
    };

    let outcome = sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    assert_eq!(runner.get_balance(&recipient_address), transfer_amount);
    assert_eq!(
        runner.get_balance(&caller_address),
        initial_balance - transfer_amount
    );
    assert_eq!(runner.get_nonce(&caller_address), U256::one());
}

#[test]
fn test_consume_submit_message() {
    let (mut runner, _) = initialize();

    let mut signer = utils::Signer::random();
    let initial_balance = Wei::new_u64(800_000);
    let transfer_amount = Wei::new_u64(115_321);
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let recipient_address = Address::new(H160([1u8; 20]));
    runner.mint_account(signer_address, initial_balance, signer.nonce.into(), None);

    runner.env.block_height += 1;
    utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = utils::standalone::mocks::compute_block_hash(runner.env.block_height);
    let transaction = utils::transfer(
        recipient_address,
        transfer_amount,
        signer.use_nonce().into(),
    );
    let signed_transaction =
        utils::sign_transaction(transaction, Some(runner.chain_id), &signer.secret_key);
    let eth_transaction =
        crate::prelude::transactions::EthTransactionKind::Legacy(signed_transaction);
    let tx_kind = sync::types::TransactionKind::submit(&eth_transaction);

    let transaction_message = sync::types::TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        action_hash: H256::default(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        trace_kind: None,
    };

    let outcome = sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    assert_eq!(runner.get_balance(&recipient_address), transfer_amount);
    assert_eq!(
        runner.get_balance(&signer_address),
        initial_balance - transfer_amount
    );
    assert_eq!(runner.get_nonce(&signer_address), U256::one());
}

fn simple_transfer_args(
    dest_address: Address,
    transfer_amount: Wei,
) -> aurora_engine::parameters::CallArgs {
    aurora_engine::parameters::CallArgs::V2(aurora_engine::parameters::FunctionCallArgsV2 {
        contract: dest_address,
        value: transfer_amount.to_bytes(),
        input: Vec::new(),
    })
}

fn sample_block() -> sync::types::BlockMessage {
    let block_height = 101;
    let block_hash = utils::standalone::mocks::compute_block_hash(block_height);

    sync::types::BlockMessage {
        height: block_height,
        hash: block_hash,
        metadata: engine_standalone_storage::BlockMetadata {
            timestamp: Timestamp::new(1_000_001),
            random_seed: H256([2u8; 32]),
        },
    }
}

fn initialize() -> (StandaloneRunner, sync::types::BlockMessage) {
    let mut runner = StandaloneRunner::default();
    runner.init_evm();

    let block_message = sample_block();
    sync::consume_message_wasmer::<false>(
        &mut runner.storage,
        sync::types::Message::Block(block_message.clone()),
    )
    .unwrap();

    let env = utils::standalone::mocks::default_env(block_message.height);
    runner.env = env;

    (runner, block_message)
}
