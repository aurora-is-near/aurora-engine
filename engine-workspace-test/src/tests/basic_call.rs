use serde_json::json;

use workspaces::Worker;
use workspaces::network::Sandbox;
use workspaces::Contract;

use crate::test_utils::engine::start_engine;

#[tokio::test]
async fn get_version() -> anyhow::Result<()> {
    //let worker = workspaces::sandbox().await?;
    //let wasm = std::fs::read(AURORA_WASM_FILEPATH)?;
    let (worker, contract): (Worker<Sandbox>, Contract) = start_engine().await?;

    let outcome = contract
        .call(&worker, "get_version")
        .args_json(json!({}))?
        .transact()
        .await?;

    println!("get_version outcome: {:#?}", outcome);

    println!("Dev Account ID: {}", contract.id());
    Ok(())
}