use super::*;
use aurora_engine_types::parameters::{PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs};
use aurora_engine_types::types::{NearGas, Yocto};
use near_sdk::test_utils::test_env::{alice, bob, carol};
use near_sdk::test_utils::{self, VMContextBuilder};
use near_sdk::{serde_json, testing_env, MockedBlockchain};

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

    let contract = Router::initialize();
    assert_eq!(contract.nonce.get().unwrap(), nonce);
}

// If an account other than the parent calls `initialize` it panics.
#[test]
#[should_panic]
fn test_reinitialize_wrong_caller() {
    let (parent, contract) = create_contract();

    assert_eq!(contract.parent.get().unwrap(), parent);
    drop(contract);

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(bob().try_into().unwrap())
        .build());
    let _contract = Router::initialize();
}

#[test]
#[should_panic]
fn test_execute_wrong_caller() {
    let (_parent, contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(bob().try_into().unwrap())
        .build());
    contract.execute(PromiseArgs::Create(promise));
}

#[test]
fn test_execute() {
    let (_parent, contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    contract.execute(PromiseArgs::Create(promise.clone()));

    let mut receipts = Receipt::get_created_receipts();
    assert_eq!(receipts.len(), 1);
    let receipt = receipts.pop().unwrap();
    assert_eq!(receipt.receiver_id(), promise.target_account_id.as_ref());

    validate_function_call_action(&receipt.actions(), promise);
}

#[test]
fn test_execute_callback() {
    let (_parent, contract) = create_contract();

    let promise = PromiseWithCallbackArgs {
        base: PromiseCreateArgs {
            target_account_id: bob().parse().unwrap(),
            method: "some_method".into(),
            args: b"hello_world".to_vec(),
            attached_balance: Yocto::new(5678),
            attached_gas: NearGas::new(100_000_000_000_000),
        },
        callback: PromiseCreateArgs {
            target_account_id: carol().parse().unwrap(),
            method: "another_method".into(),
            args: b"goodbye_world".to_vec(),
            attached_balance: Yocto::new(567),
            attached_gas: NearGas::new(10_000_000_000_000),
        },
    };

    contract.execute(PromiseArgs::Callback(promise.clone()));

    let receipts = Receipt::get_created_receipts();
    assert_eq!(receipts.len(), 2);
    let base = &receipts[0];
    let callback = &receipts[1];

    validate_function_call_action(&base.actions(), promise.base);
    validate_function_call_action(&callback.actions(), promise.callback);
}

#[test]
#[should_panic]
fn test_schedule_wrong_caller() {
    let (_parent, mut contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(bob().try_into().unwrap())
        .build());
    contract.schedule(PromiseArgs::Create(promise));
}

#[test]
fn test_schedule_and_execute() {
    let (_parent, mut contract) = create_contract();

    let promise = PromiseCreateArgs {
        target_account_id: bob().parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    contract.schedule(PromiseArgs::Create(promise.clone()));

    // no promise actually create yet
    let receipts = Receipt::get_created_receipts();
    assert!(receipts.is_empty());

    // promise stored and nonce incremented instead
    assert_eq!(contract.nonce.get().unwrap(), 1);
    let stored_promise = match contract.scheduled_promises.get(&0) {
        Some(PromiseArgs::Create(promise)) => promise,
        _ => unreachable!(),
    };
    assert_eq!(stored_promise, promise);

    // promise executed after calling `execute_scheduled`
    // anyone can call this function
    testing_env!(VMContextBuilder::new()
        .predecessor_account_id(bob().try_into().unwrap())
        .build());
    contract.execute_scheduled(0.into());

    assert_eq!(contract.nonce.get().unwrap(), 1);
    assert!(!contract.scheduled_promises.contains_key(&0));

    let mut receipts = Receipt::get_created_receipts();
    assert_eq!(receipts.len(), 1);
    let receipt = receipts.pop().unwrap();
    assert_eq!(receipt.receiver_id(), promise.target_account_id.as_ref());
    validate_function_call_action(&receipt.actions(), promise);
}

fn validate_function_call_action(actions: &[Action], promise: PromiseCreateArgs) {
    assert_eq!(actions.len(), 1);
    let action = &actions[0];
    assert_eq!(
        action.function_call().method_name(),
        promise.method.as_str()
    );
    assert_eq!(action.function_call().args(), promise.args.as_slice());
    assert_eq!(action.function_call().gas(), promise.attached_gas);
    assert_eq!(action.function_call().deposit(), promise.attached_balance);
}

fn create_contract() -> (String, Router) {
    let parent = alice();
    testing_env!(VMContextBuilder::new()
        .current_account_id(format!("some_address.{}", parent).try_into().unwrap())
        .predecessor_account_id(parent.as_str().try_into().unwrap())
        .build());
    let contract = Router::initialize();

    (parent, contract)
}

/// Cannot use the `Receipt` type from `test_utils::get_created_receipts` for introspection
/// because all the fields are private. As a work-around we serialize the object to json.
#[derive(Debug)]
struct Receipt {
    underlying: serde_json::Value,
}

impl Receipt {
    fn get_created_receipts() -> Vec<Self> {
        let receipts = test_utils::get_created_receipts();
        receipts
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .map(|v| Self {
                underlying: serde_json::from_str(&v).unwrap(),
            })
            .collect()
    }

    fn receiver_id(&self) -> &str {
        self.underlying
            .as_object()
            .unwrap()
            .get("receiver_id")
            .unwrap()
            .as_str()
            .unwrap()
    }

    fn actions(&self) -> Vec<Action> {
        self.underlying
            .as_object()
            .unwrap()
            .get("actions")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Action { underlying: v })
            .collect()
    }
}

struct Action<'a> {
    underlying: &'a serde_json::Value,
}

impl<'a> Action<'a> {
    fn function_call(&self) -> FunctionCall {
        FunctionCall {
            underlying: self
                .underlying
                .as_object()
                .unwrap()
                .get("FunctionCall")
                .unwrap(),
        }
    }
}

struct FunctionCall<'a> {
    underlying: &'a serde_json::Value,
}

impl<'a> FunctionCall<'a> {
    fn method_name(&self) -> &str {
        self.underlying
            .as_object()
            .unwrap()
            .get("method_name")
            .unwrap()
            .as_str()
            .unwrap()
    }

    fn args(&self) -> &[u8] {
        self.underlying
            .as_object()
            .unwrap()
            .get("args")
            .unwrap()
            .as_str()
            .unwrap()
            .as_bytes()
    }

    fn gas(&self) -> NearGas {
        NearGas::new(
            self.underlying
                .as_object()
                .unwrap()
                .get("gas")
                .unwrap()
                .as_u64()
                .unwrap(),
        )
    }

    fn deposit(&self) -> Yocto {
        Yocto::new(
            self.underlying
                .as_object()
                .unwrap()
                .get("deposit")
                .unwrap()
                .as_u64()
                .unwrap() as u128,
        )
    }
}
