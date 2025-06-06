#![allow(clippy::too_many_lines)]
use crate::prelude::U256;
use crate::tests::erc20_connector::workspace::{erc20_balance, exit_to_near};
use crate::utils::workspace::{
    deploy_engine_with_code, deploy_erc20_from_nep_141, deploy_nep_141, nep_141_balance_of,
    transfer_nep_141_to_erc_20,
};
use crate::utils::{self, AuroraRunner};
use aurora_engine_types::parameters::connector::{
    Erc20Identifier, Erc20Metadata, MirrorErc20TokenArgs, SetErc20MetadataArgs,
    WithdrawSerializeType,
};
use aurora_engine_workspace::account::Account;
use aurora_engine_workspace::types::NearToken;
use aurora_engine_workspace::{EngineContract, RawContract};

const TRANSFER_AMOUNT: u128 = 1000;

#[tokio::test]
async fn test_mirroring_erc20_token() {
    utils::load_library();

    let main_contract = deploy_main_contract().await;
    let silo_contract = deploy_silo_contract(&main_contract).await;
    let (nep141, ft_owner) = deploy_nep141(&main_contract).await;
    let erc20 = deploy_erc20_from_nep_141(nep141.id().as_ref(), &main_contract)
        .await
        .unwrap();
    let erc20_metadata = Erc20Metadata {
        name: "USD Tether".to_string(),
        symbol: "USDT".to_string(),
        decimals: 18,
    };

    // Set ERC-20 metadata.
    let result = main_contract
        .set_erc20_metadata(SetErc20MetadataArgs {
            erc20_identifier: Erc20Identifier::Erc20 {
                address: erc20.0.address,
            },
            metadata: erc20_metadata.clone(),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    // Should get ERC-20 address of mirrored contract.
    let result = silo_contract
        .mirror_erc20_token(MirrorErc20TokenArgs {
            contract_id: main_contract.id(),
            nep141: nep141.id(),
        })
        .max_gas()
        .transact()
        .await;
    let erc20_address = result.unwrap().into_value();

    assert_eq!(erc20_address, erc20.0.address);
    let silo_erc20_metadata = silo_contract
        .get_erc20_metadata(Erc20Identifier::Erc20 {
            address: erc20_address,
        })
        .await
        .unwrap()
        .result;
    assert_eq!(silo_erc20_metadata, erc20_metadata);

    // We need to storage_deposit to register account id of the silo contract in the nep-141.
    let result = silo_contract
        .root()
        .call(&nep141.id(), "storage_deposit")
        .args_json(serde_json::json!({
            "account_id": silo_contract.id(),
        }))
        .deposit(NearToken::from_near(2))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());
    assert_eq!(nep_141_balance_of(&nep141, &ft_owner.id()).await, 1_000_000);

    let address = aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.id().as_bytes());

    transfer_nep_141_to_erc_20(
        &nep141,
        &erc20,
        &ft_owner,
        address,
        TRANSFER_AMOUNT,
        &main_contract, // main contract
    )
    .await
    .unwrap();
    transfer_nep_141_to_erc_20(
        &nep141,
        &erc20,
        &ft_owner,
        address,
        TRANSFER_AMOUNT,
        &silo_contract, // silo contract
    )
    .await
    .unwrap();

    assert_eq!(
        erc20_balance(&erc20, address, &main_contract).await,
        TRANSFER_AMOUNT.into()
    );
    assert_eq!(
        erc20_balance(&erc20, address, &silo_contract).await,
        TRANSFER_AMOUNT.into()
    );
    assert_eq!(
        nep_141_balance_of(&nep141, &ft_owner.id()).await,
        1_000_000 - TRANSFER_AMOUNT * 2
    );

    let result = exit_to_near(
        &ft_owner,
        ft_owner.id().as_ref(),
        TRANSFER_AMOUNT,
        &erc20,
        &main_contract,
    )
    .await;
    assert!(result.is_success());

    let result = exit_to_near(
        &ft_owner,
        ft_owner.id().as_ref(),
        TRANSFER_AMOUNT,
        &erc20,
        &silo_contract,
    )
    .await;
    assert!(result.is_success());

    assert_eq!(
        erc20_balance(&erc20, address, &main_contract).await,
        0.into()
    );
    assert_eq!(
        erc20_balance(&erc20, address, &silo_contract).await,
        0.into()
    );
    assert_eq!(nep_141_balance_of(&nep141, &ft_owner.id()).await, 1_000_000);
}

// The goal of the test is to demonstrate that the new logic of the `exit_to_near` precompile allows
// to transfer ERC-20 tokens between Aurora Engine contracts, including Silos.
#[tokio::test]
async fn test_transfer_from_silo_to_silo() {
    utils::load_library();

    let main_contract = deploy_main_contract().await;
    let silo_contract = deploy_silo_contract(&main_contract).await;
    let (nep141, ft_owner) = deploy_nep141(&main_contract).await;
    let erc20 = deploy_erc20_from_nep_141(nep141.id().as_ref(), &main_contract)
        .await
        .unwrap();
    let erc20_metadata = Erc20Metadata {
        name: "USD Tether".to_string(),
        symbol: "USDT".to_string(),
        decimals: 18,
    };

    // Set ERC-20 metadata.
    let result = main_contract
        .set_erc20_metadata(SetErc20MetadataArgs {
            erc20_identifier: Erc20Identifier::Erc20 {
                address: erc20.0.address,
            },
            metadata: erc20_metadata.clone(),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    // Should get ERC-20 address of mirrored contract.
    let result = silo_contract
        .mirror_erc20_token(MirrorErc20TokenArgs {
            contract_id: main_contract.id(),
            nep141: nep141.id(),
        })
        .max_gas()
        .transact()
        .await;
    let erc20_address = result.unwrap().into_value();
    assert_eq!(erc20_address, erc20.0.address);

    let silo_erc20_metadata = silo_contract
        .get_erc20_metadata(Erc20Identifier::Erc20 {
            address: erc20_address,
        })
        .await
        .unwrap()
        .result;
    assert_eq!(silo_erc20_metadata, erc20_metadata);

    // We need to storage_deposit to register account id of the silo contract in the nep-141.
    let result = silo_contract
        .root()
        .call(&nep141.id(), "storage_deposit")
        .args_json(serde_json::json!({
            "account_id": silo_contract.id(),
        }))
        .deposit(NearToken::from_near(2))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    assert_eq!(nep_141_balance_of(&nep141, &ft_owner.id()).await, 1_000_000);

    let address = aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.id().as_bytes());
    // Deposit address `address` via ft_transfer_call.
    let result = ft_owner
        .call(&nep141.id(), "ft_transfer_call")
        .args_json(serde_json::json!({
            "receiver_id": main_contract.id(),
            "amount": TRANSFER_AMOUNT.to_string(),
            "msg": address.encode(),
        }))
        .max_gas()
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    assert_eq!(
        erc20_balance(&erc20, address, &main_contract).await,
        TRANSFER_AMOUNT.into()
    );

    let dst = format!("{}:{}", silo_contract.id().as_ref(), address.encode());
    let withdraw_amount = 100;
    let result = exit_to_near(&ft_owner, &dst, withdraw_amount, &erc20, &main_contract).await;
    assert!(result.is_success());

    assert_eq!(
        nep_141_balance_of(&nep141, &main_contract.id()).await,
        TRANSFER_AMOUNT - withdraw_amount
    );
    assert_eq!(
        erc20_balance(&erc20, address, &main_contract).await,
        (TRANSFER_AMOUNT - withdraw_amount).into()
    );

    assert_eq!(
        nep_141_balance_of(&nep141, &silo_contract.id()).await,
        withdraw_amount
    );
    assert_eq!(
        erc20_balance(&erc20, address, &silo_contract).await,
        withdraw_amount.into()
    );
}

#[tokio::test]
async fn test_deploy_erc20_token_with_meta() {
    utils::load_library();

    let main_contract = deploy_main_contract().await;
    let (nep141, _) = deploy_nep141(&main_contract).await;
    let erc20 = deploy_erc20_from_nep_141(nep141.id().as_ref(), &main_contract)
        .await
        .unwrap();

    let metadata = main_contract
        .get_erc20_metadata(Erc20Identifier::Erc20 {
            address: erc20.0.address,
        })
        .await
        .unwrap()
        .result;

    assert_eq!(metadata.name, "Example NEAR fungible token");
    assert_eq!(metadata.symbol, "EXAMPLE");
    assert_eq!(metadata.decimals, 24);
}

async fn deploy_main_contract() -> EngineContract {
    let code = AuroraRunner::get_engine_code();
    deploy_engine_with_code(code).await
}

async fn deploy_silo_contract(main_contract: &EngineContract) -> EngineContract {
    let silo_account = main_contract
        .root()
        .create_subaccount("silo", NearToken::from_near(50))
        .await
        .unwrap();
    let silo_bytes = AuroraRunner::get_engine_code();
    let contract = silo_account.deploy(&silo_bytes).await.unwrap();
    let public_key = silo_account.public_key().unwrap();
    let silo_account_id = silo_account.id();

    let silo = EngineContract {
        account: silo_account,
        contract,
        public_key,
        node: main_contract.node.clone(),
    };

    let result = silo
        .new(
            U256::from(AuroraRunner::get_default_chain_id() + 1).to_big_endian(),
            silo_account_id,
            1,
        )
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = silo
        .set_eth_connector_contract_account(main_contract.id(), WithdrawSerializeType::Borsh)
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    silo
}

async fn deploy_nep141(main_contract: &EngineContract) -> (RawContract, Account) {
    let ft_owner = main_contract
        .root()
        .create_subaccount("ft_owner", NearToken::from_near(10))
        .await
        .unwrap();
    let nep_141_account = main_contract
        .root()
        .create_subaccount("test_token", NearToken::from_near(10))
        .await
        .unwrap();

    // Deploy nep141 token
    let contract = deploy_nep_141(&nep_141_account, &ft_owner, 1_000_000, main_contract)
        .await
        .unwrap();

    (contract, ft_owner)
}
