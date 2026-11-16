mod workspace {
    use aurora_engine_types::parameters::connector::{
        BridgedToken, Erc20Identifier, Erc20Metadata, Nep245Token,
    };
    use aurora_engine_workspace::EngineContract;
    use serde_json::json;

    use crate::prelude::Address;
    use crate::tests::nep245::utils::{self, TOKEN_ID};
    use crate::utils::workspace::deploy_engine;

    /// Exercises JSON escaping of the token ID in the metadata request.
    const QUOTED_TOKEN_ID: &str = r#"nep171:nft.near:"1""#;

    async fn erc20_metadata(aurora: &EngineContract, token: Nep245Token) -> Erc20Metadata {
        aurora
            .get_erc20_metadata(Erc20Identifier::from(BridgedToken::Nep245(token)))
            .await
            .unwrap()
            .result
    }

    #[tokio::test]
    async fn test_deploy_erc20_token_reads_nep245_metadata() {
        let ctx = utils::setup().await;

        assert_eq!(
            erc20_metadata(&ctx.aurora, ctx.token()).await,
            Erc20Metadata {
                name: "Token".into(),
                symbol: "TKN".into(),
                decimals: 6,
            }
        );
    }

    #[tokio::test]
    async fn test_deploy_erc20_token_defaults_missing_nep245_metadata() {
        let aurora = deploy_engine().await;
        let mt_token = utils::deploy_mt_token(&aurora).await.unwrap();
        let partial = Nep245Token {
            account_id: mt_token.id(),
            token_id: QUOTED_TOKEN_ID.into(),
        };
        let metadata = json!({ "name": "Quoted", "symbol": null, "decimals": null });
        utils::mint(&mt_token, &aurora.id(), QUOTED_TOKEN_ID, 1, Some(metadata))
            .await
            .unwrap();
        let cases = [
            (
                partial,
                Erc20Metadata {
                    name: "Quoted".into(),
                    ..Erc20Metadata::default()
                },
            ),
            // `mt_metadata_base_by_token_id` returns `[null]` for an unknown token.
            (
                Nep245Token {
                    account_id: mt_token.id(),
                    token_id: "unknown".into(),
                },
                Erc20Metadata::default(),
            ),
            // The metadata promise fails on an account without a contract.
            (
                Nep245Token {
                    account_id: aurora.root().id(),
                    token_id: TOKEN_ID.into(),
                },
                Erc20Metadata::default(),
            ),
        ];

        for (token, expected) in cases {
            utils::deploy_erc20_from_nep245(token.clone(), &aurora)
                .await
                .unwrap();
            assert_eq!(erc20_metadata(&aurora, token).await, expected);
        }
    }

    #[tokio::test]
    async fn test_deploy_erc20_token_rejects_registered_nep245_token() {
        let ctx = utils::setup().await;

        let error = utils::deploy_erc20_from_nep245(ctx.token(), &ctx.aurora)
            .await
            .err()
            .unwrap();

        assert!(
            error
                .to_string()
                .contains("ERR_NEP245_TOKEN_ALREADY_REGISTERED"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn test_nep245_tokens_map_to_distinct_erc20_contracts() {
        let ctx = utils::setup().await;
        let aurora = &ctx.aurora;
        let quoted = Nep245Token {
            account_id: ctx.mt_token.id(),
            token_id: QUOTED_TOKEN_ID.into(),
        };
        let quoted_erc20 = utils::deploy_erc20_from_nep245(quoted.clone(), aurora)
            .await
            .unwrap();
        // The same account bridged as a NEP-141 token gets its own ERC-20 contract.
        let nep141_erc20: Address = aurora
            .deploy_erc20_token_legacy(ctx.mt_token.id())
            .max_gas()
            .transact()
            .await
            .unwrap()
            .into_value();

        let nep245_tokens = [
            (ctx.token(), ctx.erc20.0.address),
            (quoted, quoted_erc20.0.address),
        ];
        assert_ne!(nep245_tokens[0].1, nep245_tokens[1].1);
        for (token, address) in nep245_tokens {
            assert_ne!(address, nep141_erc20);
            assert_eq!(
                aurora
                    .get_erc20_from_nep245(token.clone())
                    .await
                    .unwrap()
                    .result,
                address
            );
            assert_eq!(
                aurora.get_nep245_from_erc20(address).await.unwrap().result,
                token
            );
            assert!(aurora.get_nep141_from_erc20(address).await.is_err());
        }
        assert_eq!(
            aurora
                .get_erc20_from_nep141(ctx.mt_token.id())
                .await
                .unwrap()
                .result,
            nep141_erc20
        );
        assert!(aurora.get_nep245_from_erc20(nep141_erc20).await.is_err());
    }
}
