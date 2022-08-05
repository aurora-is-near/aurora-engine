use serde_json::json;
use workspaces::Worker;
use workspaces::network::Sandbox;
use workspaces::Contract;

use crate::test_utils::deploy_evm;

#[tokio::test]
async fn get_version() -> anyhow::Result<()> {
    let (worker, contract): (Worker<Sandbox>, Contract) = deploy_evm().await?;

    let outcome = contract
        .call(&worker, "get_version")
        .args_json(json!({}))?
        .transact()
        .await?;

    println!("get_version outcome: {:#?}", outcome);

    
    println!("Dev Acc ID: {}", contract.id());
    Ok(())
}


#[tokio::test]
async fn test() -> anyhow::Result<()> {
    let (worker, contract): (Worker<Sandbox>, Contract) = deploy_evm().await?;
    
    println!("get_version outcome: {:#?}", contract);

    let a = worker.root_account();

    let calling_account_id = "a";

    let owner = worker.root_account();
    let test_account = owner
        .create_subaccount(&worker, calling_account_id)
        .transact()
        .await?
        .unwrap();

    println!("Dev Account ID: {:?}", test_account);
    Ok(())
}