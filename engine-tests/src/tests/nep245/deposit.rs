use aurora_engine_types::parameters::connector::Nep245Token;
use near_vm_runner::logic::types::ReturnData;

use crate::prelude::{Address, U256};
use crate::tests::nep245::utils::{self, TOKEN_ID};
use crate::utils::AuroraRunner;

const MT_TOKEN: &str = "mt.near";
const SENDER: &str = "user.near";
const DEPOSIT_AMOUNT: u128 = 300_000;

fn mt_on_transfer_args(token_ids: &[&str], msg: &str) -> Vec<u8> {
    utils::mt_on_transfer_args(SENDER, token_ids, DEPOSIT_AMOUNT, msg)
}

#[test]
fn test_mt_on_transfer_wasm() {
    let mut runner = AuroraRunner::new();
    let owner = runner.aurora_account_id.clone();
    let token = Nep245Token {
        account_id: MT_TOKEN.parse().unwrap(),
        token_id: TOKEN_ID.into(),
    };
    let erc20 = utils::deploy_erc20_wasm(&mut runner, token);
    let recipient = Address::from_array([0x11; 20]);
    let msg = recipient.encode();

    // The export must abort the receipt, so the NEP-245 resolver refunds the sender.
    for (token_ids, expected) in [
        (
            &["unregistered"][..],
            aurora_engine::errors::ERR_NEP245_NOT_FOUND,
        ),
        (&[TOKEN_ID, TOKEN_ID][..], b"ERR_ARGS".as_slice()),
    ] {
        let args = mt_on_transfer_args(token_ids, msg.as_str());
        let error = runner.call("mt_on_transfer", MT_TOKEN, args).unwrap_err();
        assert_eq!(error.kind.as_bytes(), expected);
    }
    assert_eq!(runner.balance_of(erc20, recipient, &owner), U256::zero());

    let args = mt_on_transfer_args(&[TOKEN_ID], msg.as_str());
    let outcome = runner.call("mt_on_transfer", MT_TOKEN, args).unwrap();

    assert_eq!(outcome.return_data, ReturnData::Value(br#"["0"]"#.to_vec()));
    assert_eq!(
        runner.balance_of(erc20, recipient, &owner),
        DEPOSIT_AMOUNT.into()
    );
}

mod workspace {
    use aurora_engine_types::storage::{KeyPrefix, address_to_key};
    use near_sdk::json_types::U128;

    use super::DEPOSIT_AMOUNT;
    use crate::prelude::Address;
    use crate::tests::nep245::utils::{self, TOKEN_ID, TOTAL_SUPPLY};

    const UNREGISTERED_TOKEN_ID: &str = "nep141:unregistered.near";

    #[tokio::test]
    async fn test_mt_transfer_call_mints_erc20() {
        let ctx = utils::setup().await;
        let recipient = Address::from_array([0x11; 20]);

        ctx.deposit(DEPOSIT_AMOUNT, recipient).await.unwrap();

        assert_eq!(
            ctx.mt_balance_of(&ctx.user.id()).await,
            TOTAL_SUPPLY - DEPOSIT_AMOUNT
        );
        assert_eq!(ctx.mt_balance_of(&ctx.aurora.id()).await, DEPOSIT_AMOUNT);
        assert_eq!(ctx.erc20_balance_of(recipient).await, DEPOSIT_AMOUNT.into());
        ctx.assert_erc20_backed().await;
    }

    #[tokio::test]
    async fn test_mt_on_transfer_mint_failure_rolls_back_and_refunds() {
        let ctx = utils::setup().await;
        let recipient = ctx.user_address();
        ctx.deposit(DEPOSIT_AMOUNT, recipient).await.unwrap();
        let erc20_address = ctx.erc20.0.address;
        let original_code = ctx.aurora.get_code(erc20_address).await.unwrap().result;
        let code_key = address_to_key(KeyPrefix::Code, &erc20_address);
        let engine_id = ctx.aurora.id().as_ref().parse().unwrap();
        let worker = ctx.aurora.node.worker();
        // PUSH0 PUSH0 REVERT makes the mint fail. The engine still applies the reverted
        // call's state changes, such as the caller's nonce, so only the aborted receipt
        // keeps the engine state unchanged.
        let reverting_code = [0x5f, 0x5f, 0xfd];
        worker
            .patch_state(&engine_id, &code_key, &reverting_code)
            .await
            .unwrap();
        let before = worker.view_state(&engine_id).await.unwrap();

        let result = utils::mt_transfer_call(
            &ctx.user,
            &ctx.mt_token,
            &ctx.aurora.id(),
            TOKEN_ID,
            DEPOSIT_AMOUNT,
            recipient.encode().as_str(),
        )
        .await
        .unwrap();

        assert!(result.is_success(), "{result:?}");
        utils::assert_receipt_failed(&result, &ctx.aurora.id(), "Reverted with message:");
        assert_eq!(result.json::<Vec<U128>>().unwrap(), [U128(0)]);
        assert_eq!(worker.view_state(&engine_id).await.unwrap(), before);
        assert_eq!(
            ctx.mt_balance_of(&ctx.user.id()).await,
            TOTAL_SUPPLY - DEPOSIT_AMOUNT
        );
        assert_eq!(ctx.mt_balance_of(&ctx.aurora.id()).await, DEPOSIT_AMOUNT);

        worker
            .patch_state(&engine_id, &code_key, &original_code)
            .await
            .unwrap();
        assert_eq!(ctx.erc20_balance_of(recipient).await, DEPOSIT_AMOUNT.into());
        ctx.assert_erc20_backed().await;
        // A subsequent valid deposit still mints exactly once.
        ctx.deposit(DEPOSIT_AMOUNT, recipient).await.unwrap();
        assert_eq!(
            ctx.erc20_balance_of(recipient).await,
            (DEPOSIT_AMOUNT * 2).into()
        );
        ctx.assert_erc20_backed().await;
    }

    #[tokio::test]
    async fn test_failed_mt_on_transfer_refunds_sender() {
        let ctx = utils::setup().await;
        let user_id = ctx.user.id();
        let aurora_id = ctx.aurora.id();
        utils::mint(
            &ctx.mt_token,
            &user_id,
            UNREGISTERED_TOKEN_ID,
            TOTAL_SUPPLY,
            None,
        )
        .await
        .unwrap();
        let recipient = Address::from_array([0x11; 20]).encode();
        let cases = [
            (
                &[(UNREGISTERED_TOKEN_ID, DEPOSIT_AMOUNT)][..],
                recipient.as_str(),
                "ERR_NEP245_NOT_FOUND",
            ),
            (
                &[(TOKEN_ID, DEPOSIT_AMOUNT)][..],
                "not an address",
                "ERR_INVALID_ON_TRANSFER_MESSAGE_DATA",
            ),
            // Each token would need its own EVM transaction, so batches are rejected.
            (
                &[
                    (TOKEN_ID, DEPOSIT_AMOUNT),
                    (UNREGISTERED_TOKEN_ID, DEPOSIT_AMOUNT),
                ],
                recipient.as_str(),
                "ERR_ARGS",
            ),
        ];

        for (tokens, msg, error) in cases {
            let result =
                utils::mt_batch_transfer_call(&ctx.user, &ctx.mt_token, &aurora_id, tokens, msg)
                    .await
                    .unwrap();

            assert!(result.is_success(), "{result:?}");
            utils::assert_receipt_failed(&result, &aurora_id, error);
            let used: Vec<U128> = result.json().unwrap();
            assert_eq!(used, vec![U128(0); tokens.len()]);
        }

        for token_id in [TOKEN_ID, UNREGISTERED_TOKEN_ID] {
            assert_eq!(
                utils::mt_balance_of(&ctx.mt_token, &user_id, token_id).await,
                TOTAL_SUPPLY
            );
            assert_eq!(
                utils::mt_balance_of(&ctx.mt_token, &aurora_id, token_id).await,
                0
            );
        }
        ctx.assert_erc20_backed().await;
    }
}
