//! Exits to the Defuse (`intents.near`) contract check the NEP-245 exit gas constants against
//! a real multi-token implementation. The release WASM is downloaded on the first run.
mod workspace {
    use aurora_engine_sdk::types::near_account_to_evm_address;
    use aurora_engine_types::parameters::connector::Nep245Token;
    use aurora_engine_workspace::account::Account;
    use aurora_engine_workspace::types::NearToken;
    use aurora_engine_workspace::{EngineContract, RawContract};
    use near_sdk::json_types::U128;
    use serde_json::json;

    use crate::tests::erc20_connector::workspace::{erc20_balance, exit_to_near};
    use crate::tests::nep245::utils;
    use crate::utils::workspace::{
        create_sub_account, deploy_engine, deploy_ft_receiver, deploy_nep_141, download_wasm,
    };

    const DEFUSE_URL: &str =
        "https://github.com/near/intents/releases/download/defuse/v0.4.2/defuse.wasm";
    const DEFUSE_SHA256: &str = "f67f9559b3b512c52dfa11723a28b9119bcf773ac01780370d6409e5f1fb20e5";
    const TOTAL_SUPPLY: u128 = 1_000_000;
    const DEPOSIT_AMOUNT: u128 = 300_000;
    const EXIT_AMOUNT: u128 = 50_000;
    const UNUSED_AMOUNT: u128 = 20_000;

    async fn deploy_defuse(aurora: &EngineContract) -> RawContract {
        let wasm = download_wasm(DEFUSE_URL, "defuse-v0.4.2.wasm", DEFUSE_SHA256)
            .await
            .unwrap();
        // Storage staking of the 1.4 MB contract takes about 15 NEAR.
        let account = create_sub_account(&aurora.root(), "defuse", NearToken::from_near(30))
            .await
            .unwrap();
        let defuse = account.deploy(&wasm).await.unwrap();
        let result = defuse
            .call("new")
            .args_json(json!({
                "config": {
                    "wnear_id": aurora.root().id(),
                    "fees": { "fee": 0, "fee_collector": defuse.id() },
                },
            }))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");
        defuse
    }

    /// Deploys the engine and Defuse, and deposits a user's NEP-141 tokens to Defuse, which
    /// tracks them as `nep141:<contract>`. Returns the engine, Defuse, the user and the token ID.
    async fn setup() -> (EngineContract, RawContract, Account, String) {
        let aurora = deploy_engine().await;
        let root = aurora.root();
        let user = create_sub_account(&root, "user", NearToken::from_near(50))
            .await
            .unwrap();
        let ft_account = create_sub_account(&root, "ft", NearToken::from_near(50))
            .await
            .unwrap();
        let ft = deploy_nep_141(&ft_account, &user, TOTAL_SUPPLY, &aurora)
            .await
            .unwrap();
        let defuse = deploy_defuse(&aurora).await;

        let result = root
            .call(&ft.id(), "storage_deposit")
            .args_json(json!({ "account_id": defuse.id() }))
            .deposit(NearToken::from_near(1))
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");
        let result = user
            .call(&ft.id(), "ft_transfer_call")
            .args_json(json!({
                "receiver_id": defuse.id(),
                "amount": U128(TOTAL_SUPPLY),
                "msg": "",
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");
        let token_id = format!("nep141:{}", ft.id());
        assert_eq!(
            utils::mt_balance_of(&defuse, &user.id(), &token_id).await,
            TOTAL_SUPPLY
        );

        (aurora, defuse, user, token_id)
    }

    #[tokio::test]
    async fn test_exit_to_defuse() {
        let (aurora, defuse, user, token_id) = setup().await;

        // Defuse has no NEP-245 metadata method, so the ERC-20 gets the default metadata.
        let token = Nep245Token {
            account_id: defuse.id(),
            token_id: token_id.clone(),
        };
        let erc20 = utils::deploy_erc20_from_nep245(token, &aurora)
            .await
            .unwrap();
        let holder = near_account_to_evm_address(&user.id());
        let result = utils::mt_transfer_call(
            &user,
            &defuse,
            &aurora.id(),
            &token_id,
            DEPOSIT_AMOUNT,
            holder.encode().as_str(),
        )
        .await
        .unwrap();
        assert!(result.receipt_failures().is_empty(), "{result:?}");
        assert_eq!(result.json::<Vec<U128>>().unwrap(), [U128(DEPOSIT_AMOUNT)]);

        let receiver = deploy_ft_receiver(&aurora).await.unwrap();
        let cases = [
            (user.id().to_string(), user.id(), EXIT_AMOUNT),
            (
                format!("{}:keep", receiver.id()),
                receiver.id(),
                EXIT_AMOUNT,
            ),
            (
                format!("{}:return:{UNUSED_AMOUNT}", receiver.id()),
                receiver.id(),
                EXIT_AMOUNT - UNUSED_AMOUNT,
            ),
        ];
        let mut locked = DEPOSIT_AMOUNT;
        let mut expected_erc20 = DEPOSIT_AMOUNT;

        for (recipient, recipient_id, used) in cases {
            let before = utils::mt_balance_of(&defuse, &recipient_id, &token_id).await;
            let result = exit_to_near(&user, &recipient, EXIT_AMOUNT, &erc20, &aurora).await;

            // The attached gas must cover Defuse's `mt_transfer` and `mt_transfer_call`.
            assert!(result.receipt_failures().is_empty(), "{result:?}");
            for outcome in result.receipt_outcomes() {
                if outcome.executor_id.as_str() == defuse.id().as_ref() {
                    eprintln!(
                        "{recipient}: Defuse receipt burnt {} GGas",
                        outcome.gas_burnt.as_ggas()
                    );
                }
            }
            locked -= used;
            expected_erc20 -= if cfg!(feature = "error_refund") {
                used
            } else {
                EXIT_AMOUNT
            };
            assert_eq!(
                utils::mt_balance_of(&defuse, &recipient_id, &token_id).await,
                before + used,
                "{recipient}"
            );
            assert_eq!(
                utils::mt_balance_of(&defuse, &aurora.id(), &token_id).await,
                locked,
                "{recipient}"
            );
            assert_eq!(
                erc20_balance(&erc20, holder, &aurora).await,
                expected_erc20.into(),
                "{recipient}"
            );
        }
    }
}
