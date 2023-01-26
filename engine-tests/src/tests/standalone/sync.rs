use aurora_engine::deposit_event::TokenMessageData;
use aurora_engine_sdk::env::{Env, Timestamp};
use aurora_engine_types::types::{Address, Balance, Fee, NEP141Wei, Wei};
use aurora_engine_types::{account_id::AccountId, H160, H256, U256};
use borsh::{BorshDeserialize, BorshSerialize};
use engine_standalone_storage::sync;

use crate::test_utils::{self, standalone::StandaloneRunner};

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

    runner.close()
}

#[test]
fn test_consume_deposit_message() {
    let (mut runner, block_message) = initialize();

    let recipient_address = Address::new(H160([22u8; 20]));
    let deposit_amount = Wei::new_u64(123_456_789);
    let proof = mock_proof(recipient_address, deposit_amount);

    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([0x11; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::Deposit(proof.try_to_vec().unwrap()),
        promise_data: Vec::new(),
    };

    let outcome = sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    let outcome = match outcome {
        sync::ConsumeMessageOutcome::TransactionIncluded(outcome) => outcome,
        other => panic!("Unexpected outcome {:?}", other),
    };

    let finish_deposit_args = match outcome.maybe_result.unwrap().unwrap() {
        sync::TransactionExecutionResult::Promise(promise_args) => {
            let bytes = promise_args.callback.args;
            aurora_engine::parameters::FinishDepositCallArgs::try_from_slice(&bytes).unwrap()
        }
        other => panic!("Unexpected result {:?}", other),
    };
    // Now executing aurora callbacks, so predecessor_account_id = current_account_id
    runner.env.predecessor_account_id = runner.env.current_account_id.clone();

    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([0x22; 32]),
        position: 1,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::FinishDeposit(finish_deposit_args),
        promise_data: Vec::new(),
    };

    let outcome = sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    let outcome = match outcome {
        sync::ConsumeMessageOutcome::TransactionIncluded(outcome) => outcome,
        other => panic!("Unexpected outcome {:?}", other),
    };

    let ft_on_transfer_args = match outcome.maybe_result.unwrap().unwrap() {
        sync::TransactionExecutionResult::Promise(promise_args) => {
            let bytes = promise_args.base.args;
            let json = aurora_engine::json::parse_json(&bytes).unwrap();
            aurora_engine::parameters::NEP141FtOnTransferArgs::try_from(json)
                .ok()
                .unwrap()
        }
        other => panic!("Unexpected result {:?}", other),
    };

    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([0x33; 32]),
        position: 2,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::FtOnTransfer(ft_on_transfer_args),
        promise_data: Vec::new(),
    };

    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

    assert_eq!(runner.get_balance(&recipient_address), deposit_amount);

    runner.close()
}

#[test]
fn test_consume_deploy_message() {
    let (mut runner, block_message) = initialize();

    let code = b"hello_world!".to_vec();
    let input = test_utils::create_deploy_transaction(code.clone(), U256::zero()).data;

    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::Deploy(input),
        promise_data: Vec::new(),
    };

    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

    let diff = runner
        .storage
        .get_transaction_diff(engine_standalone_storage::TransactionIncluded {
            block_hash: block_message.hash,
            position: 0,
        })
        .unwrap();
    let mut deployed_address = Address::zero();
    for (key, value) in diff.iter() {
        match value.value() {
            Some(bytes) if bytes == code.as_slice() => {
                deployed_address = Address::try_from_slice(&key[2..22]).unwrap();
                break;
            }
            _ => continue,
        }
    }

    assert_eq!(runner.get_code(&deployed_address), code);

    runner.close()
}

#[test]
fn test_consume_deploy_erc20_message() {
    let (mut runner, block_message) = initialize();

    let token: AccountId = "some_nep141.near".parse().unwrap();
    let mint_amount: u128 = 555_555;
    let dest_address = Address::new(H160([170u8; 20]));

    let args = aurora_engine::parameters::DeployErc20TokenArgs {
        nep141: token.clone(),
    };
    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::DeployErc20(args),
        promise_data: Vec::new(),
    };

    // Deploy ERC-20 (this would be the flow for bridging a new NEP-141 to Aurora)
    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

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
    test_utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = test_utils::standalone::mocks::compute_block_hash(runner.env.block_height);

    let args = aurora_engine::parameters::NEP141FtOnTransferArgs {
        sender_id: "mr_money_bags.near".parse().unwrap(),
        amount: Balance::new(mint_amount),
        msg: hex::encode(dest_address.as_bytes()),
    };
    let transaction_message = sync::types::TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::FtOnTransfer(args),
        promise_data: Vec::new(),
    };

    // Mint new tokens (via ft_on_transfer flow, same as the bridge)
    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

    // Check balance is correct
    let deployed_token = test_utils::erc20::ERC20(
        test_utils::erc20::ERC20Constructor::load()
            .0
            .deployed_at(Address::try_from_slice(&erc20_address).unwrap()),
    );
    let signer = test_utils::Signer::random();
    let tx = deployed_token.balance_of(dest_address, signer.nonce.into());
    let result = runner.submit_transaction(&signer.secret_key, tx).unwrap();
    assert_eq!(
        U256::from_big_endian(&test_utils::unwrap_success(result)).low_u128(),
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
    let args = aurora_engine::parameters::NEP141FtOnTransferArgs {
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
    let transaction_message = sync::types::TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::FtOnTransfer(args),
        promise_data: Vec::new(),
    };

    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

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
    test_utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = test_utils::standalone::mocks::compute_block_hash(runner.env.block_height);

    let transaction_message = sync::types::TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::Call(simple_transfer_args(
            recipient_address,
            transfer_amount,
        )),
        promise_data: Vec::new(),
    };

    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

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

    let mut signer = test_utils::Signer::random();
    let initial_balance = Wei::new_u64(800_000);
    let transfer_amount = Wei::new_u64(115_321);
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
    let recipient_address = Address::new(H160([1u8; 20]));
    runner.mint_account(signer_address, initial_balance, signer.nonce.into(), None);

    runner.env.block_height += 1;
    test_utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = test_utils::standalone::mocks::compute_block_hash(runner.env.block_height);
    let transaction = test_utils::transfer(
        recipient_address,
        transfer_amount,
        signer.use_nonce().into(),
    );
    let signed_transaction =
        test_utils::sign_transaction(transaction, Some(runner.chain_id), &signer.secret_key);
    let eth_transaction =
        crate::prelude::transactions::EthTransactionKind::Legacy(signed_transaction);

    let transaction_message = sync::types::TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: sync::types::TransactionKind::Submit(eth_transaction),
        promise_data: Vec::new(),
    };

    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();

    assert_eq!(runner.get_balance(&recipient_address), transfer_amount);
    assert_eq!(
        runner.get_balance(&signer_address),
        initial_balance - transfer_amount
    );
    assert_eq!(runner.get_nonce(&signer_address), U256::one());
}

fn mock_proof(recipient_address: Address, deposit_amount: Wei) -> aurora_engine::proof::Proof {
    let eth_custodian_address = test_utils::standalone::mocks::ETH_CUSTODIAN_ADDRESS;

    let fee = Fee::new(NEP141Wei::new(0));
    let message = ["aurora", ":", recipient_address.encode().as_str()].concat();
    let token_message_data: TokenMessageData =
        TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
            .unwrap();

    let deposit_event = aurora_engine::deposit_event::DepositedEvent {
        eth_custodian_address,
        sender: Address::new(H160([0u8; 20])),
        token_message_data,
        amount: NEP141Wei::new(deposit_amount.raw().as_u128()),
        fee,
    };

    let event_schema = ethabi::Event {
        name: aurora_engine::deposit_event::DEPOSITED_EVENT.into(),
        inputs: aurora_engine::deposit_event::DepositedEvent::event_params(),
        anonymous: false,
    };
    let log_entry = aurora_engine::log_entry::LogEntry {
        address: eth_custodian_address.raw(),
        topics: vec![
            event_schema.signature(),
            // the sender is not important
            crate::prelude::H256::zero(),
        ],
        data: ethabi::encode(&[
            ethabi::Token::String(message),
            ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
            ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
        ]),
    };
    aurora_engine::proof::Proof {
        log_index: 1,
        // Only this field matters for the purpose of this test
        log_entry_data: rlp::encode(&log_entry).to_vec(),
        receipt_index: 1,
        receipt_data: Vec::new(),
        header_data: Vec::new(),
        proof: Vec::new(),
    }
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
    let block_hash = test_utils::standalone::mocks::compute_block_hash(block_height);

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
    sync::consume_message(
        &mut runner.storage,
        sync::types::Message::Block(block_message.clone()),
    )
    .unwrap();

    let env = test_utils::standalone::mocks::default_env(block_message.height);
    runner.env = env;

    (runner, block_message)
}
