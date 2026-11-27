use aurora_engine::engine;
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::env::{Env, Timestamp};
use aurora_engine_types::parameters::connector::{self, BridgedToken, Nep245Token, TransferCall};
use aurora_engine_types::types::{Address, Balance, Wei};
use aurora_engine_types::{H160, H256, U256, account_id::AccountId, borsh};
use engine_standalone_storage::sync::{
    self, ConsumeMessageOutcome, TransactionExecutionResult,
    types::{Message, TransactionKind, TransactionMessage},
};

use crate::utils::solidity::erc20::{ERC20, ERC20Constructor};
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
    let tx_kind = TransactionKind::Deploy(input);
    let raw_input = tx_kind.raw_bytes();

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
        raw_input,
        action_hash: H256::default(),
    };

    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction_message)),
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
    let tx_kind = TransactionKind::DeployErc20(args);
    let raw_input = tx_kind.raw_bytes();
    let transaction_message = TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        raw_input,
        action_hash: H256::default(),
    };

    // Deploy ERC-20 (this would be the flow for bridging a new NEP-141 to Aurora)
    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction_message)),
    )
    .unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    let erc20_address = runner
        .storage
        .with_engine_access(runner.env.block_height + 1, 0, &[], |io| {
            engine::get_erc20_from_nep141(&io, &token)
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
    let tx_kind = TransactionKind::FtOnTransfer(args);
    let raw_input = tx_kind.raw_bytes();
    let transaction_message = TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        raw_input,
        action_hash: H256::default(),
    };

    // Mint new tokens (via ft_on_transfer flow, same as the bridge)
    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction_message)),
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
    let tx_kind = TransactionKind::FtOnTransfer(args);
    let raw_input = tx_kind.raw_bytes();
    let caller = utils::standalone::mocks::EXT_ETH_CONNECTOR.parse().unwrap();
    let transaction_message = TransactionMessage {
        block_hash: block_message.hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller,
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        raw_input,
        action_hash: H256::default(),
    };

    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction_message)),
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
    let caller_address = aurora_engine_sdk::types::near_account_to_evm_address(caller);
    let recipient_address = Address::new(H160([1u8; 20]));
    runner.mint_account(caller_address, initial_balance, U256::zero(), None);

    runner.env.block_height += 1;
    runner.env.signer_account_id = caller.parse().unwrap();
    runner.env.predecessor_account_id = caller.parse().unwrap();
    utils::standalone::mocks::insert_block(&mut runner.storage, runner.env.block_height);
    let block_hash = utils::standalone::mocks::compute_block_hash(runner.env.block_height);

    let tx_kind = TransactionKind::Call(simple_transfer_args(recipient_address, transfer_amount));
    let raw_input = tx_kind.raw_bytes();
    let transaction_message = TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        raw_input,
        action_hash: H256::default(),
    };

    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction_message)),
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
    let tx_kind = TransactionKind::Submit(eth_transaction);
    let raw_input = tx_kind.raw_bytes();

    let transaction_message = TransactionMessage {
        block_hash,
        near_receipt_id: H256([8u8; 32]),
        position: 0,
        succeeded: true,
        signer: runner.env.signer_account_id(),
        caller: runner.env.predecessor_account_id(),
        attached_near: 0,
        transaction: tx_kind,
        promise_data: Vec::new(),
        raw_input,
        action_hash: H256::default(),
    };

    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction_message)),
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
    sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Block(block_message.clone()),
    )
    .unwrap();

    let env = utils::standalone::mocks::default_env(block_message.height);
    runner.env = env;

    (runner, block_message)
}

const LEGACY_NEP141: &[u8] = b"\x09\x00\x00\x00wrap.near";
const TAGGED_NEP141: &[u8] = b"\x00\x09\x00\x00\x00wrap.near";
const TAGGED_NEP245: &[u8] = b"\x01\x0a\x00\x00\x00multi.near\x04\x00\x00\x00coin";
const CALLBACK: &str = "deploy_erc20_token_callback";

fn callback_cases() -> [(BridgedToken, &'static [u8], &'static [u8]); 3] {
    let nep141 = BridgedToken::Nep141("wrap.near".parse::<AccountId>().unwrap().into());
    let metadata141 = br#"{"name":"Wrapped NEAR","symbol":"wNEAR","decimals":24}"#;
    [
        (nep141.clone(), LEGACY_NEP141, metadata141),
        (nep141, TAGGED_NEP141, metadata141),
        (
            BridgedToken::Nep245(Nep245Token {
                account_id: "multi.near".parse().unwrap(),
                token_id: "coin".into(),
            }),
            TAGGED_NEP245,
            br#"[{"name":"Coin","symbol":"COIN","decimals":18}]"#,
        ),
    ]
}

fn token_message(
    runner: &mut StandaloneRunner,
    input: &[u8],
    transaction: TransactionKind,
) -> TransactionMessage {
    let mut message = StandaloneRunner::template_tx_msg(
        &mut runner.storage,
        &runner.env,
        1,
        H256([3; 32]),
        &[],
        input.to_vec(),
    );
    message.transaction = transaction;
    // Stored V3 messages omit action_hash; match the existing serialization fixtures.
    message.action_hash = H256::zero();
    message
}

#[test]
fn test_parse_callback_formats() {
    for (token, bytes, _) in callback_cases() {
        let kind = sync::parse_transaction_kind(CALLBACK, bytes.to_vec(), &[]).unwrap();
        assert_eq!(kind, TransactionKind::DeployErc20Callback(token));
    }

    let nep141 = TransactionKind::DeployErc20Callback(BridgedToken::Nep141(
        "wrap.near".parse::<AccountId>().unwrap().into(),
    ));
    assert_eq!(nep141.raw_bytes(), TAGGED_NEP141);
    let nep245 = sync::parse_transaction_kind(CALLBACK, TAGGED_NEP245.to_vec(), &[]).unwrap();
    assert_eq!(nep245.raw_bytes(), TAGGED_NEP245);
}

#[test]
fn test_parse_callback_rejects_malformed_or_trailing_data() {
    for bytes in [&[][..], &[2, 0, 0, 0][..]] {
        assert!(sync::parse_transaction_kind(CALLBACK, bytes.to_vec(), &[]).is_err());
    }
    for bytes in [LEGACY_NEP141, TAGGED_NEP141, TAGGED_NEP245] {
        let mut truncated = bytes.to_vec();
        truncated.pop();
        assert!(sync::parse_transaction_kind(CALLBACK, truncated, &[]).is_err());
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(sync::parse_transaction_kind(CALLBACK, trailing, &[]).is_err());
    }
}

#[test]
fn test_consume_callback_preserves_recorded_input() {
    for (token, input, metadata) in callback_cases() {
        let (mut runner, _) = initialize();
        let kind = sync::parse_transaction_kind(CALLBACK, input.to_vec(), &[]).unwrap();
        let mut transaction = token_message(&mut runner, input, kind);
        transaction.promise_data = vec![Some(metadata.to_vec())];

        let outcome = sync::consume_message::<AuroraModExp>(
            &mut runner.storage,
            Message::Transaction(Box::new(transaction.clone())),
        )
        .unwrap();
        let ConsumeMessageOutcome::TransactionIncluded(outcome) = outcome else {
            panic!("Callback should produce an included transaction");
        };
        let Some(TransactionExecutionResult::DeployErc20(address)) =
            outcome.maybe_result.as_ref().unwrap()
        else {
            panic!("Callback should deploy an ERC-20");
        };
        let address = *address;
        assert!(!outcome.diff.is_empty());
        outcome.commit(&mut runner.storage).unwrap();

        let stored = runner
            .storage
            .get_transaction_data(transaction.near_receipt_id)
            .unwrap();
        assert_eq!(stored, transaction);
        let mapped = runner
            .storage
            .with_engine_access(runner.env.block_height + 1, 0, &[], |io| {
                engine::get_erc20_from_nep(&io, &token)
            })
            .result
            .unwrap();
        assert_eq!(mapped, address);
        assert!(!runner.get_code(&address).is_empty());
    }
}

#[test]
fn test_legacy_callback_keeps_private_call_check() {
    let (mut runner, _) = initialize();
    let kind = sync::parse_transaction_kind(CALLBACK, LEGACY_NEP141.to_vec(), &[]).unwrap();
    let mut transaction = token_message(&mut runner, LEGACY_NEP141, kind);
    transaction.caller = "other.near".parse().unwrap();
    transaction.promise_data = vec![Some(callback_cases()[0].2.to_vec())];
    let outcome =
        sync::execute_transaction_message::<AuroraModExp>(&runner.storage, transaction).unwrap();

    assert!(outcome.maybe_result.is_err());
    assert!(outcome.diff.is_empty());
    outcome.commit(&mut runner.storage).unwrap();
    assert!(runner.storage.get_transaction_data(outcome.hash).is_err());
}

#[test]
fn test_mirror_erc20_token_callback_replay() {
    let prefix = b"\x09\x00\x00\x00main.near";
    let address = Address::from_array([0x44; 20]);
    let metadata = connector::Erc20Metadata {
        name: "Mirrored Coin".into(),
        symbol: "COIN".into(),
        decimals: 6,
    };
    for (token, suffix, _) in callback_cases() {
        let input = [prefix.as_slice(), suffix].concat();
        let kind = sync::parse_transaction_kind("mirror_erc20_token_callback", input.clone(), &[])
            .unwrap();
        assert_eq!(
            kind,
            TransactionKind::MirrorErc20TokenCallback(connector::MirrorErc20TokenArgs {
                contract_id: "main.near".parse().unwrap(),
                token: token.clone(),
            })
        );
        let (mut runner, _) = initialize();
        let mut transaction = token_message(&mut runner, &input, kind);
        transaction.promise_data = vec![
            Some(borsh::to_vec(&address).unwrap()),
            Some(serde_json::to_vec(&metadata).unwrap()),
        ];
        assert_eq!(
            TransactionMessage::try_from_slice(&transaction.to_bytes()).unwrap(),
            transaction
        );
        let outcome =
            sync::execute_transaction_message::<AuroraModExp>(&runner.storage, transaction.clone())
                .unwrap();
        assert!(outcome.maybe_result.is_ok());
        assert!(!outcome.diff.is_empty());
        outcome.commit(&mut runner.storage).unwrap();
        assert_eq!(
            runner
                .storage
                .get_transaction_data(transaction.near_receipt_id)
                .unwrap(),
            transaction
        );
        let mapped = runner
            .storage
            .with_engine_access(runner.env.block_height + 1, 0, &[], |io| {
                engine::get_erc20_from_nep(&io, &token)
            })
            .result
            .unwrap();
        assert_eq!(mapped, address);
    }
}

const MT_ON_TRANSFER: &str = "mt_on_transfer";
const MT_AMOUNT: u128 = 1_000;
const MT_RECIPIENT: Address = Address::from_array([0x11; 20]);

fn deploy_nep245_erc20(runner: &mut StandaloneRunner) -> Address {
    let [_, _, (token, input, metadata)] = callback_cases();
    let kind = sync::parse_transaction_kind(CALLBACK, input.to_vec(), &[]).unwrap();
    let mut transaction = token_message(runner, input, kind);
    transaction.promise_data = vec![Some(metadata.to_vec())];
    let outcome =
        sync::execute_transaction_message::<AuroraModExp>(&runner.storage, transaction).unwrap();
    outcome.commit(&mut runner.storage).unwrap();

    runner
        .storage
        .with_engine_access(runner.env.block_height + 1, 0, &[], |io| {
            engine::get_erc20_from_nep(&io, &token)
        })
        .result
        .unwrap()
}

#[test]
fn test_exit_refund_replay() {
    let callback = "exit_to_near_precompile_callback";
    for (transfer_call, result, refund) in [
        (TransferCall::None, Some(b"".as_slice()), 0u128),
        (TransferCall::None, None, 100),
        (TransferCall::Nep141, Some(br#""0""#.as_slice()), 100),
        (TransferCall::Nep141, Some(br#""60""#.as_slice()), 40),
        (TransferCall::Nep141, Some(br#""100""#.as_slice()), 0),
        (TransferCall::Nep245, Some(br#"["0"]"#.as_slice()), 100),
        (TransferCall::Nep245, Some(br#"["60"]"#.as_slice()), 40),
        (TransferCall::Nep245, Some(br#"["100"]"#.as_slice()), 0),
    ] {
        let (mut runner, _) = initialize();
        let erc20 = deploy_nep245_erc20(&mut runner);
        let args = connector::ExitToNearPrecompileCallbackArgs {
            refund: Some(connector::RefundCallArgs {
                recipient_address: MT_RECIPIENT,
                erc20_address: Some(erc20),
                amount: U256::from(100).to_big_endian(),
            }),
            transfer_near: None,
            transfer_call,
        };
        let input = borsh::to_vec(&args).unwrap();
        let promise_data = vec![result.map(<[u8]>::to_vec)];
        let kind = sync::parse_transaction_kind(callback, input.clone(), &promise_data).unwrap();
        assert_eq!(kind.raw_bytes(), input);
        let account = runner.env.current_account_id();
        let representation = kind.clone().eth_repr(
            &account,
            &account,
            runner.env.block_height,
            2,
            &runner.storage,
        );
        if refund == 0 {
            assert_eq!(representation.data, callback.as_bytes());
        } else {
            assert_eq!(representation.to, Some(erc20));
            assert_eq!(
                representation.data,
                engine::setup_refund_on_error_input(U256::from(refund), MT_RECIPIENT)
            );
        }
        let mut transaction = token_message(&mut runner, &input, kind);
        transaction.position = 2;
        transaction.near_receipt_id = H256([4; 32]);
        transaction.caller = account;
        transaction.promise_data = promise_data;
        assert_eq!(
            TransactionMessage::try_from_slice(&transaction.to_bytes()).unwrap(),
            transaction
        );
        let outcome =
            sync::execute_transaction_message::<AuroraModExp>(&runner.storage, transaction.clone())
                .unwrap();
        if refund == 0 {
            assert!(outcome.maybe_result.as_ref().unwrap().is_none());
            assert!(outcome.diff.is_empty());
        } else {
            let Some(TransactionExecutionResult::Submit(Ok(result))) =
                outcome.maybe_result.as_ref().unwrap()
            else {
                panic!("expected refund")
            };
            assert!(result.status.is_ok());
            assert!(result.logs.iter().any(|log| log.address == erc20
                && U256::from_big_endian(&log.data) == U256::from(refund)));
        }
        outcome.commit(&mut runner.storage).unwrap();
        assert_eq!(
            runner
                .storage
                .get_transaction_data(transaction.near_receipt_id)
                .unwrap(),
            transaction
        );
    }
}

#[test]
fn test_transfer_call_failed_result_has_no_refund() {
    let callback = "exit_to_near_precompile_callback";
    let (mut runner, _) = initialize();
    let erc20 = deploy_nep245_erc20(&mut runner);
    let mut args = connector::ExitToNearPrecompileCallbackArgs {
        refund: Some(connector::RefundCallArgs {
            recipient_address: MT_RECIPIENT,
            erc20_address: Some(erc20),
            amount: U256::from(100).to_big_endian(),
        }),
        transfer_near: None,
        transfer_call: connector::TransferCall::Nep245,
    };
    for (transfer_call, result) in [
        (TransferCall::Nep141, None),
        (TransferCall::Nep141, Some(br#""101""#.as_slice())),
        (TransferCall::Nep141, Some(br#"["0"]"#.as_slice())),
        (TransferCall::Nep245, None),
        (TransferCall::Nep245, Some(br#"["101"]"#.as_slice())),
        (TransferCall::Nep245, Some(br#"["0","0"]"#.as_slice())),
    ] {
        args.transfer_call = transfer_call;
        let input = borsh::to_vec(&args).unwrap();
        let promise_data = vec![result.map(<[u8]>::to_vec)];
        let kind = sync::parse_transaction_kind(callback, input.clone(), &promise_data).unwrap();
        let account = runner.env.current_account_id();
        let representation = kind.clone().eth_repr(
            &account,
            &account,
            runner.env.block_height,
            2,
            &runner.storage,
        );
        assert_eq!(representation.data, callback.as_bytes());
        let mut transaction = token_message(&mut runner, &input, kind);
        transaction.position = 2;
        transaction.caller = account;
        transaction.promise_data = promise_data;
        let outcome =
            sync::execute_transaction_message::<AuroraModExp>(&runner.storage, transaction)
                .unwrap();
        assert!(outcome.maybe_result.is_err());
        assert!(outcome.diff.is_empty());
    }
}

#[test]
fn test_exit_callback_requires_one_promise_result() {
    let input = borsh::to_vec(&connector::ExitToNearPrecompileCallbackArgs::default()).unwrap();
    for results in [vec![], vec![None, None]] {
        assert!(
            sync::parse_transaction_kind(
                "exit_to_near_precompile_callback",
                input.clone(),
                &results
            )
            .is_err()
        );
    }
}

fn mt_on_transfer_args(token_ids: &[&str], msg: &str) -> connector::MtOnTransferArgs {
    connector::MtOnTransferArgs {
        sender_id: "sender.near".parse().unwrap(),
        previous_owner_ids: vec!["sender.near".parse().unwrap(); token_ids.len()],
        token_ids: token_ids.iter().map(ToString::to_string).collect(),
        amounts: vec![Balance::new(MT_AMOUNT); token_ids.len()],
        msg: msg.into(),
    }
}

#[test]
fn test_mt_on_transfer_replay_mints_bridged_token() {
    let (mut runner, _) = initialize();
    let erc20 = deploy_nep245_erc20(&mut runner);
    let args = mt_on_transfer_args(&["coin"], &MT_RECIPIENT.encode());
    let input = serde_json::to_vec(&args).unwrap();
    let kind = sync::parse_transaction_kind(MT_ON_TRANSFER, input.clone(), &[]).unwrap();
    assert_eq!(kind, TransactionKind::MtOnTransfer(args));
    assert_eq!(kind.raw_bytes(), input);
    let mut transaction = token_message(&mut runner, &input, kind);
    transaction.near_receipt_id = H256([4; 32]);
    transaction.position = 2;
    transaction.caller = "multi.near".parse().unwrap();

    let outcome = sync::consume_message::<AuroraModExp>(
        &mut runner.storage,
        Message::Transaction(Box::new(transaction.clone())),
    )
    .unwrap();
    let ConsumeMessageOutcome::TransactionIncluded(outcome) = outcome else {
        panic!("Deposit should produce an included transaction");
    };
    let Some(TransactionExecutionResult::Submit(Ok(result))) =
        outcome.maybe_result.as_ref().unwrap()
    else {
        panic!("Deposit should mint the bridged token");
    };
    assert!(result.status.is_ok());
    let mut recipient_topic = [0; 32];
    recipient_topic[12..].copy_from_slice(MT_RECIPIENT.as_bytes());
    assert!(
        result
            .logs
            .iter()
            .any(|log| log.address == erc20 && log.topics.get(2) == Some(&recipient_topic))
    );
    outcome.commit(&mut runner.storage).unwrap();

    let stored = runner
        .storage
        .get_transaction_data(transaction.near_receipt_id)
        .unwrap();
    assert_eq!(stored, transaction);
}

#[test]
fn test_mt_on_transfer_eth_repr() {
    let (mut runner, _) = initialize();
    let erc20 = deploy_nep245_erc20(&mut runner);
    let engine_account = runner.env.current_account_id();
    let caller: AccountId = "multi.near".parse().unwrap();
    let eth_repr = |args| {
        TransactionKind::MtOnTransfer(args).eth_repr(
            &engine_account,
            &caller,
            runner.env.block_height,
            2,
            &runner.storage,
        )
    };

    let msg = format!("0x{}", MT_RECIPIENT.encode());
    let mint = eth_repr(mt_on_transfer_args(&["coin"], &msg));
    assert_eq!(
        mint.address,
        aurora_engine_sdk::types::near_account_to_evm_address(&engine_account)
    );
    assert_eq!(mint.to, Some(erc20));
    assert!(mint.value.is_zero());
    assert_eq!(
        mint.data,
        engine::setup_receive_erc20_tokens_input(&MT_RECIPIENT, MT_AMOUNT)
    );

    let mut misaligned = mt_on_transfer_args(&["coin"], &msg);
    misaligned.amounts.clear();
    for args in [
        misaligned,
        mt_on_transfer_args(&["coin", "coin"], &msg),
        mt_on_transfer_args(&["unknown"], &msg),
        mt_on_transfer_args(&["coin"], "0x1234"),
    ] {
        assert_eq!(eth_repr(args).data, MT_ON_TRANSFER.as_bytes());
    }
}
