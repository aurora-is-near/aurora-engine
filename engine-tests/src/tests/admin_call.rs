//! Tests of the `admin_call` contract method on the compiled WASM run by `AuroraRunner`, which
//! shows the receipt the engine asks the NEAR host to create, and in the NEAR sandbox (the
//! `workspace` module), which executes that receipt.

use crate::utils;
use near_primitives_core::types::{Balance, Gas, GasWeight};
use near_vm_runner::logic::mocks::mock_external::MockAction;
use near_vm_runner::logic::types::ReturnData;

const RECEIVER: &str = "token.near";
const TRANSFER: &str = r#"{"Transfer":{"amount":"1"}}"#;

/// `admin_call` input that sends the given actions, written as JSON, to `receiver`.
fn input(receiver: &str, actions: &str) -> Vec<u8> {
    format!(r#"{{"receiver_id":"{receiver}","actions":[{actions}]}}"#).into_bytes()
}

#[test]
fn test_admin_call_creates_batch_receipt() {
    let mut runner = utils::deploy_runner();
    // The standalone engine has no transaction kind for `admin_call`.
    runner.standalone_runner = None;
    runner.context.attached_deposit = Balance::from_yoctonear(1);
    // `deploy_runner` makes the engine account its own owner.
    let owner = runner.aurora_account_id.clone();
    let call = r#"{"FunctionCall":{"method_name":"f","args":"e30=","deposit":"2","gas":3}}"#;
    let input = input(RECEIVER, &format!("{call},{TRANSFER}"));

    let outcome = runner.call("admin_call", &owner, input).unwrap();

    // One receipt to the receiver carrying both actions in the given order.
    let expected = [
        MockAction::CreateReceipt {
            receipt_indices: vec![],
            receiver_id: RECEIVER.parse().unwrap(),
        },
        MockAction::FunctionCallWeight {
            receipt_index: 0,
            method_name: b"f".to_vec(),
            args: b"{}".to_vec(),
            attached_deposit: Balance::from_yoctonear(2),
            prepaid_gas: Gas::from_gas(3),
            gas_weight: GasWeight(0),
        },
        MockAction::Transfer {
            receipt_index: 0,
            deposit: Balance::from_yoctonear(1),
        },
    ];
    assert_eq!(runner.ext.underlying.action_log, expected);
    // The receipt is the return value of the call, so the caller gets the result of the batch.
    assert!(matches!(outcome.return_data, ReturnData::ReceiptIndex(0)));
}

#[test]
fn test_admin_call_rejections() {
    let mut runner = utils::deploy_runner();
    let engine = runner.aurora_account_id.clone();
    let owner = engine.as_str();
    let valid = input(RECEIVER, TRANSFER);
    // Caller, attached deposit, input, and the error the contract panics with.
    let cases = [
        ("stranger.near", 1, valid.clone(), "NotAllowed"),
        (owner, 0, valid, "ERR_DEPOSIT_REQUIRED"),
        (owner, 1, input(owner, TRANSFER), "ERR_INVALID_RECEIVER"),
        (owner, 1, input(RECEIVER, ""), "ERR_INVALID_ACTIONS_NUMBER"),
        (owner, 1, input(RECEIVER, "{}"), "ERR_JSON_DESERIALIZE"),
    ];

    for (caller, deposit, input, expected) in cases {
        runner.context.attached_deposit = Balance::from_yoctonear(deposit);
        let err = runner.call("admin_call", caller, input).unwrap_err();

        assert!(format!("{:?}", err.kind).contains(expected), "{err:?}");
        assert!(runner.ext.underlying.action_log.is_empty());
    }
}

mod workspace {
    use crate::utils::workspace::{
        create_sub_account, deploy_engine, deploy_nep_141, nep_141_balance_of, transfer_nep_141,
    };
    use aurora_engine_types::account_id::AccountId;
    use aurora_engine_types::parameters::admin::{AdminCallAction, AdminCallArgs};
    use aurora_engine_types::types::{NearGas, Yocto};
    use aurora_engine_workspace::EngineContract;
    use aurora_engine_workspace::account::Account;
    use aurora_engine_workspace::types::{ExecutionFinalResult, KeyType, NearToken, SecretKey};
    use near_sdk::json_types::U128;
    use serde_json::json;

    const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);
    const CALL_GAS: u64 = 30_000_000_000_000;
    const TRANSFER_AMOUNT: u128 = NearToken::from_near(5).as_yoctonear();

    fn transfer_to(receiver_id: AccountId) -> AdminCallArgs {
        AdminCallArgs {
            receiver_id,
            actions: vec![AdminCallAction::Transfer {
                amount: Yocto::new(TRANSFER_AMOUNT),
            }],
        }
    }

    /// Sends `admin_call` to the engine from `sender` with one yoctoNEAR attached.
    async fn admin_call(
        sender: &Account,
        aurora: &EngineContract,
        args: &(impl serde::Serialize + Sync),
    ) -> ExecutionFinalResult {
        sender
            .call(&aurora.id(), "admin_call")
            .args_json(args)
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await
            .unwrap()
    }

    async fn sub_account(aurora: &EngineContract, name: &str) -> Account {
        create_sub_account(&aurora.root(), name, NearToken::from_near(60))
            .await
            .unwrap()
    }

    async fn balance_of(aurora: &EngineContract, account: &Account) -> u128 {
        aurora.node.get_balance(&account.id()).await.unwrap()
    }

    fn assert_failed_with(result: ExecutionFinalResult, expected: &str) {
        let err = format!("{:?}", result.into_result().unwrap_err());
        assert!(err.contains(expected), "{err}");
    }

    #[tokio::test]
    async fn test_admin_call_transfers_nep141_tokens() {
        let aurora = deploy_engine().await;
        let token_owner = sub_account(&aurora, "token_owner").await;
        let user = sub_account(&aurora, "user").await;
        let token = deploy_nep_141(
            &sub_account(&aurora, "token").await,
            &token_owner,
            1_000,
            &aurora,
        )
        .await
        .unwrap();
        // Tokens that ended up on the engine account.
        transfer_nep_141(&token.id(), &token_owner, aurora.id().as_ref(), 1_000)
            .await
            .unwrap();

        // The input as an operator writes it by hand: `args` is the base64 of the JSON
        // arguments of each call. One batch registers the user in the token contract, sends
        // the tokens, and reads the resulting balance.
        assert_eq!(user.id().as_ref(), "user.root");
        let input = json!({
            "receiver_id": token.id(),
            "actions": [
                {"FunctionCall": {
                    "method_name": "storage_deposit",
                    // {"account_id":"user.root"}
                    "args": "eyJhY2NvdW50X2lkIjoidXNlci5yb290In0=",
                    "deposit": "10000000000000000000000",
                    "gas": CALL_GAS,
                }},
                {"FunctionCall": {
                    "method_name": "ft_transfer",
                    // {"receiver_id":"user.root","amount":"400"}
                    "args": "eyJyZWNlaXZlcl9pZCI6InVzZXIucm9vdCIsImFtb3VudCI6IjQwMCJ9",
                    "deposit": "1",
                    "gas": CALL_GAS,
                }},
                {"FunctionCall": {
                    "method_name": "ft_balance_of",
                    // {"account_id":"user.root"}
                    "args": "eyJhY2NvdW50X2lkIjoidXNlci5yb290In0=",
                    "deposit": "0",
                    "gas": CALL_GAS,
                }},
            ],
        });
        let result = admin_call(&aurora.account, &aurora, &input).await;
        assert!(result.is_success(), "{result:?}");

        // The call returns what the last action of the batch returned.
        assert_eq!(result.json::<U128>().unwrap().0, 400);
        assert_eq!(nep_141_balance_of(&token, &aurora.id()).await, 600);
    }

    #[tokio::test]
    async fn test_admin_call_batch_is_atomic() {
        let aurora = deploy_engine().await;
        let recipient = sub_account(&aurora, "recipient").await;
        let balance = balance_of(&aurora, &recipient).await;

        // The recipient has no contract, so the function call fails and takes the transfer
        // that precedes it in the batch down with it.
        let mut args = transfer_to(recipient.id());
        args.actions.push(AdminCallAction::FunctionCall {
            method_name: "no_such_method".to_string(),
            args: vec![],
            deposit: Yocto::new(0),
            gas: NearGas::new(CALL_GAS),
        });
        let result = aurora
            .admin_call(args)
            .deposit(ONE_YOCTO)
            .max_gas()
            .transact()
            .await;

        assert!(result.is_err(), "{result:?}");
        assert_eq!(balance_of(&aurora, &recipient).await, balance);
    }

    #[tokio::test]
    async fn test_admin_call_requires_owner() {
        let aurora = deploy_engine().await;
        let recipient = sub_account(&aurora, "recipient").await;
        let new_owner = sub_account(&aurora, "new_owner").await;
        let args = transfer_to(recipient.id());
        let balance = balance_of(&aurora, &recipient).await;

        let result = admin_call(&new_owner, &aurora, &args).await;
        assert_failed_with(result, "ERR_NOT_ALLOWED");

        let result = aurora
            .set_owner(&new_owner.id())
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // The engine account still signs with its full access key, but it is not the owner
        // any more.
        let result = admin_call(&aurora.account, &aurora, &args).await;
        assert_failed_with(result, "ERR_NOT_ALLOWED");
        assert_eq!(balance_of(&aurora, &recipient).await, balance);

        // The new owner is a different account from the engine; the transfer is still paid
        // from the engine's balance.
        let result = admin_call(&new_owner, &aurora, &args).await;
        assert!(result.is_success(), "{result:?}");
        assert_eq!(
            balance_of(&aurora, &recipient).await - balance,
            TRANSFER_AMOUNT
        );
    }

    #[tokio::test]
    async fn test_admin_call_cannot_be_made_with_function_call_key() {
        // The deposit requirement keeps function call access keys out: NEAR does not let such
        // a key attach a deposit, and the contract rejects a call without one.
        let aurora = deploy_engine().await;
        let recipient = sub_account(&aurora, "recipient").await;
        let args = transfer_to(recipient.id());
        let balance = balance_of(&aurora, &recipient).await;

        let secret_key = SecretKey::from_random(KeyType::ED25519);
        let result = aurora
            .account
            .add_function_call_key(secret_key.public_key(), &aurora.id(), &["admin_call"])
            .await
            .unwrap();
        assert!(result.is_success());
        // The owner's account, signing with the function call key.
        let limited_owner = aurora.create_account(&aurora.id(), secret_key);
        let call = || {
            limited_owner
                .call(&aurora.id(), "admin_call")
                .args_json(&args)
                .max_gas()
        };

        // With a deposit, the network does not accept the transaction at all.
        let result = call().deposit(ONE_YOCTO).transact().await;
        assert!(result.is_err(), "{result:?}");

        // Without one, the transaction reaches the contract, which rejects it.
        let result = call().transact().await.unwrap();
        assert_failed_with(result, "ERR_DEPOSIT_REQUIRED");
        assert_eq!(balance_of(&aurora, &recipient).await, balance);
    }
}
