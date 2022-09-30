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

    let balance: u128 = contract
        .eth_connector_contract
        .call("ft_total_eth_supply_on_aurora")
        .view()
        .await?
        .json::<String>()?
        .parse()
        .unwrap();
    assert_eq!(balance, 0);

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

    contract
        .assert_eth_on_near_balance(
            &receiver_id,
            DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128,
        )
        .await?;
    contract
        .assert_eth_on_near_balance(
            &contract.engine_contract.id(),
            DEPOSITED_FEE - transfer_amount as u128,
        )
        .await?;
    contract
        .assert_total_eth_supply_on_near(DEPOSITED_AMOUNT)
        .await?;
    contract.assert_total_eth_supply_on_aurora(0).await?;
    contract.assert_total_supply(DEPOSITED_AMOUNT).await?;
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
    println!("{:#?}", res);
    assert!(res.is_success());

    let data: WithdrawResult = res.borsh()?;
    let custodian_addr = validate_eth_address(CUSTODIAN_ADDRESS);
    assert_eq!(data.recipient_id, recipient_addr);
    assert_eq!(data.amount, withdraw_amount);
    assert_eq!(data.eth_custodian_address, custodian_addr);

    contract
        .assert_eth_on_near_balance(
            &contract.engine_contract.id(),
            DEPOSITED_FEE - withdraw_amount.as_u128(),
        )
        .await?;
    contract
        .assert_eth_on_near_balance(&receiver_id, DEPOSITED_AMOUNT - DEPOSITED_FEE)
        .await?;
    contract
        .assert_total_supply(DEPOSITED_AMOUNT - withdraw_amount.as_u128())
        .await?;
    Ok(())
}

#[tokio::test]
async fn test_deposit_eth_to_near_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;
    contract.assert_proof_was_used(PROOF_DATA_NEAR).await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    contract
        .assert_eth_on_near_balance(&contract.engine_contract.id(), DEPOSITED_FEE)
        .await?;
    contract
        .assert_eth_on_near_balance(&receiver_id, DEPOSITED_AMOUNT - DEPOSITED_FEE)
        .await?;
    contract.assert_total_eth_supply_on_aurora(0).await?;
    contract
        .assert_total_eth_supply_on_near(DEPOSITED_AMOUNT)
        .await?;
    contract.assert_total_supply(DEPOSITED_AMOUNT).await?;
    Ok(())
}

// NOTE: We don't test relayer fee
#[tokio::test]
async fn test_deposit_eth_to_aurora_balance_total_supply() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_aurora().await?;
    contract.assert_proof_was_used(PROOF_DATA_ETH).await?;

    // NOTE: Relayer FEE not calculated
    // assert_eq!(balance, DEPOSITED_EVM_AMOUNT - DEPOSITED_EVM_FEE);
    contract
        .assert_eth_balance(
            &validate_eth_address(RECIPIENT_ETH_ADDRESS),
            DEPOSITED_EVM_AMOUNT,
        )
        .await?;
    contract.assert_total_supply(DEPOSITED_EVM_AMOUNT).await?;
    contract
        .assert_total_eth_supply_on_near(DEPOSITED_EVM_AMOUNT)
        .await?;
    contract
        .assert_total_eth_supply_on_aurora(DEPOSITED_EVM_AMOUNT)
        .await?;
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_eth() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let balance = contract.get_eth_on_near_balance(&receiver_id).await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = contract
        .get_eth_on_near_balance(&contract.engine_contract.id())
        .await?;
    assert_eq!(balance.0, DEPOSITED_FEE);

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
            contract.eth_connector_contract.id(),
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
    contract
        .assert_eth_on_near_balance(&receiver_id, DEPOSITED_AMOUNT - DEPOSITED_FEE)
        .await?;
    contract
        .assert_eth_on_near_balance(&contract.eth_connector_contract.id(), transfer_amount.0)
        .await?;
    contract
        .assert_eth_on_near_balance(
            &contract.engine_contract.id(),
            DEPOSITED_FEE - transfer_amount.0,
        )
        .await?;
    contract
        .assert_eth_balance(
            &validate_eth_address(RECIPIENT_ETH_ADDRESS),
            transfer_amount.0,
        )
        .await?;
    contract.assert_total_supply(DEPOSITED_AMOUNT).await?;
    contract
        .assert_total_eth_supply_on_near(DEPOSITED_AMOUNT)
        .await?;
    contract
        .assert_total_eth_supply_on_aurora(transfer_amount.0)
        .await?;
    Ok(())
}

#[tokio::test]
async fn test_ft_transfer_call_without_message() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

    let receiver_id = AccountId::try_from(DEPOSITED_RECIPIENT.to_string()).unwrap();
    let balance = contract.get_eth_on_near_balance(&receiver_id).await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = contract
        .get_eth_on_near_balance(&contract.engine_contract.id())
        .await?;
    assert_eq!(balance.0, DEPOSITED_FEE);
    let balance = contract
        .get_eth_on_near_balance(&contract.eth_connector_contract.id())
        .await?;
    assert_eq!(balance.0, 0);

    let transfer_amount: U128 = 50.into();
    let memo: Option<String> = None;
    let message = "";
    // Send to Aurora contract with wrong message should failed
    let res = contract
        .engine_contract
        .call("ft_transfer_call")
        .args_json((
            contract.eth_connector_contract.id(),
            transfer_amount,
            &memo,
            message,
        ))
        .gas(DEFAULT_GAS)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    println!("{:#?}", res);
    // TODO: FIX IT
    // assert!(res.is_failure());
    contract.assert_error_message(res, "ERR_INVALID_ON_TRANSFER_MESSAGE_FORMAT");

    // Assert balances remain unchanged
    let balance = contract.get_eth_on_near_balance(&receiver_id).await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT - DEPOSITED_FEE);
    let balance = contract
        .get_eth_on_near_balance(&contract.engine_contract.id())
        .await?;
    assert_eq!(balance.0, DEPOSITED_FEE);
    let balance = contract
        .get_eth_on_near_balance(&contract.eth_connector_contract.id())
        .await?;
    assert_eq!(balance.0, 0);

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
    let balance = contract.get_eth_on_near_balance(&receiver_id).await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT - DEPOSITED_FEE);
    let balance = contract.get_eth_on_near_balance(&some_acc).await?;
    assert_eq!(balance.0, 0);
    let balance = contract
        .get_eth_on_near_balance(&contract.engine_contract.id())
        .await?;
    assert_eq!(balance.0, DEPOSITED_FEE);
    let balance = contract
        .get_eth_on_near_balance(&contract.eth_connector_contract.id())
        .await?;
    assert_eq!(balance.0, 0);

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

    let balance = contract.get_eth_on_near_balance(&receiver_id).await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT - DEPOSITED_FEE);
    let balance = contract
        .get_eth_on_near_balance(&dummy_contract.id())
        .await?;
    assert_eq!(balance.0, transfer_amount.0);
    let balance = contract
        .get_eth_on_near_balance(&contract.engine_contract.id())
        .await?;
    assert_eq!(balance.0, DEPOSITED_FEE - transfer_amount.0);
    let balance = contract
        .get_eth_on_near_balance(&contract.eth_connector_contract.id())
        .await?;
    assert_eq!(balance.0, 0);

    let balance = contract
        .get_eth_balance(&validate_eth_address(RECIPIENT_ETH_ADDRESS))
        .await?;
    assert_eq!(balance, 0);

    let balance = contract.total_supply().await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    let balance = contract.total_eth_supply_on_near().await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    let balance = contract.total_eth_supply_on_aurora().await?;
    assert_eq!(balance, 0);

    Ok(())
}
