use workspaces::Worker;
use workspaces::network::Sandbox;
use workspaces::prelude::*;
use workspaces::Contract;

const AURORA_WASM_FILEPATH: &str = "../mainnet-release.wasm";


pub async fn start_engine() -> anyhow::Result<(Worker<Sandbox>, Contract)> {
    let worker = workspaces::sandbox().await?;
    let wasm = std::fs::read(AURORA_WASM_FILEPATH)?;
    let contract = worker.dev_deploy(&wasm).await?;
    return Ok((worker.clone(), contract));
}
