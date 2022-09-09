use aurora_engine::fungible_token::FungibleTokenMetadata;
use aurora_engine::parameters::InitCallArgs;
use borsh::BorshSerialize;
use workspaces::Worker;
use workspaces::network::Sandbox;
use workspaces::prelude::*;
use workspaces::Contract;
use aurora_engine::parameters::NewCallArgs;
use crate::test_utils::str_to_account_id;
use crate::prelude::U256;

const AURORA_WASM_FILEPATH: &str = "../mainnet-test.wasm";

pub const MAINNET_CHAIN_ID: u32 = 1313161556;


pub async fn deploy_evm_test() -> anyhow::Result<(Worker<Sandbox>, Contract)> {
    let worker = workspaces::sandbox().await?;
    let wasm = std::fs::read(AURORA_WASM_FILEPATH)?;
    let contract = worker.dev_deploy(&wasm).await?;

    // Record Chain metadata
    let args = NewCallArgs {
        chain_id: crate::prelude::u256_to_arr(&U256::from(MAINNET_CHAIN_ID)),
        owner_id: str_to_account_id("test.near"),
        bridge_prover_id: str_to_account_id("bridge_prover.near"),
        upgrade_delay_blocks: 1,
    };

    contract
        .call( "new")
        .args(args.try_to_vec().unwrap())
        .transact()
        .await;

    // Setup new eth connector
    let init_evm = InitCallArgs {
        prover_account: str_to_account_id("prover.near"),
        eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
        metadata: FungibleTokenMetadata::default(),
    };

    contract
        .call( "new_eth_connector")
        .args(init_evm.try_to_vec().unwrap())
        .transact()
        .await;

    return Ok((worker, contract));
}
