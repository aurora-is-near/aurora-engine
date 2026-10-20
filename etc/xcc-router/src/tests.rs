use super::Router;
use aurora_engine_types::parameters::{PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs};
use aurora_engine_types::types::{NearGas, Yocto};
use near_primitives::types::GasWeight;
use near_sdk::mock::MockAction;
use near_sdk::test_utils::test_env::{alice, bob, carol};
use near_sdk::test_utils::{self, VMContextBuilder};
use near_sdk::{Gas, NearToken, testing_env};

const WNEAR_ACCOUNT: &str = "wrap.near";

#[test]
fn test_initialize() {
    let (parent, contract) = create_contract();

    assert_eq!(contract.parent.get().unwrap(), parent);
}

/// `initialize` should be able to be called multiple times without resetting the state.
#[test]
fn test_reinitialize() {
    let (_parent, mut contract) = create_contract();

    let nonce = 8;
    contract.nonce.set(&nonce);
    drop(contract);

    let contract = Router::initialize(WNEAR_ACCOUNT.parse().unwrap(), false);
    assert_eq!(contract.nonce.get().unwrap(), nonce);
}

// If an account other than the parent calls `initialize` it panics.
#[test]
#[should_panic(expected = "ERR_ILLEGAL_CALLER")]
fn test_reinitialize_wrong_caller() {
    let (parent, contract) = create_contract();

    assert_eq!(contract.parent.get().unwrap(), parent);
    drop(contract);

    testing_env!(
        VMContextBuilder::new()
            .predecessor_account_id(bob())
            .build()
    );
    let _contract = Router::initialize(WNEAR_ACCOUNT.parse().unwrap(), false);
}

#[test]
#[should_panic(expected = "ERR_ILLEGAL_CALLER")]
fn test_execute_wrong_caller() {
    let (_parent, contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().as_str().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    testing_env!(
        VMContextBuilder::new()
            .predecessor_account_id(bob())
            .build()
    );
    contract.execute(PromiseArgs::Create(promise));
}

#[test]
fn test_execute() {
    let (_parent, contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().as_str().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    contract.execute(PromiseArgs::Create(promise.clone()));

    let mut receipts = test_utils::get_created_receipts();
    assert_eq!(receipts.len(), 1);
    let receipt = receipts.pop().unwrap();
    assert_eq!(
        receipt.receiver_id.as_str(),
        promise.target_account_id.as_ref()
    );

    validate_function_call_action(&receipt.actions, promise, 0);
}

#[test]
fn test_execute_callback() {
    let (_parent, contract) = create_contract();

    let promise = PromiseWithCallbackArgs {
        base: PromiseCreateArgs {
            target_account_id: bob().as_str().parse().unwrap(),
            method: "some_method".into(),
            args: b"hello_world".to_vec(),
            attached_balance: Yocto::new(5678),
            attached_gas: NearGas::new(100_000_000_000_000),
        },
        callback: PromiseCreateArgs {
            target_account_id: carol().as_str().parse().unwrap(),
            method: "another_method".into(),
            args: b"goodbye_world".to_vec(),
            attached_balance: Yocto::new(567),
            attached_gas: NearGas::new(10_000_000_000_000),
        },
    };

    contract.execute(PromiseArgs::Callback(promise.clone()));

    let receipts = test_utils::get_created_receipts();
    assert_eq!(receipts.len(), 2);
    let base = &receipts[0];
    let callback = &receipts[1];

    validate_function_call_action(&base.actions, promise.base, 0);
    validate_function_call_action(&callback.actions, promise.callback, 2);
}

#[test]
#[should_panic(expected = "ERR_ILLEGAL_CALLER")]
fn test_schedule_wrong_caller() {
    let (_parent, mut contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().as_str().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    testing_env!(
        VMContextBuilder::new()
            .predecessor_account_id(bob())
            .build()
    );
    contract.schedule(PromiseArgs::Create(promise));
}

#[test]
fn test_schedule_and_execute() {
    let (_parent, mut contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().as_str().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    contract.schedule(PromiseArgs::Create(promise.clone()));

    // no promise actually create yet
    let receipts = test_utils::get_created_receipts();
    assert!(receipts.is_empty());

    // promise stored and nonce incremented instead
    assert_eq!(contract.nonce.get().unwrap(), 1);
    let Some(PromiseArgs::Create(stored_promise)) = contract.scheduled_promises.get(&0) else {
        unreachable!()
    };
    assert_eq!(stored_promise, &promise);

    // promise executed after calling `execute_scheduled`
    // anyone can call this function
    testing_env!(
        VMContextBuilder::new()
            .predecessor_account_id(bob())
            .build()
    );
    contract.execute_scheduled(0.into());

    assert_eq!(contract.nonce.get().unwrap(), 1);
    assert!(!contract.scheduled_promises.contains_key(&0));

    let mut receipts = test_utils::get_created_receipts();
    assert_eq!(receipts.len(), 1);
    let receipt = receipts.pop().unwrap();
    assert_eq!(
        receipt.receiver_id.as_str(),
        promise.target_account_id.as_ref()
    );
    validate_function_call_action(&receipt.actions, promise, 0);
}

fn validate_function_call_action(
    actions: &[MockAction],
    promise: PromiseCreateArgs,
    receipt_index: u64,
) {
    assert_eq!(actions.len(), 1);
    let action = &actions[0];

    assert_eq!(
        *action,
        MockAction::FunctionCallWeight {
            receipt_index,
            method_name: promise.method.into_bytes(),
            args: promise.args,
            attached_deposit: NearToken::from_yoctonear(promise.attached_balance.as_u128()),
            prepaid_gas: Gas::from_gas(promise.attached_gas.as_u64()),
            gas_weight: GasWeight(0),
        }
    );
}

fn create_contract() -> (near_sdk::AccountId, Router) {
    let parent = alice();
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(format!("some_address.{parent}").try_into().unwrap())
            .predecessor_account_id(parent.clone())
            .build()
    );
    let contract = Router::initialize(WNEAR_ACCOUNT.parse().unwrap(), false);

    (parent, contract)
}

#[cfg(feature = "all-promise-actions")]
#[test]
fn test_public_key_encoding() {
    use aurora_engine_types::public_key::PublicKey;

    let mut expected_ed25519 = vec![0];
    expected_ed25519.extend_from_slice(&[7; 32]);
    assert_eq!(
        super::to_sdk_pk(&PublicKey::Ed25519([7; 32])).as_bytes(),
        expected_ed25519
    );

    let mut expected_secp256k1 = vec![1];
    expected_secp256k1.extend_from_slice(&[9; 64]);
    assert_eq!(
        super::to_sdk_pk(&PublicKey::Secp256k1([9; 64])).as_bytes(),
        expected_secp256k1
    );
}
