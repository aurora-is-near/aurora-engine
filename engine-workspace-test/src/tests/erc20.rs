use aurora_engine_types::U256;
use byte_slice_cast::AsByteSlice;
use serde_json::json;

use workspaces::network::Sandbox;
use workspaces::Contract;
use workspaces::Worker;
use workspaces::types::SecretKey;

use crate::test_utils::engine::start_engine;
use crate::test_utils::erc20::ERC20Constructor;
use crate::test_utils::sign_transaction;
use crate::test_utils::str_to_account_id;

fn pad_to_bytes32(s: &[u8]) -> Option<[u8; 32]> {
    let s_len = s.len();

    if s_len > 32 {
        return None;
    }

    let mut result: [u8; 32] = Default::default();

    result[..s_len].clone_from_slice(s);

    Some(result)
}

#[tokio::test]
async fn erc20_deploy() -> anyhow::Result<()> {
    let (worker, contract): (Worker<Sandbox>, Contract) = start_engine().await?;

    let erc20_deploy = ERC20Constructor::load();

    let transaction = erc20_deploy.deploy("Test Token", "TEST", U256([0, 0, 0, 1u64]));

    //check fn submit_transaction_profiled
    let calling_account_id = str_to_account_id("test.near");
    let test_account = libsecp256k1::SecretKey::parse(&pad_to_bytes32(calling_account_id_bytes).unwrap())?;
    let owner = worker.root_account().secret_key();
    let signed_tx = sign_transaction(transaction, Some(MAINNET_CHAIN_ID), owner);

    let encoded_tx = rlp::encode(&signed_tx).to_vec();

    let outcome = contract
        .call(&worker, "submit")
        .args(encoded_tx)
        .transact()
        .await?;

    println!("submit outcome: {:#?}", outcome);

    println!("Dev Account ID: {}", contract.id());
    Ok(())
}
