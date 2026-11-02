use super::*;
use crate::contract_methods::silo;
use crate::engine::Engine;
use crate::parameters::{TransactionStatus, ViewCallArgs};
use crate::state::EngineState;
use aurora_engine_sdk::env::Fixed;
use aurora_engine_sdk::promise::Noop;
use aurora_engine_test_doubles::io::{Storage, StoragePointer};
use aurora_engine_test_doubles::promise::PromiseTracker;
use aurora_engine_types::U256;
use aurora_engine_types::parameters::connector::{
    BridgedToken, Erc20Metadata, Nep245Token, RefundCallArgs,
};
use aurora_engine_types::parameters::silo::{WhitelistKind, WhitelistStatusArgs};
use aurora_engine_types::types::{Balance, WeiU256, make_address};
use std::cell::RefCell;

fn test_env() -> Fixed {
    Fixed {
        signer_account_id: "sender.near".parse().unwrap(),
        current_account_id: "aurora.near".parse().unwrap(),
        predecessor_account_id: "multi.near".parse().unwrap(),
        ..Fixed::default()
    }
}

/// Deploys ERC-20 tokens for two NEP-245 tokens of the predecessor contract.
fn deploy_tokens(mut io: StoragePointer<'_>, env: &Fixed) -> [Address; 2] {
    state::set_state(&mut io, &EngineState::default()).unwrap();
    ["first.token", "second.token"].map(|token_id| {
        let token = BridgedToken::Nep245(Nep245Token {
            account_id: env.predecessor_account_id(),
            token_id: token_id.into(),
        });
        engine::deploy_erc20_token(token, None, io, env, &mut Noop).unwrap()
    })
}

fn balance_of(io: StoragePointer<'_>, env: &Fixed, token: Address, recipient: Address) -> U256 {
    let engine: Engine<_, _> =
        Engine::new(Address::zero(), env.current_account_id(), io, env).unwrap();
    let mut input = sdk::keccak(b"balanceOf(address)").as_bytes()[..4].to_vec();
    input.extend(ethabi::encode(&[ethabi::Token::Address(
        recipient.raw().0.into(),
    )]));
    let status = engine
        .view_with_args(ViewCallArgs {
            sender: Address::zero(),
            address: token,
            amount: WeiU256::default(),
            input,
        })
        .unwrap();
    let TransactionStatus::Succeed(output) = status else {
        panic!("Unexpected balanceOf result: {status:?}");
    };
    U256::from_big_endian(&output)
}

fn mt_args(env: &Fixed, recipient: Address, amount: u128) -> MtOnTransferArgs {
    MtOnTransferArgs {
        sender_id: env.signer_account_id(),
        previous_owner_ids: vec!["owner.near".parse().unwrap()],
        token_ids: vec!["first.token".into()],
        amounts: vec![Balance::new(amount)],
        msg: format!("0x{}", recipient.encode()),
    }
}

fn transfer_call_callback_args(
    token: Address,
    recipient: Address,
) -> ExitToNearPrecompileCallbackArgs {
    ExitToNearPrecompileCallbackArgs {
        refund: Some(RefundCallArgs {
            recipient_address: recipient,
            erc20_address: Some(token),
            amount: U256::from(100).to_big_endian(),
        }),
        transfer_near: None,
        transfer_call: TransferCall::Nep245,
    }
}

#[test_case::test_case(TransferCall::Nep141, 0, 100; "ft_full_refund")]
#[test_case::test_case(TransferCall::Nep141, 60, 40; "ft_partial_refund")]
#[test_case::test_case(TransferCall::Nep141, 100, 0; "ft_fully_used")]
#[test_case::test_case(TransferCall::Nep245, 0, 100; "mt_full_refund")]
#[test_case::test_case(TransferCall::Nep245, 60, 40; "mt_partial_refund")]
#[test_case::test_case(TransferCall::Nep245, 100, 0; "mt_fully_used")]
fn test_transfer_call_callback_refunds_only_unused_tokens(
    transfer_call: TransferCall,
    used: u128,
    unused: u128,
) {
    let mut env = test_env();
    let storage = RefCell::new(Storage::default());
    let io = StoragePointer(&storage);
    let [token, other] = deploy_tokens(io, &env);
    let recipient = make_address(6, 6);
    env.predecessor_account_id = env.current_account_id();
    let args = ExitToNearPrecompileCallbackArgs {
        transfer_call,
        ..transfer_call_callback_args(token, recipient)
    };
    storage.borrow_mut().input = borsh::to_vec(&args).unwrap();
    let mut handler = PromiseTracker::default();
    let result = if transfer_call == TransferCall::Nep141 {
        format!(r#""{used}""#)
    } else {
        format!(r#"["{used}"]"#)
    };
    handler
        .promise_results
        .push(PromiseResult::Successful(result.into_bytes()));

    let result = exit_to_near_precompile_callback(io, &env, &mut handler).unwrap();

    assert_eq!(result.is_some(), unused > 0);
    assert_eq!(balance_of(io, &env, token, recipient), U256::from(unused));
    assert_eq!(balance_of(io, &env, other, recipient), U256::zero());
    assert!(handler.scheduled_promises.is_empty());
}

#[test_case::test_case(TransferCall::Nep141; "ft")]
#[test_case::test_case(TransferCall::Nep245; "mt")]
fn test_transfer_call_callback_handles_unconfirmed_refunds(transfer_call: TransferCall) {
    let storage = RefCell::new(Storage::default());
    let mut io = StoragePointer(&storage);
    let mut env = test_env();
    env.predecessor_account_id = env.current_account_id();
    state::set_state(&mut io, &EngineState::default()).unwrap();
    storage.borrow_mut().input = borsh::to_vec(&ExitToNearPrecompileCallbackArgs {
        transfer_call,
        ..transfer_call_callback_args(make_address(1, 1), make_address(2, 2))
    })
    .unwrap();
    let mut results = vec![PromiseResult::Failed, PromiseResult::NotReady];
    for bytes in [
        b"".as_slice(),
        b"[]",
        br#"["0","0"]"#,
        br#"["101"]"#,
        br#"["-1"]"#,
        br#""-1""#,
        br#""101""#,
        b"[null]",
        b"[60]",
        b"60",
        br#"["340282366920938463463374607431768211456"]"#,
        br#""340282366920938463463374607431768211456""#,
        br#"["0"] trailing"#,
        br#""0" trailing"#,
    ] {
        results.push(PromiseResult::Successful(bytes.to_vec()));
    }
    let wrong_shape = if transfer_call == TransferCall::Nep141 {
        br#"["60"]"#.as_slice()
    } else {
        br#""60""#.as_slice()
    };
    results.push(PromiseResult::Successful(wrong_shape.to_vec()));
    results.push(PromiseResult::Successful(vec![b' '; 257]));
    let before = storage.borrow().kv_store.clone();
    for result in results {
        let mut handler = PromiseTracker::default();
        handler.promise_results.push(result);
        let result = exit_to_near_precompile_callback(io, &env, &mut handler);
        assert!(result.is_err());
        assert_eq!(storage.borrow().kv_store, before);
        assert!(handler.scheduled_promises.is_empty());
    }
}

#[test]
fn test_transfer_call_callback_keeps_authorization_and_promise_checks() {
    let storage = RefCell::new(Storage::default());
    let mut io = StoragePointer(&storage);
    let mut env = test_env();
    state::set_state(&mut io, &EngineState::default()).unwrap();
    storage.borrow_mut().input = borsh::to_vec(&transfer_call_callback_args(
        make_address(1, 1),
        make_address(2, 2),
    ))
    .unwrap();
    let mut handler = PromiseTracker::default();
    handler
        .promise_results
        .push(PromiseResult::Successful(br#"["0"]"#.to_vec()));
    let before = storage.borrow().kv_store.clone();
    let error = exit_to_near_precompile_callback(io, &env, &mut handler).unwrap_err();
    assert_eq!(error.message.as_ref().as_ref(), b"ERR_PRIVATE_CALL");
    env.predecessor_account_id = env.current_account_id();
    for count in [0, 2] {
        handler.promise_results = vec![PromiseResult::Failed; count];
        let error = exit_to_near_precompile_callback(io, &env, &mut handler).unwrap_err();
        assert_eq!(error.message.as_ref().as_ref(), errors::ERR_PROMISE_COUNT);
    }
    assert_eq!(storage.borrow().kv_store, before);
}

#[test]
fn test_exit_refund_amount_boundary_and_legacy_behavior() {
    let refund = RefundCallArgs {
        recipient_address: make_address(1, 1),
        erc20_address: None,
        amount: U256::from(u128::MAX).to_big_endian(),
    };
    for (transfer_call, result) in [
        (TransferCall::Nep141, format!(r#""{}""#, u128::MAX - 1)),
        (TransferCall::Nep245, format!(r#"["{}"]"#, u128::MAX - 1)),
    ] {
        let args = ExitToNearPrecompileCallbackArgs {
            refund: Some(refund.clone()),
            transfer_call,
            ..Default::default()
        };
        let result = PromiseResult::Successful(result.into_bytes());
        assert_eq!(
            resolve_exit_to_near_refund(&args, Some(&result))
                .unwrap()
                .unwrap()
                .amount,
            U256::from(1).to_big_endian()
        );
        assert!(resolve_exit_to_near_refund(&args, None).is_err());
        let oversized = ExitToNearPrecompileCallbackArgs {
            refund: Some(RefundCallArgs {
                amount: (U256::from(u128::MAX) + U256::one()).to_big_endian(),
                ..refund.clone()
            }),
            ..args
        };
        assert!(resolve_exit_to_near_refund(&oversized, Some(&result)).is_err());
    }
    let args = ExitToNearPrecompileCallbackArgs {
        refund: Some(refund.clone()),
        transfer_near: None,
        ..Default::default()
    };
    assert_eq!(
        resolve_exit_to_near_refund(&args, Some(&PromiseResult::Failed)).unwrap(),
        Some(refund)
    );
    assert_eq!(
        resolve_exit_to_near_refund(&args, Some(&PromiseResult::Successful(Vec::new()))).unwrap(),
        None
    );
    assert!(resolve_exit_to_near_refund(&args, None).is_err());
    assert!(resolve_exit_to_near_refund(&args, Some(&PromiseResult::NotReady)).is_err());

    // Without `error_refund`, the precompile omits refund arguments, so no result is settled.
    for transfer_call in [
        TransferCall::None,
        TransferCall::Nep141,
        TransferCall::Nep245,
    ] {
        let args = ExitToNearPrecompileCallbackArgs {
            transfer_call,
            ..Default::default()
        };
        for result in [None, Some(&PromiseResult::Failed)] {
            assert_eq!(resolve_exit_to_near_refund(&args, result).unwrap(), None);
        }
    }
}

#[test_case::test_case(11, false; "regular_amount")]
#[test_case::test_case(0, false; "zero_amount")]
#[test_case::test_case(u128::MAX, false; "max_amount")]
#[test_case::test_case(11, true; "fallback_recipient")]
fn test_mt_on_transfer_mints_token(amount: u128, use_fallback: bool) {
    let env = test_env();
    let storage = RefCell::new(Storage::default());
    let mut io = StoragePointer(&storage);
    let [first, second] = deploy_tokens(io, &env);
    let recipient = make_address(6, 6);
    let fallback = make_address(7, 7);
    if use_fallback {
        silo::set_erc20_fallback_address(&mut io, Some(fallback));
        silo::set_whitelist_status(
            &io,
            &WhitelistStatusArgs {
                kind: WhitelistKind::Address,
                active: true,
            },
        );
    }
    storage.borrow_mut().input = serde_json::to_vec(&mt_args(&env, recipient, amount)).unwrap();

    let result = mt_on_transfer(io, &env, &mut Noop).unwrap().unwrap();

    assert_eq!(storage.borrow().output, br#"["0"]"#);
    assert!(matches!(result.status, TransactionStatus::Succeed(_)));
    assert!(result.logs.iter().all(|log| log.address == first));
    let destination = if use_fallback { fallback } else { recipient };
    assert_eq!(balance_of(io, &env, first, destination), U256::from(amount));
    assert_eq!(balance_of(io, &env, second, destination), U256::zero());
    if use_fallback {
        assert_eq!(balance_of(io, &env, first, recipient), U256::zero());
    }
}

#[test]
fn test_mt_on_transfer_unknown_token_does_not_mint() {
    let env = test_env();
    let storage = RefCell::new(Storage::default());
    let io = StoragePointer(&storage);
    deploy_tokens(io, &env);
    let mut args = mt_args(&env, make_address(6, 6), 11);
    args.token_ids[0] = "unregistered.token".into();
    storage.borrow_mut().input = serde_json::to_vec(&args).unwrap();
    let before = storage.borrow().kv_store.clone();

    let error = mt_on_transfer(io, &env, &mut Noop).unwrap_err();

    assert_eq!(
        error.message.as_ref().as_ref(),
        errors::ERR_NEP245_NOT_FOUND
    );
    assert_eq!(storage.borrow().kv_store, before);
}

#[test]
fn test_mt_on_transfer_propagates_mint_failure() {
    let env = test_env();
    let storage = RefCell::new(Storage::default());
    let mut io = StoragePointer(&storage);
    let [first, _] = deploy_tokens(io, &env);
    // PUSH0 PUSH0 REVERT makes the mint fail.
    engine::set_code(&mut io, &first, &[0x5f, 0x5f, 0xfd]);
    storage.borrow_mut().input =
        serde_json::to_vec(&mt_args(&env, make_address(6, 6), 11)).unwrap();

    // The WASM export must receive this error to abort the receipt, so the resolver
    // refunds the token.
    let error = mt_on_transfer(io, &env, &mut Noop).unwrap_err();
    assert_eq!(error.message.as_ref().as_ref(), b"Reverted with message: ");
    assert!(storage.borrow().output.is_empty());
}

#[test_case::test_case(0, 0, 0; "empty")]
#[test_case::test_case(2, 2, 2; "batch")]
#[test_case::test_case(1, 0, 1; "missing_amount")]
#[test_case::test_case(1, 2, 1; "extra_amount")]
#[test_case::test_case(1, 1, 0; "missing_owner")]
fn test_mt_on_transfer_rejects_batch_or_malformed_args(
    tokens: usize,
    amounts: usize,
    owners: usize,
) {
    let env = test_env();
    let storage = RefCell::new(Storage::default());
    let io = StoragePointer(&storage);
    // Both tokens are registered, so a batch is rejected only because of its size.
    deploy_tokens(io, &env);
    let args = MtOnTransferArgs {
        sender_id: "sender.near".parse().unwrap(),
        previous_owner_ids: vec!["owner.near".parse().unwrap(); owners],
        token_ids: ["first.token", "second.token"]
            .into_iter()
            .cycle()
            .take(tokens)
            .map(Into::into)
            .collect(),
        amounts: vec![Balance::new(1); amounts],
        msg: make_address(6, 6).encode().to_string(),
    };
    storage.borrow_mut().input = serde_json::to_vec(&args).unwrap();
    let before = storage.borrow().kv_store.clone();

    let error = mt_on_transfer(io, &env, &mut Noop).unwrap_err();

    assert_eq!(error.message.as_ref().as_ref(), errors::ERR_ARGS.as_bytes());
    assert_eq!(storage.borrow().kv_store, before);
}

#[test_case::test_case("0x1234"; "too_short")]
#[test_case::test_case("gggggggggggggggggggggggggggggggggggggggg"; "invalid_hex")]
#[test_case::test_case("111111111111111111111111111111111111111é"; "non_ascii")]
fn test_mt_on_transfer_rejects_invalid_recipient(message: &str) {
    let env = test_env();
    let storage = RefCell::new(Storage::default());
    let mut io = StoragePointer(&storage);
    state::set_state(&mut io, &EngineState::default()).unwrap();
    let mut args = mt_args(&env, make_address(6, 6), 11);
    args.msg = message.into();
    storage.borrow_mut().input = serde_json::to_vec(&args).unwrap();

    let error = mt_on_transfer(io, &env, &mut Noop).unwrap_err();

    assert_eq!(
        error.message.as_ref().as_ref(),
        b"ERR_INVALID_ON_TRANSFER_MESSAGE_DATA"
    );
}

/// Full NEP-148 metadata, including fields that `Erc20Metadata` doesn't have.
const NEP148_METADATA: &str = r#"{
    "spec": "ft-1.0.0",
    "name": "Wrapped NEAR",
    "symbol": "wNEAR",
    "icon": "data:image/svg+xml,%3Csvg%3E%3C/svg%3E",
    "reference": "https://example.com/wnear.json",
    "reference_hash": "AK3YRHqKhCJNmKfV6SrutnlWW/icN5J8NUPtKsNXR1M=",
    "decimals": 24
}"#;
const NEP245_METADATA: &str = r#"[{"name":"Wrapped NEAR","symbol":"wNEAR","decimals":24}]"#;

fn nep141_token() -> BridgedToken {
    BridgedToken::Nep141("wrap.near".parse::<AccountId>().unwrap().into())
}

fn nep245_token() -> BridgedToken {
    BridgedToken::Nep245(Nep245Token {
        account_id: "multi.near".parse().unwrap(),
        token_id: "wnear".into(),
    })
}

fn schedule_deploy_erc20_token(
    io: StoragePointer<'_>,
    env: &Fixed,
    token: &BridgedToken,
) -> PromiseWithCallbackArgs {
    let args = match token.clone() {
        BridgedToken::Nep141(token) => DeployErc20TokenArgs::WithMetadata(token.0),
        BridgedToken::Nep245(token) => DeployErc20TokenArgs::Nep245(token),
    };
    io.0.borrow_mut().input = borsh::to_vec(&args).unwrap();
    let result = deploy_erc20_token(io, env, &mut PromiseTracker::default()).unwrap();
    let PromiseOrValue::Promise(promise) = result else {
        panic!("Expected a metadata promise");
    };
    promise
}

/// Runs the callback with the input scheduled by `deploy_erc20_token`.
fn run_deploy_erc20_token_callback(
    io: StoragePointer<'_>,
    env: &Fixed,
    token: &BridgedToken,
    promise_result: PromiseResult,
) -> Result<Address, ContractError> {
    let callback_args = schedule_deploy_erc20_token(io, env, token).callback.args;
    let mut handler = PromiseTracker::default();
    handler.promise_results.push(promise_result);
    io.0.borrow_mut().input = callback_args;
    deploy_erc20_token_callback(io, env, &mut handler)
}

fn read_erc20_metadata(io: StoragePointer<'_>, env: &Fixed, address: Address) -> Erc20Metadata {
    let engine: Engine<_, _> =
        Engine::new(Address::zero(), env.current_account_id(), io, env).unwrap();
    engine
        .get_erc20_metadata(&Erc20Identifier::Erc20 { address })
        .unwrap()
}

#[test]
fn test_erc20_metadata_json_distinguishes_token_standards() {
    let storage = RefCell::new(Storage::default());
    let mut io = StoragePointer(&storage);
    let env = Fixed {
        predecessor_account_id: "owner.near".parse().unwrap(),
        ..test_env()
    };
    state::set_state(
        &mut io,
        &EngineState {
            owner_id: env.predecessor_account_id(),
            ..EngineState::default()
        },
    )
    .unwrap();
    let account_id: AccountId = "multi.near".parse().unwrap();
    let tokens = [
        BridgedToken::Nep141(account_id.clone().into()),
        BridgedToken::Nep245(Nep245Token {
            account_id,
            token_id: "coin".into(),
        }),
    ];
    let mut metadata = [
        Erc20Metadata {
            name: "Fungible Token".into(),
            symbol: "FT".into(),
            decimals: 18,
        },
        Erc20Metadata {
            name: "Multi Token".into(),
            symbol: "MT".into(),
            decimals: 6,
        },
    ];
    let addresses = tokens
        .into_iter()
        .zip(&metadata)
        .map(|(token, metadata)| {
            engine::deploy_erc20_token(token, Some(metadata.clone()), io, &env, &mut Noop).unwrap()
        })
        .collect::<Vec<_>>();
    let identifiers = [
        serde_json::json!({ "account_id": "multi.near" }),
        serde_json::json!({ "account_id": "multi.near", "token_id": "coin" }),
    ];

    for (index, identifier) in identifiers.iter().enumerate() {
        storage.borrow_mut().input = serde_json::to_vec(identifier).unwrap();
        get_erc20_metadata(io, &env).unwrap();
        let actual: Erc20Metadata = serde_json::from_slice(&storage.borrow().output).unwrap();
        assert_eq!(actual, metadata[index]);

        metadata[index].name.push_str(" Updated");
        storage.borrow_mut().input = serde_json::to_vec(&serde_json::json!({
            "erc20_identifier": identifier,
            "metadata": metadata[index],
        }))
        .unwrap();
        let result = set_erc20_metadata(io, &env, &mut Noop).unwrap();
        assert!(result.status.is_ok());

        for (address, expected) in addresses.iter().zip(&metadata) {
            assert_eq!(read_erc20_metadata(io, &env, *address), *expected);
        }
    }
}

fn callback_setup(storage: &RefCell<Storage>) -> (StoragePointer<'_>, Fixed) {
    let mut io = StoragePointer(storage);
    // The callback is private, so the predecessor must be the engine itself.
    let env = Fixed {
        predecessor_account_id: "aurora.near".parse().unwrap(),
        ..test_env()
    };
    state::set_state(&mut io, &EngineState::default()).unwrap();
    (io, env)
}

#[test_case::test_case(&nep141_token(), "ft_metadata", b""; "nep141")]
#[test_case::test_case(
    &BridgedToken::Nep245(Nep245Token {
        account_id: "multi.near".parse().unwrap(),
        token_id: r#"nep141:a"],"token_ids":["b\c"#.into(),
    }),
    "mt_metadata_base_by_token_id",
    br#"{"token_ids":["nep141:a\"],\"token_ids\":[\"b\\c"]}"#;
    "nep245_escapes_token_id"
)]
fn test_deploy_erc20_token_schedules_metadata_call(
    token: &BridgedToken,
    method: &str,
    args: &[u8],
) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);

    let promise = schedule_deploy_erc20_token(io, &env, token);

    assert_eq!(promise.base.target_account_id, *token.account_id());
    assert_eq!(promise.base.method, method);
    assert_eq!(promise.base.args, args);
    assert_eq!(promise.callback.target_account_id, env.current_account_id());
    assert_eq!(promise.callback.method, "deploy_erc20_token_callback");
    // NEP-141 callback arguments are tagged too.
    assert_eq!(promise.callback.args, borsh::to_vec(token).unwrap());
}

#[test_case::test_case(&nep141_token(), NEP148_METADATA, None; "nep141_borsh")]
#[test_case::test_case(&nep245_token(), NEP245_METADATA, None; "nep245_borsh")]
// Callbacks scheduled before NEP-245 support carry an untagged NEP-141 `AccountId`.
#[test_case::test_case(
    &nep141_token(),
    NEP148_METADATA,
    Some(b"\x09\x00\x00\x00wrap.near");
    "legacy_nep141"
)]
fn test_deploy_erc20_token_callback_uses_promise_metadata(
    token: &BridgedToken,
    metadata: &str,
    legacy_args: Option<&[u8]>,
) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);
    let result = PromiseResult::Successful(metadata.as_bytes().to_vec());

    let address = if let Some(args) = legacy_args {
        storage.borrow_mut().input = args.to_vec();
        let mut handler = PromiseTracker::default();
        handler.promise_results.push(result);
        deploy_erc20_token_callback(io, &env, &mut handler).unwrap()
    } else {
        run_deploy_erc20_token_callback(io, &env, token, result).unwrap()
    };

    assert_eq!(engine::get_erc20_from_nep(&io, token).unwrap(), address);
    assert_eq!(
        read_erc20_metadata(io, &env, address),
        Erc20Metadata {
            name: "Wrapped NEAR".into(),
            symbol: "wNEAR".into(),
            decimals: 24,
        }
    );
    assert_eq!(
        storage.borrow().output,
        borsh::to_vec(address.as_bytes()).unwrap()
    );
}

#[test_case::test_case(true, false, 0, errors::ERR_PAUSED; "paused_before_private_and_input")]
#[test_case::test_case(false, false, 0, b"ERR_PRIVATE_CALL"; "private_before_count_and_input")]
#[test_case::test_case(false, true, 0, errors::ERR_PROMISE_COUNT; "missing_result")]
#[test_case::test_case(false, true, 2, errors::ERR_PROMISE_COUNT; "extra_result")]
fn test_deploy_erc20_token_callback_checks_context(
    paused: bool,
    private: bool,
    promise_count: usize,
    expected_error: &[u8],
) {
    let storage = RefCell::new(Storage::default());
    let (mut io, mut env) = callback_setup(&storage);
    let mut state = state::get_state(&io).unwrap();
    state.is_paused = paused;
    state::set_state(&mut io, &state).unwrap();
    if !private {
        env.predecessor_account_id = "other.near".parse().unwrap();
    }
    storage.borrow_mut().input = b"invalid input".to_vec();
    let before = storage.borrow().kv_store.clone();
    let mut handler = PromiseTracker::default();
    handler.promise_results = vec![PromiseResult::Failed; promise_count];

    let error = deploy_erc20_token_callback(io, &env, &mut handler).unwrap_err();

    assert_eq!(error.message.as_ref().as_ref(), expected_error);
    assert_eq!(storage.borrow().kv_store, before);
    assert!(storage.borrow().output.is_empty());
}

#[test_case::test_case(&[2, 0, 0, 0]; "unknown_standard")]
#[test_case::test_case(b"\x09\x00\x00\x00wrap.nea"; "truncated_legacy")]
#[test_case::test_case(b"\x00\x09\x00\x00\x00wrap.near\x00"; "trailing_data")]
fn test_deploy_erc20_token_callback_rejects_malformed_args(input: &[u8]) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);
    let mut handler = PromiseTracker::default();
    handler
        .promise_results
        .push(PromiseResult::Successful(NEP148_METADATA.into()));
    io.0.borrow_mut().input = input.to_vec();
    let before = storage.borrow().kv_store.clone();

    let error = deploy_erc20_token_callback(io, &env, &mut handler).unwrap_err();

    assert_eq!(error.message.as_ref().as_ref(), b"ERR_ARG_PARSE");
    assert_eq!(storage.borrow().kv_store, before);
}

#[test_case::test_case(PromiseResult::Failed, errors::ERR_GETTING_ERC20_FROM_NEP141; "failed")]
#[test_case::test_case(
    PromiseResult::Successful(br#"{"name":"Wrapped NEAR","symbol":"wNEAR"}"#.to_vec()),
    b"missing field `decimals`";
    "missing_decimals"
)]
fn test_deploy_erc20_token_callback_requires_nep141_metadata(result: PromiseResult, error: &[u8]) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);

    let actual = run_deploy_erc20_token_callback(io, &env, &nep141_token(), result).unwrap_err();

    assert!(actual.message.as_ref().as_ref().starts_with(error));
}

#[test_case::test_case(PromiseResult::Failed; "failed")]
#[test_case::test_case(PromiseResult::Successful(b"[]".to_vec()); "empty_array")]
#[test_case::test_case(PromiseResult::Successful(b"[null]".to_vec()); "null_entry")]
fn test_deploy_erc20_token_callback_defaults_nep245_metadata(result: PromiseResult) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);

    let address = run_deploy_erc20_token_callback(io, &env, &nep245_token(), result).unwrap();

    assert_eq!(
        read_erc20_metadata(io, &env, address),
        Erc20Metadata::default()
    );
}

// Field-level validation is covered by the `Nep245Metadata` conversion tests in `engine-types`.
#[test_case::test_case(r#"[{"name":"First","symbol":"ONE","decimals":24},{"name":"Second","symbol":"TWO","decimals":6}]"#, b"ERR_ARGS"; "two_objects")]
#[test_case::test_case("[null,null]", b"ERR_ARGS"; "two_nulls")]
#[test_case::test_case(r#"[{"name":"Wrapped NEAR","decimals":"256"}]"#, b"ERR_ARGS"; "invalid_conversion")]
#[test_case::test_case(r#"[{"symbol":"wNEAR","decimals":24}]"#, b"missing field `name`"; "invalid_entry")]
#[test_case::test_case(r#"{"name":"Wrapped NEAR","symbol":"wNEAR","decimals":24}"#, b"invalid type: map"; "object")]
#[test_case::test_case("null", b"invalid type: null"; "null_response")]
fn test_deploy_erc20_token_callback_rejects_invalid_nep245_metadata(metadata: &str, error: &[u8]) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);
    let before = storage.borrow().kv_store.clone();
    let result = PromiseResult::Successful(metadata.as_bytes().to_vec());

    let actual = run_deploy_erc20_token_callback(io, &env, &nep245_token(), result).unwrap_err();

    assert!(actual.message.as_ref().as_ref().starts_with(error));
    assert_eq!(storage.borrow().kv_store, before);
}

#[test_case::test_case(&nep141_token(), errors::ERR_GETTING_ERC20_FROM_NEP141; "nep141")]
#[test_case::test_case(&nep245_token(), errors::ERR_GETTING_ERC20_FROM_NEP245; "nep245")]
fn test_mirror_erc20_token_callback_reports_failed_promise(
    token: &BridgedToken,
    expected_error: &[u8],
) {
    let storage = RefCell::new(Storage::default());
    let (io, env) = callback_setup(&storage);
    io.0.borrow_mut().input = borsh::to_vec(&MirrorErc20TokenArgs {
        contract_id: "main.near".parse().unwrap(),
        token: token.clone(),
    })
    .unwrap();
    let before = storage.borrow().kv_store.clone();
    let address = PromiseResult::Successful(make_address(4, 4).as_bytes().to_vec());

    // Either the address or the metadata request may fail.
    for promise_results in [
        vec![PromiseResult::Failed, PromiseResult::Failed],
        vec![address, PromiseResult::Failed],
    ] {
        let mut handler = PromiseTracker::default();
        handler.promise_results = promise_results;

        let error = mirror_erc20_token_callback(io, &env, &mut handler).unwrap_err();

        assert_eq!(error.message.as_ref().as_ref(), expected_error);
        assert_eq!(storage.borrow().kv_store, before);
    }
}
