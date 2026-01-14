use aurora_engine_types::types::NEP141Wei;
use near_sdk::serde_json::{Value, json};
use near_sdk::{json_types::U128, serde};
use near_workspaces::types::NearToken;
use near_workspaces::{AccountId, Contract};
use std::str::FromStr;

use crate::utils::*;

const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

#[tokio::test]
async fn test_aurora_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;

    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let transfer_amount: U128 = 70.into();
    let receiver_id = contract.engine_contract.id();
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id,
            "amount": transfer_amount,
            "memo": "transfer memo"
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let balance = contract
        .eth_connector_contract
        .call("ft_balance_of")
        .args_json((&receiver_id,))
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(balance, transfer_amount);

    let balance = contract
        .eth_connector_contract
        .call("ft_balance_of")
        .args_json((user_acc.id(),))
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT - transfer_amount.0);

    let balance = contract
        .eth_connector_contract
        .call("ft_total_supply")
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    Ok(())
}

#[tokio::test]
async fn test_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let transfer_amount: U128 = 70.into();
    let receiver_id = contract.engine_contract.id();
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id,
            "amount": transfer_amount,
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - transfer_amount.0,
    );
    assert_eq!(
        contract.get_eth_on_near_balance(receiver_id).await?.0,
        transfer_amount.0,
    );
    assert_eq!(DEPOSITED_AMOUNT, contract.total_supply().await?);
    Ok(())
}

#[tokio::test]
async fn test_withdraw_eth_from_near() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let withdraw_amount = NEP141Wei::new(100);
    let res = user_acc
        .call(contract.engine_contract.id(), "withdraw")
        .args_borsh((*RECIPIENT_ADDRESS, withdraw_amount))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );
    assert_eq!(
        contract.total_supply().await?,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );
    Ok(())
}

#[tokio::test]
async fn test_deposit_eth_to_near_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let result = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(result.is_success(), "{result:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT
    );
    assert_eq!(contract.total_supply().await?, DEPOSITED_AMOUNT);

    Ok(())
}

// NOTE: We don't test relayer fee
#[tokio::test]
async fn test_deposit_eth_to_aurora_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let result = contract
        .deposit_eth_to_aurora(DEPOSITED_AMOUNT.into(), &RECIPIENT_ADDRESS)
        .await?;
    assert!(result.is_success(), "{result:#?}");

    assert_eq!(
        contract.get_eth_balance(&RECIPIENT_ADDRESS).await?,
        DEPOSITED_AMOUNT
    );
    assert_eq!(contract.total_supply().await?, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_eth() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;

    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.engine_contract.id())
            .await?
            .0,
        0,
    );

    let transfer_amount: U128 = 50.into();
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": contract.engine_contract.id(),
            "amount": transfer_amount,
            "msg": RECIPIENT_ADDRESS.encode(),
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - transfer_amount.0,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.engine_contract.id())
            .await?
            .0,
        transfer_amount.0,
    );
    assert_eq!(
        contract.get_eth_balance(&RECIPIENT_ADDRESS).await?,
        transfer_amount.0,
    );
    assert_eq!(contract.total_supply().await?, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn test_ft_transfer_call_without_message() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let result = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(result.is_success(), "{result:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.engine_contract.id())
            .await?
            .0,
        0,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.eth_connector_contract.id())
            .await?
            .0,
        0,
    );

    let transfer_amount: U128 = 50.into();
    // Send to Aurora contract with wrong message should fail
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": contract.engine_contract.id(),
            "amount": transfer_amount,
            "msg": "", // `msg` should be an address encoded in hex if the receiver is engine contract
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(contract.check_error_message(&res, "ERR_INVALID_ADDRESS")?);

    // Assert balances remain unchanged
    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.engine_contract.id())
            .await?
            .0,
        0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );

    // Sending to random account should not change balances
    let some_acc = AccountId::from_str("some-test-acc")?;
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": &some_acc,
            "amount": transfer_amount,
            "msg": ""
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    // some-test-acc does not implement `ft_on_transfer`, and therefore the call fails
    // and the transfer will be reverted.
    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT
    );
    assert_eq!(contract.get_eth_on_near_balance(&some_acc).await?.0, 0);
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.engine_contract.id())
            .await?
            .0,
        0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );

    let dummy_contract = dummy_contract(&contract).await?;
    // Sending to external receiver who implements the `ft_on_transfer` with empty message should
    // be successful.
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": dummy_contract.id(),
            "amount": transfer_amount,
            "msg": ""
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - transfer_amount.0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(dummy_contract.id())
            .await?
            .0,
        transfer_amount.0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.engine_contract.id())
            .await?
            .0,
        0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );
    assert_eq!(contract.get_eth_balance(&RECIPIENT_ADDRESS).await?, 0);
    assert_eq!(contract.total_supply().await?, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
async fn test_admin_controlled_only_admin_can_pause() -> anyhow::Result<()> {
    let contract = TestContract::new_with_owner("owner").await?;
    let user_acc = contract.create_sub_account("some-user").await?;
    let args = json!({"key": "deposit"});
    let res = user_acc
        .call(contract.eth_connector_contract.id(), "pa_pause_feature")
        .args_json(&args)
        .transact()
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(&res, "Insufficient permissions for method")?);

    let res = contract
        .owner
        .as_ref()
        .unwrap()
        .call(contract.eth_connector_contract.id(), "pa_pause_feature")
        .args_json(args)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");
    Ok(())
}

#[tokio::test]
async fn test_access_right() -> anyhow::Result<()> {
    let contract = TestContract::new_with_owner("owner").await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let res = contract
        .eth_connector_contract
        .call("get_aurora_engine_account_id")
        .view()
        .await?
        .json::<AccountId>()?;
    assert_eq!(&res, contract.engine_contract.id());

    let withdraw_amount = NEP141Wei::new(100);
    let res = user_acc
        .call(contract.eth_connector_contract.id(), "engine_withdraw")
        .args_borsh((user_acc.id(), *RECIPIENT_ADDRESS, withdraw_amount))
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(&res, "Method can be called only by aurora engine")?);

    let res = contract
        .owner
        .as_ref()
        .unwrap()
        .call(
            contract.eth_connector_contract.id(),
            "set_aurora_engine_account_id",
        )
        .args_json(json!({
            "new_aurora_engine_account_id": user_acc.id()
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let res = contract
        .eth_connector_contract
        .call("get_aurora_engine_account_id")
        .view()
        .await?
        .json::<AccountId>()?;
    assert_eq!(&res, user_acc.id());

    let res = user_acc
        .call(contract.eth_connector_contract.id(), "engine_withdraw")
        .args_borsh((user_acc.id(), *RECIPIENT_ADDRESS, withdraw_amount))
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );
    assert_eq!(
        contract.total_supply().await?,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );

    Ok(())
}

#[tokio::test]
async fn test_withdraw_from_near_pausability() -> anyhow::Result<()> {
    let contract = TestContract::new_with_owner("owner").await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");
    let res = contract
        .deposit_eth_to_near(
            contract.owner.as_ref().unwrap().id(),
            DEPOSITED_AMOUNT.into(),
        )
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let pause_args = json!({"key": "engine_withdraw"});

    let withdraw_amount = NEP141Wei::new(100);
    // 1st withdraw - should succeed
    let res = user_acc
        .call(contract.engine_contract.id(), "withdraw")
        .args_borsh((*RECIPIENT_ADDRESS, withdraw_amount))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    // Pause withdraw
    let res = contract
        .owner
        .as_ref()
        .unwrap()
        .call(contract.eth_connector_contract.id(), "pa_pause_feature")
        .args_json(&pause_args)
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    // 2nd withdraw - should be failed
    let res = user_acc
        .call(contract.engine_contract.id(), "withdraw")
        .args_borsh((*RECIPIENT_ADDRESS, withdraw_amount))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(&res, "Pausable: Method is paused")?);

    // Direct call to eth-connector from owner should be success
    let res = contract
        .owner
        .as_ref()
        .unwrap()
        .call(contract.eth_connector_contract.id(), "withdraw")
        .args_borsh((*RECIPIENT_ADDRESS, withdraw_amount))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success());

    // Unpause all
    let res = contract
        .owner
        .as_ref()
        .unwrap()
        .call(contract.eth_connector_contract.id(), "pa_unpause_feature")
        .args_json(pause_args)
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let res = user_acc
        .call(contract.engine_contract.id(), "withdraw")
        .args_borsh((*RECIPIENT_ADDRESS, withdraw_amount))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.total_supply().await?,
        DEPOSITED_AMOUNT * 2 - 3 * withdraw_amount.as_u128()
    );
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_max_value() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let transfer_amount: U128 = u128::MAX.into();
    let receiver_id = contract.engine_contract.id();
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id,
            "amount": transfer_amount,
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(&res, "The account doesn't have enough balance")?);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_empty_value() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let receiver_id = contract.engine_contract.id();
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id,
            "amount": "", // empty string
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(&res, "cannot parse integer from empty string")?);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_wrong_u128_json_type() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success());

    let transfer_amount = 200;
    let receiver_id = AccountId::from_str(DEPOSITED_RECIPIENT)?;
    let res = contract
        .engine_contract
        .call("ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id,
            "amount": transfer_amount, // should be serialized in string, but i32 is not.
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_failure());
    assert!(contract.check_error_message(&res, "Wait for a string")?);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_user() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success());

    let transfer_amount: U128 = 70.into();
    let receiver_id = contract.create_sub_account("some-acc").await?;
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id.id(),
            "amount": transfer_amount,
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - transfer_amount.0,
    );
    assert_eq!(
        contract.get_eth_on_near_balance(receiver_id.id()).await?.0,
        transfer_amount.0,
    );
    assert_eq!(DEPOSITED_AMOUNT, contract.total_supply().await?);

    let transfer_amount2: U128 = 1000.into();
    let res = user_acc
        .call(contract.engine_contract.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": &receiver_id.id(),
            "amount": transfer_amount2,
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");
    assert_eq!(
        contract.get_eth_on_near_balance(receiver_id.id()).await?.0,
        transfer_amount.0 + transfer_amount2.0,
    );
    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - transfer_amount.0 - transfer_amount2.0,
    );
    Ok(())
}

#[tokio::test]
async fn test_withdraw_from_user() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let user_acc = contract
        .create_sub_account(DEPOSITED_RECIPIENT_NAME)
        .await?;
    let res = contract
        .deposit_eth_to_near(user_acc.id(), DEPOSITED_AMOUNT.into())
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let withdraw_amount = NEP141Wei::new(130);
    let res = user_acc
        .call(contract.engine_contract.id(), "withdraw")
        .args_borsh((*RECIPIENT_ADDRESS, withdraw_amount))
        .deposit(ONE_YOCTO)
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(user_acc.id()).await?.0,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128()
    );
    assert_eq!(
        contract.total_supply().await?,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );

    Ok(())
}

#[tokio::test]
async fn test_ft_metadata() -> anyhow::Result<()> {
    use aurora_engine_types::parameters::connector::FungibleTokenMetadata as ft_m;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct FungibleTokenMetadata {
        pub spec: String,
        pub name: String,
        pub symbol: String,
        pub icon: Option<String>,
        pub reference: Option<String>,
        pub reference_hash: Option<[u8; 32]>,
        pub decimals: u8,
    }

    let contract = TestContract::new().await?;
    let metadata = contract
        .engine_contract
        .call("ft_metadata")
        .max_gas()
        .transact()
        .await?
        .into_result()?
        .json::<FungibleTokenMetadata>()?;

    let m = ft_m::default();
    let reference_hash = m.reference_hash.map(|h| {
        let x: [u8; 32] = h.as_ref().try_into().unwrap();
        x
    });
    assert_eq!(metadata.spec, m.spec);
    assert_eq!(metadata.decimals, m.decimals);
    assert_eq!(metadata.icon, m.icon);
    assert_eq!(metadata.name, m.name);
    assert_eq!(metadata.reference, m.reference);
    assert_eq!(metadata.reference_hash, reference_hash);
    assert_eq!(metadata.symbol, m.symbol);
    Ok(())
}

#[tokio::test]
async fn test_storage_deposit() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;

    let res = contract
        .engine_contract
        .call("storage_deposit")
        .args_json(json!({
            "account_id": "account.near",
        }))
        .deposit(NearToken::from_yoctonear(1250000000000000000000))
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");

    let res = contract
        .engine_contract
        .call("storage_balance_of")
        .args_json(json!({
            "account_id": "account.near",
        }))
        .max_gas()
        .transact()
        .await?
        .json::<Value>()?;

    // The NEP-141 implementation of ETH intentionally set the storage deposit amount equal to 0
    // so any non-zero deposit amount is automatically returned to the user, leaving 0 storage
    // balance behind.
    assert_eq!(res, json!({"available": "0", "total": "0"}));

    Ok(())
}

#[tokio::test]
async fn test_storage_unregister() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;

    let res = contract
        .engine_contract
        .call("storage_unregister")
        .args_json(json!({
            "force": true,
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await?;
    assert!(res.is_success(), "{res:#?}");
    Ok(())
}

async fn dummy_contract(contract: &TestContract) -> anyhow::Result<Contract> {
    contract
        .create_sub_account("ft-rec")
        .await?
        .deploy(&dummy_ft_receiver_bytes())
        .await?
        .into_result()
        .map_err(Into::into)
}
