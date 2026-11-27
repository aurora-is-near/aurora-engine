use aurora_engine_sdk::types::near_account_to_evm_address;
use aurora_engine_types::borsh;
use aurora_engine_types::parameters::connector::{
    ExitToNearPrecompileCallbackArgs, Nep245Token, RefundCallArgs, TransferCall,
};
use aurora_engine_types::parameters::engine::SubmitResult;
use aurora_engine_types::types::{PromiseResult, u256_to_arr};
use near_vm_runner::logic::mocks::mock_external::MockAction;
use serde_json::json;

use crate::prelude::{Address, U256};
use crate::tests::nep245::utils::{self, TOKEN_ID};
use crate::utils::AuroraRunner;

const MT_TOKEN: &str = "mt.near";
const HOLDER: &str = "holder.near";
const RECEIVER: &str = "receiver.near";
const DEPOSIT_AMOUNT: u128 = 300_000;
const EXIT_AMOUNT: u128 = 50_000;
const UNUSED_AMOUNT: u128 = 20_000;
/// Whether exit callbacks carry refund arguments.
const REFUND: bool = cfg!(feature = "error_refund");

/// A function call scheduled by the engine.
struct FunctionCall {
    method: String,
    args: Vec<u8>,
}

/// Withdraws `EXIT_AMOUNT` of the holder's ERC-20 tokens to `recipient` and returns the
/// function calls the engine scheduled.
fn exit(runner: &mut AuroraRunner, erc20: Address, recipient: &str) -> Vec<FunctionCall> {
    runner.ext.underlying.action_log.clear();
    let input = utils::withdraw_to_near_input(recipient, EXIT_AMOUNT);
    let outcome = runner.evm_call(erc20, input, HOLDER).unwrap();
    let result: SubmitResult = borsh::from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok(), "{result:?}");

    runner
        .ext
        .underlying
        .action_log
        .iter()
        .filter_map(|action| match action {
            MockAction::FunctionCallWeight {
                method_name, args, ..
            } => Some(FunctionCall {
                method: String::from_utf8(method_name.clone()).unwrap(),
                args: args.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// Checks the token transfer, and the exit callback when refunds are enabled.
fn check_calls(
    calls: &[FunctionCall],
    method: &str,
    args: &serde_json::Value,
    callback: &ExitToNearPrecompileCallbackArgs,
) -> Option<Vec<u8>> {
    let (transfer, callback_call) = match calls {
        [transfer] => (transfer, None),
        [transfer, callback] => (transfer, Some(callback)),
        _ => panic!("expected a transfer and an optional callback"),
    };
    assert_eq!(transfer.method, method);
    assert_eq!(
        &serde_json::from_slice::<serde_json::Value>(&transfer.args).unwrap(),
        args
    );
    // Without refund arguments, plain transfers and `transfer_call` need no callback.
    assert_eq!(callback_call.is_some(), REFUND);

    callback_call.map(|call| {
        assert_eq!(call.method, "exit_to_near_precompile_callback");
        assert_eq!(
            &ExitToNearPrecompileCallbackArgs::deserialize(&call.args).unwrap(),
            callback
        );
        call.args.clone()
    })
}

/// Runs the exit callback as the NEAR runtime would after the transfer finished with `result`.
fn settle(runner: &mut AuroraRunner, callback_args: Vec<u8>, result: PromiseResult) {
    let owner = runner.aurora_account_id.clone();
    runner.promise_results = vec![result];
    runner
        .call("exit_to_near_precompile_callback", &owner, callback_args)
        .unwrap();
    runner.promise_results.clear();
}

#[test]
fn test_mt_exit_wasm() {
    let mut runner = AuroraRunner::new();
    let owner = runner.aurora_account_id.clone();
    let token = Nep245Token {
        account_id: MT_TOKEN.parse().unwrap(),
        token_id: TOKEN_ID.into(),
    };
    let erc20 = utils::deploy_erc20_wasm(&mut runner, token);
    // The holder calls the engine from NEAR, so its EVM address derives from the account ID.
    let holder = near_account_to_evm_address(HOLDER);
    let args = utils::mt_on_transfer_args(
        HOLDER,
        &[TOKEN_ID],
        DEPOSIT_AMOUNT,
        holder.encode().as_str(),
    );
    runner.call("mt_on_transfer", MT_TOKEN, args).unwrap();
    let refund = RefundCallArgs {
        recipient_address: holder,
        erc20_address: Some(erc20),
        amount: u256_to_arr(&U256::from(EXIT_AMOUNT)),
    };
    let mut balance = DEPOSIT_AMOUNT;

    // A plain exit transfers the tokens with `mt_transfer`.
    let calls = exit(&mut runner, erc20, RECEIVER);
    balance -= EXIT_AMOUNT;
    let callback = check_calls(
        &calls,
        "mt_transfer",
        &json!({
            "receiver_id": RECEIVER,
            "token_id": TOKEN_ID,
            "amount": EXIT_AMOUNT.to_string(),
        }),
        &ExitToNearPrecompileCallbackArgs {
            refund: Some(refund.clone()),
            transfer_near: None,
            transfer_call: TransferCall::None,
        },
    );
    if let Some(callback) = callback {
        // A failed transfer refunds the whole amount.
        settle(&mut runner, callback, PromiseResult::Failed);
        balance += EXIT_AMOUNT;
    }
    assert_eq!(runner.balance_of(erc20, holder, &owner), balance.into());

    // A message after the recipient turns the exit into `mt_transfer_call`.
    let msg = format!("return:{UNUSED_AMOUNT}");
    let calls = exit(&mut runner, erc20, &format!("{RECEIVER}:{msg}"));
    balance -= EXIT_AMOUNT;
    let callback = check_calls(
        &calls,
        "mt_transfer_call",
        &json!({
            "receiver_id": RECEIVER,
            "token_id": TOKEN_ID,
            "amount": EXIT_AMOUNT.to_string(),
            "msg": msg,
        }),
        &ExitToNearPrecompileCallbackArgs {
            refund: Some(refund),
            transfer_near: None,
            transfer_call: TransferCall::Nep245,
        },
    );
    if let Some(callback) = callback {
        // `mt_transfer_call` returns the used amounts, so only the unused part is refunded.
        let used = format!(r#"["{}"]"#, EXIT_AMOUNT - UNUSED_AMOUNT);
        settle(
            &mut runner,
            callback,
            PromiseResult::Successful(used.into_bytes()),
        );
        balance += UNUSED_AMOUNT;
    }
    assert_eq!(runner.balance_of(erc20, holder, &owner), balance.into());
}

mod workspace {
    use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
    use aurora_engine_workspace::types::ExecutionFinalResult;

    use super::{DEPOSIT_AMOUNT, EXIT_AMOUNT, REFUND, UNUSED_AMOUNT};
    use crate::tests::erc20_connector::workspace::exit_to_near;
    use crate::tests::nep245::utils::{self, Nep245Context, TOKEN_ID, TOTAL_SUPPLY};
    use crate::utils::workspace::deploy_ft_receiver;

    /// Panic message of the mock NEP-245 contract for a rejected receiver.
    const REJECTED: &str = "Receiver rejects the tokens";

    /// Bridges `DEPOSIT_AMOUNT` of the user's tokens to the user's EVM address.
    async fn setup() -> Nep245Context {
        let ctx = utils::setup().await;
        ctx.deposit(DEPOSIT_AMOUNT, ctx.user_address())
            .await
            .unwrap();
        ctx
    }

    /// Withdraws `EXIT_AMOUNT` of the user's ERC-20 tokens to `recipient` on NEAR.
    async fn exit(ctx: &Nep245Context, recipient: &str) -> ExecutionFinalResult {
        exit_to_near(&ctx.user, recipient, EXIT_AMOUNT, &ctx.erc20, &ctx.aurora).await
    }

    #[tokio::test]
    async fn test_exit_with_mt_transfer() {
        let ctx = setup().await;
        let user_id = ctx.user.id();

        // `:unwrap` applies only to wNEAR, so a NEP-245 exit ignores it.
        for (exits, suffix) in (1..).zip(["", ":unwrap"]) {
            exit(&ctx, &format!("{user_id}{suffix}")).await.unwrap();

            let withdrawn = exits * EXIT_AMOUNT;
            assert_eq!(
                ctx.mt_balance_of(&user_id).await,
                TOTAL_SUPPLY - DEPOSIT_AMOUNT + withdrawn
            );
            assert_eq!(
                ctx.mt_balance_of(&ctx.aurora.id()).await,
                DEPOSIT_AMOUNT - withdrawn
            );
            assert_eq!(
                ctx.erc20_balance_of(ctx.user_address()).await,
                (DEPOSIT_AMOUNT - withdrawn).into()
            );
            ctx.assert_erc20_backed().await;
        }
    }

    #[tokio::test]
    async fn test_exit_with_mt_transfer_call_settles_unused_tokens() {
        let ctx = setup().await;
        let receiver = deploy_ft_receiver(&ctx.aurora).await.unwrap();
        let cases = [
            ("keep".to_string(), 0),
            (format!("return:{UNUSED_AMOUNT}"), UNUSED_AMOUNT),
            ("panic".to_string(), EXIT_AMOUNT),
            // The token contract caps the unused amount at the transferred one.
            (format!("return:{}", EXIT_AMOUNT + 1), EXIT_AMOUNT),
        ];
        let mut received = 0;
        let mut locked = DEPOSIT_AMOUNT;
        let mut erc20_balance = DEPOSIT_AMOUNT;
        let mut surplus = 0;
        let mt_token_id = ctx.mt_token.id();
        let receiver_id = receiver.id();

        for (msg, unused) in cases {
            let result = exit(&ctx, &format!("{receiver_id}:{msg}")).await;
            assert!(
                result
                    .receipt_outcomes()
                    .iter()
                    .any(|outcome| outcome.executor_id.as_str() == mt_token_id.as_ref()),
                "missing NEP-245 transfer receipt: {msg}"
            );
            let receiver_outcome = result
                .receipt_outcomes()
                .iter()
                .find(|outcome| outcome.executor_id.as_str() == receiver_id.as_ref())
                .unwrap_or_else(|| panic!("missing receiver receipt: {msg}"));
            assert_eq!(receiver_outcome.is_failure(), msg == "panic", "{msg}");
            let submitted: SubmitResult = result.borsh().unwrap();
            assert!(submitted.status.is_ok(), "{msg}: {submitted:?}");

            let used = EXIT_AMOUNT - unused;
            received += used;
            locked -= used;
            if REFUND {
                // The callback refunds the ERC-20 tokens the receiver did not use.
                erc20_balance -= used;
            } else {
                // Without the callback the returned tokens stay on the engine account.
                erc20_balance -= EXIT_AMOUNT;
                surplus += unused;
            }
            assert_eq!(
                utils::mt_balance_of(&ctx.mt_token, &receiver.id(), TOKEN_ID).await,
                received,
                "{msg}"
            );
            assert_eq!(ctx.mt_balance_of(&ctx.aurora.id()).await, locked, "{msg}");
            assert_eq!(
                ctx.erc20_balance_of(ctx.user_address()).await,
                erc20_balance.into(),
                "{msg}"
            );
            ctx.assert_locked_surplus(surplus).await;
        }
    }

    #[tokio::test]
    async fn test_mt_transfer_call_refunds_only_remaining_balance() {
        let ctx = setup().await;
        let receiver = deploy_ft_receiver(&ctx.aurora).await.unwrap();
        let spent = EXIT_AMOUNT - UNUSED_AMOUNT;
        let user_id = ctx.user.id();
        let recipient = format!("{}:spend:{user_id}:{spent}", receiver.id());

        let result = exit(&ctx, &recipient).await;

        assert!(result.receipt_failures().is_empty(), "{result:?}");
        let submitted: SubmitResult = result.borsh().unwrap();
        assert!(submitted.status.is_ok(), "{submitted:?}");
        assert_eq!(
            utils::mt_balance_of(&ctx.mt_token, &receiver.id(), TOKEN_ID).await,
            0
        );
        assert_eq!(
            ctx.mt_balance_of(&user_id).await,
            TOTAL_SUPPLY - DEPOSIT_AMOUNT + spent
        );
        assert_eq!(
            ctx.mt_balance_of(&ctx.aurora.id()).await,
            DEPOSIT_AMOUNT - spent
        );
        let refunded = if REFUND { UNUSED_AMOUNT } else { 0 };
        assert_eq!(
            ctx.erc20_balance_of(ctx.user_address()).await,
            (DEPOSIT_AMOUNT - EXIT_AMOUNT + refunded).into()
        );
        ctx.assert_locked_surplus(UNUSED_AMOUNT - refunded).await;
    }

    #[tokio::test]
    async fn test_mt_transfer_call_resolver_failure_after_transfer() {
        let ctx = setup().await;
        let receiver = deploy_ft_receiver(&ctx.aurora).await.unwrap();
        let result = ctx
            .mt_token
            .call("set_resolver_failure")
            .args_json(serde_json::json!({ "enabled": true }))
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");

        let result = exit(&ctx, &format!("{}:keep", receiver.id())).await;

        utils::assert_receipt_failed(&result, &ctx.mt_token.id(), "Resolver failure requested");
        let receiver_outcome = result
            .receipt_outcomes()
            .iter()
            .find(|outcome| outcome.executor_id.as_str() == receiver.id().as_ref())
            .expect("the receiver must execute before the resolver fails");
        assert!(receiver_outcome.is_success(), "{result:?}");
        if REFUND {
            utils::assert_receipt_failed(&result, &ctx.aurora.id(), "ERR_TRANSFER_CALL_FAILED");
        }
        let submitted: SubmitResult = result.borsh().unwrap();
        assert!(submitted.status.is_ok(), "{submitted:?}");
        // The committed transfer must not be paid again through an ERC-20 refund.
        assert_eq!(
            utils::mt_balance_of(&ctx.mt_token, &receiver.id(), TOKEN_ID).await,
            EXIT_AMOUNT
        );
        assert_eq!(
            ctx.mt_balance_of(&ctx.aurora.id()).await,
            DEPOSIT_AMOUNT - EXIT_AMOUNT
        );
        assert_eq!(
            ctx.mt_balance_of(&ctx.user.id()).await,
            TOTAL_SUPPLY - DEPOSIT_AMOUNT
        );
        assert_eq!(
            ctx.erc20_balance_of(ctx.user_address()).await,
            (DEPOSIT_AMOUNT - EXIT_AMOUNT).into()
        );
        ctx.assert_erc20_backed().await;
    }

    #[tokio::test]
    async fn test_exit_to_rejecting_receiver() {
        let ctx = setup().await;
        let receiver = deploy_ft_receiver(&ctx.aurora).await.unwrap();
        utils::set_rejected_receiver(&ctx.mt_token, &receiver.id(), true)
            .await
            .unwrap();
        let aurora_id = ctx.aurora.id();

        // A failed `mt_transfer` leaves the tokens on the engine; the callback refunds them.
        let result = exit(&ctx, receiver.id().as_ref()).await;
        utils::assert_receipt_failed(&result, &ctx.mt_token.id(), REJECTED);
        let (mut erc20_balance, mut surplus) = if REFUND {
            (DEPOSIT_AMOUNT, 0)
        } else {
            (DEPOSIT_AMOUNT - EXIT_AMOUNT, EXIT_AMOUNT)
        };
        assert_eq!(
            ctx.erc20_balance_of(ctx.user_address()).await,
            erc20_balance.into()
        );
        assert_eq!(ctx.mt_balance_of(&aurora_id).await, DEPOSIT_AMOUNT);
        ctx.assert_locked_surplus(surplus).await;

        // A failed `mt_transfer_call` reports no used amount, so the callback refunds nothing.
        let result = exit(&ctx, &format!("{}:keep", receiver.id())).await;
        utils::assert_receipt_failed(&result, &ctx.mt_token.id(), REJECTED);
        if REFUND {
            utils::assert_receipt_failed(&result, &aurora_id, "ERR_TRANSFER_CALL_FAILED");
        }
        erc20_balance -= EXIT_AMOUNT;
        surplus += EXIT_AMOUNT;
        assert_eq!(
            ctx.erc20_balance_of(ctx.user_address()).await,
            erc20_balance.into()
        );
        assert_eq!(ctx.mt_balance_of(&aurora_id).await, DEPOSIT_AMOUNT);
        ctx.assert_locked_surplus(surplus).await;
    }

    #[tokio::test]
    async fn test_exit_more_than_balance_reverts() {
        let ctx = setup().await;
        let user_id = ctx.user.id();
        let mt_token_id = ctx.mt_token.id();

        let result = exit_to_near(
            &ctx.user,
            user_id.as_ref(),
            DEPOSIT_AMOUNT + 1,
            &ctx.erc20,
            &ctx.aurora,
        )
        .await;

        assert!(
            result
                .outcomes()
                .iter()
                .all(|outcome| outcome.executor_id.as_str() != mt_token_id.as_ref()),
            "the reverted exit must not reach the token contract"
        );
        let submitted: SubmitResult = result.borsh().unwrap();
        assert!(
            matches!(&submitted.status, TransactionStatus::Revert(_)),
            "expected EVM revert, got {submitted:?}"
        );
        assert_eq!(
            ctx.mt_balance_of(&user_id).await,
            TOTAL_SUPPLY - DEPOSIT_AMOUNT
        );
        assert_eq!(ctx.mt_balance_of(&ctx.aurora.id()).await, DEPOSIT_AMOUNT);
        assert_eq!(
            ctx.erc20_balance_of(ctx.user_address()).await,
            DEPOSIT_AMOUNT.into()
        );
        ctx.assert_erc20_backed().await;
    }
}
