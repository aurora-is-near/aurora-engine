use near_primitives::account::Account;
use serde_json::json;

use workspaces::prelude::*;
use workspaces::Contract;
use workspaces::result::CallExecutionDetails;

const AURORA_WASM_FILEPATH: &str = "../../mainnet-release.wasm";




#[tokio::test]
async fn get_version() -> anyhow::Result<()> {
    //let worker = workspaces::sandbox().await?;
    //let wasm = std::fs::read(AURORA_WASM_FILEPATH)?;
    let (worker, contract) = start_engine().await?;

    let outcome = contract
        .call(&worker, "get_version")
        .args_json(json!({}))?
        .transact()
        .await?;

    println!("get_version outcome: {:#?}", outcome);

    println!("Dev Account ID: {}", contract.id());
    Ok(())
}