use crate::utils::*;
use near_sdk::ONE_YOCTO;
use workspaces::AccountId;

#[tokio::test]
async fn test_aurora_ft_transfer() -> anyhow::Result<()> {
    let contract = TestContract::new().await?;
    contract.call_deposit_eth_to_near().await?;

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

    let balance = contract.get_eth_on_near_balance(&receiver_id).await?;
    assert_eq!(
        balance.0,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = contract
        .get_eth_on_near_balance(&contract.eth_connector_contract.id())
        .await?;
    assert_eq!(balance.0, DEPOSITED_FEE - transfer_amount as u128);

    let balance = contract.total_supply().await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    let balance = contract.total_eth_supply_on_aurora().await?;
    assert_eq!(balance, 0);

    let balance = contract.total_eth_supply_on_near().await?;
    assert_eq!(balance.0, DEPOSITED_AMOUNT);

    Ok(())
}
