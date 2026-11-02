use aurora_engine_types::parameters::connector::{
    BridgedToken, Erc20Identifier, Erc20Metadata, MirrorErc20TokenArgs, Nep245Token,
};
use aurora_engine_types::{borsh, types::Address, types::PromiseResult};
use near_vm_runner::logic::mocks::mock_external::MockAction;
use near_vm_runner::logic::types::ReturnData;

#[test]
#[allow(clippy::too_many_lines)]
fn test_mirroring_nep245_erc20_token_wasm() {
    let mut runner = crate::utils::deploy_runner();
    let owner = runner.aurora_account_id.clone();
    for (token_id, address) in [
        ("first.token", Address::from_array([0x11; 20])),
        ("second.\"token", Address::from_array([0x22; 20])),
    ] {
        let token = Nep245Token {
            account_id: "multi.near".parse().unwrap(),
            token_id: token_id.into(),
        };
        let bridged_token = BridgedToken::Nep245(token.clone());
        let identifier = Erc20Identifier::from(bridged_token.clone());
        let args = MirrorErc20TokenArgs {
            contract_id: "main.near".parse().unwrap(),
            token: bridged_token,
        };
        let input = borsh::to_vec(&args).unwrap();
        runner.ext.underlying.action_log.clear();
        // Scheduling does not change EVM state and is not a standalone transaction kind.
        let standalone = runner.standalone_runner.take();
        runner
            .call("mirror_erc20_token", &owner, input.clone())
            .unwrap();
        runner.standalone_runner = standalone;

        let calls: Vec<_> = runner
            .ext
            .underlying
            .action_log
            .iter()
            .filter_map(|action| {
                if let MockAction::FunctionCallWeight {
                    method_name, args, ..
                } = action
                {
                    Some((method_name.as_slice(), args.as_slice()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            calls,
            vec![
                (
                    b"get_erc20_from_nep245".as_slice(),
                    borsh::to_vec(&token).unwrap().as_slice()
                ),
                (
                    b"get_erc20_metadata".as_slice(),
                    serde_json::to_vec(&identifier).unwrap().as_slice()
                ),
                (b"mirror_erc20_token_callback".as_slice(), input.as_slice()),
            ]
        );
        let callback_gas = runner
            .ext
            .underlying
            .action_log
            .iter()
            .find_map(|action| match action {
                MockAction::FunctionCallWeight {
                    method_name,
                    prepaid_gas,
                    ..
                } if method_name == b"mirror_erc20_token_callback" => Some(*prepaid_gas),
                _ => None,
            })
            .unwrap();

        let metadata = Erc20Metadata {
            name: token_id.into(),
            symbol: "MT".into(),
            decimals: 6,
        };
        runner.promise_results = vec![
            PromiseResult::Successful(address.as_bytes().to_vec()),
            PromiseResult::Successful(serde_json::to_vec(&metadata).unwrap()),
        ];
        let prepaid_gas = runner.context.prepaid_gas;
        runner.context.prepaid_gas = callback_gas;
        let outcome = runner
            .call("mirror_erc20_token_callback", &owner, input)
            .unwrap();
        runner.context.prepaid_gas = prepaid_gas;
        assert_eq!(
            outcome.return_data,
            ReturnData::Value(borsh::to_vec(address.as_bytes()).unwrap())
        );
        runner.promise_results.clear();

        let outcome = runner
            .one_shot()
            .call(
                "get_erc20_from_nep245",
                &owner,
                borsh::to_vec(&token).unwrap(),
            )
            .unwrap();
        assert_eq!(
            outcome.return_data,
            ReturnData::Value(address.as_bytes().to_vec())
        );
        let error = runner
            .one_shot()
            .call(
                "get_erc20_from_nep141",
                &owner,
                borsh::to_vec(&token.account_id).unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            error.kind.as_bytes(),
            aurora_engine::errors::ERR_NEP141_NOT_FOUND
        );
        let outcome = runner
            .one_shot()
            .call(
                "get_erc20_metadata",
                &owner,
                serde_json::to_vec(&identifier).unwrap(),
            )
            .unwrap();
        assert_eq!(
            outcome.return_data,
            ReturnData::Value(serde_json::to_vec(&metadata).unwrap())
        );
    }
}

mod workspace {
    use aurora_engine_types::parameters::connector::{
        BridgedToken, Erc20Identifier, Erc20Metadata, MirrorErc20TokenArgs, Nep245Token,
        SetErc20MetadataArgs,
    };
    use aurora_engine_types::parameters::engine::DeployErc20TokenArgs;

    use crate::tests::erc20_mirror::{deploy_main_contract, deploy_silo_contract};

    #[tokio::test]
    async fn test_mirroring_nep245_erc20_tokens() {
        let main_contract = deploy_main_contract().await;
        let silo_contract = deploy_silo_contract(&main_contract).await;
        let mut addresses = Vec::new();

        // Metadata for each ERC-20 is set below; this account needs no NEP-245 contract.
        for (token_id, decimals) in [("first.token", 6), ("second.\"token", 18)] {
            let token = Nep245Token {
                account_id: main_contract.root().id(),
                token_id: token_id.into(),
            };
            let address = main_contract
                .deploy_erc20_token(DeployErc20TokenArgs::Nep245(token.clone()))
                .max_gas()
                .transact()
                .await
                .unwrap()
                .into_value();
            let bridged_token = BridgedToken::Nep245(token.clone());
            let identifier = Erc20Identifier::from(bridged_token.clone());
            let metadata = Erc20Metadata {
                name: token_id.into(),
                symbol: "MT".into(),
                decimals,
            };
            let result = main_contract
                .set_erc20_metadata(SetErc20MetadataArgs {
                    erc20_identifier: identifier.clone(),
                    metadata: metadata.clone(),
                })
                .max_gas()
                .transact()
                .await
                .unwrap();
            assert!(result.is_success());

            let mirrored = silo_contract
                .mirror_erc20_token(MirrorErc20TokenArgs {
                    contract_id: main_contract.id(),
                    token: bridged_token,
                })
                .max_gas()
                .transact()
                .await
                .unwrap()
                .into_value();
            assert_eq!(mirrored, address);
            assert_eq!(
                silo_contract
                    .get_erc20_metadata(identifier)
                    .await
                    .unwrap()
                    .result,
                metadata
            );
            let mapped = silo_contract
                .get_erc20_from_nep245(token)
                .await
                .unwrap()
                .result;
            assert_eq!(mapped, address);
            addresses.push(address);
        }
        assert_ne!(addresses[0], addresses[1]);
    }
}
