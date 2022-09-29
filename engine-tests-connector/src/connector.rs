use crate::utils::*;
use aurora_engine::parameters::WithdrawResult;
use aurora_engine_types::types::NEP141Wei;
use near_sdk::json_types::U128;
use near_sdk::ONE_YOCTO;
use workspaces::AccountId;

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
