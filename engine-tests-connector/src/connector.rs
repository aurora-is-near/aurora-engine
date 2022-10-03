use crate::utils::*;
use aurora_engine::parameters::WithdrawResult;
use aurora_engine_types::types::NEP141Wei;
use aurora_engine_types::U256;
use byte_slice_cast::AsByteSlice;
use near_sdk::json_types::U128;
use near_sdk::ONE_YOCTO;
use workspaces::AccountId;

/// Bytes for a NEAR smart contract implementing `ft_on_transfer`
fn dummy_ft_receiver_bytes() -> Vec<u8> {
    let base_path = std::path::Path::new("../etc")
        .join("tests")
        .join("ft-receiver");
    let output_path = base_path.join("target/wasm32-unknown-unknown/release/ft_receiver.wasm");
    crate::rust::compile(base_path);
    std::fs::read(output_path).unwrap()
}

#[tokio::test]
async fn test_aurora_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    let proof = contract.get_proof(PROOF_DATA_NEAR);
    let res = contract
        .eth_connector_contract
        .call("deposit")
        .args_borsh(proof)
        .gas(DEFAULT_GAS)
        .transact()
        .await?;
    assert!(res.is_success());

    let transfer_amount = 70;
    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let res = contract
        .eth_connector_contract
        .call("ft_transfer")
        .args_json((&receiver_id, transfer_amount.to_string(), "transfer memo"))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    let balance = contract
        .eth_connector_contract
        .call("ft_balance_of")
        .args_json((&receiver_id,))
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(
        balance.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = contract
        .eth_connector_contract
        .call("ft_balance_of")
        .args_json((&contract.eth_connector_contract.id(),))
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(balance.0, DEPOSITED_FEE - transfer_amount as u128);

    let balance = contract
        .eth_connector_contract
        .call("ft_total_supply")
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    let balance = contract
        .eth_connector_contract
        .call("ft_total_eth_supply_on_near")
        .view()
        .await?
        .json::<U128>()
        .unwrap();
    assert_eq!(balance.0, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let transfer_amount = 70;
    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let res = contract
        .engine_contract
        .call("ft_transfer")
        .args_json((&receiver_id, transfer_amount.to_string(), "transfer memo"))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE - transfer_amount as u128,
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(DEPOSITED_AMOUNT, contract.total_supply().await?.0);
    Ok(())
}

#[tokio::test]
async fn test_withdraw_eth_from_near() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let withdraw_amount = NEP141Wei::new(100);
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);
    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let res = contract
        .engine_contract
        .call("withdraw")
        .args_borsh((recipient_addr, withdraw_amount))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    let data: WithdrawResult = res.borsh()?;
    let custodian_addr = validate_eth_address(CUSTODIAN_ADDRESS);
    assert_eq!(data.recipient_id, recipient_addr);
    assert_eq!(data.amount, withdraw_amount);
    assert_eq!(data.eth_custodian_address, custodian_addr);

    assert_eq!(
        contract
            .get_eth_on_near_balance(&&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE - withdraw_amount.as_u128(),
    );
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract.total_supply().await?.0,
        DEPOSITED_AMOUNT - withdraw_amount.as_u128(),
    );
    Ok(())
}

#[tokio::test]
async fn test_deposit_eth_to_near_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;
    assert!(
        contract.call_is_used_proof(PROOF_DATA_NEAR).await?,
        "Expected not to fail because the proof should have been already used",
    );

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE
    );
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_AMOUNT);
    Ok(())
}

// NOTE: We don't test relayer fee
#[tokio::test]
async fn test_deposit_eth_to_aurora_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_aurora().await?;
    assert!(
        contract.call_is_used_proof(PROOF_DATA_ETH).await?,
        "Expected not to fail because the proof should have been already used",
    );

    assert_eq!(
        contract
            .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS),)
            .await?,
        DEPOSITED_EVM_AMOUNT
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_EVM_AMOUNT,
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_EVM_AMOUNT,);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_eth() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE,
    );

    let transfer_amount: U128 = 50.into();
    let fee: u128 = 30;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(
        &mut validate_eth_address(RECIPIENT_ETH_ADDRESS)
            .as_bytes()
            .to_vec(),
    );

    let message = [CONTRACT_ACC, hex::encode(msg).as_str()].join(":");
    let memo: Option<String> = None;
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((
            contract.engine_contract.id(),
            transfer_amount,
            memo,
            message,
        ))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS),)
            .await?,
        transfer_amount.0,
    );
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT,
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_AMOUNT);
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_without_message() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE,
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0,
    );

    let transfer_amount: U128 = 50.into();
    let memo: Option<String> = None;
    let message = "";
    // Send to Aurora contract with wrong message should failed
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((
            contract.engine_contract.id(),
            transfer_amount,
            &memo,
            message,
        ))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(contract.check_error_message(res, "ERR_INVALID_ON_TRANSFER_MESSAGE_FORMAT"));

    // Assert balances remain unchanged
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );

    // Sending to random account should not change balances
    let some_acc = AccountId::try_from("some-test-acc".to_string()).unwrap();
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((&some_acc, transfer_amount, &memo, message))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    // some-test-acc does not implement `ft_on_transfer` therefore the call fails and the transfer is reverted.
    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(contract.get_eth_on_near_balance(&some_acc).await?.0, 0);
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );

    let dummy_contract = contract
        .create_sub_account("ft-rec")
        .await?
        .deploy(&dummy_ft_receiver_bytes()[..])
        .await?
        .into_result()?;

    // Sending to external receiver with empty message should be success
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((&dummy_contract.id(), transfer_amount, &memo, message))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.is_success());

    assert_eq!(
        contract.get_eth_on_near_balance(&receiver_id).await?.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&dummy_contract.id())
            .await?
            .0,
        transfer_amount.0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.engine_contract.id())
            .await?
            .0,
        DEPOSITED_FEE - transfer_amount.0
    );
    assert_eq!(
        contract
            .get_eth_on_near_balance(&contract.eth_connector_contract.id())
            .await?
            .0,
        0
    );
    assert_eq!(
        contract
            .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS))
            .await?,
        0
    );
    assert_eq!(contract.total_supply().await?.0, DEPOSITED_AMOUNT);
    assert_eq!(
        contract.total_eth_supply_on_near().await?.0,
        DEPOSITED_AMOUNT
    );
    Ok(())
}
