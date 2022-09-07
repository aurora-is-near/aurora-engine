use serde_json::json;
use workspaces::Worker;
use workspaces::network::Sandbox;
use workspaces::Contract;

use crate::test_utils::{deploy_evm, deploy_evm_test};

#[tokio::test]
async fn get_version() -> anyhow::Result<()> {
    let (worker, contract): (Worker<Sandbox>, Contract) = deploy_evm().await?;

    let outcome = contract
        .call( "get_version")
        .args_json(json!({}))
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

    println!("Dev Account ID: {:?}", owner);
    Ok(())
}