use aurora_engine_types::U256;

use near_primitives::account;
use near_primitives::test_utils;
use workspaces::network::DevAccountDeployer;
use workspaces::network::Sandbox;
use workspaces::types::KeyType::SECP256K1;
use workspaces::types::SecretKey;
use workspaces::Account;
use workspaces::Contract;
use workspaces::Worker;
use crate::test_utils::MAINNET_CHAIN_ID;
use crate::test_utils::deploy_evm_test;
use crate::test_utils::erc20::ERC20Constructor;
use crate::test_utils::sign_transaction;

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
    // Start engine
    let (worker, contract): (Worker<Sandbox>, Contract) = deploy_evm_test().await?;

    // Build account
    let source_account_seed = "source";
    let test_account =
        libsecp256k1::SecretKey::parse(&pad_to_bytes32(source_account_seed.as_bytes()).unwrap())?;
    let test_near_account = workspaces::types::SecretKey::from_seed(SECP256K1, source_account_seed);

    // Set account tx to submit
    println!("{:?}", worker.root_account().id());
    println!("{:?}", contract);
    contract
        .batch(&worker)
        // Delete default root account for assigned in workspace
        //.delete_key(worker.root_account().secret_key().public_key())
        // Add new key for aurora transaction
        .add_key(
            test_near_account.public_key(),
            workspaces::types::AccessKey::full_access(),
        )
        .transact()
        .await?;
    // Build transaction
    let erc20_deploy = ERC20Constructor::load();

    let transaction = erc20_deploy.deploy("Test Token", "TEST", U256([0, 0, 0, 1u64]));

    // Sign transaction

    let signed_tx = sign_transaction(transaction, Some(MAINNET_CHAIN_ID.into()), &test_account);

    let encoded_tx = rlp::encode(&signed_tx).to_vec();

    // Encode outcome
    let outcome = contract
        .call(&worker, "submit")
        .args(encoded_tx)
        .transact()
        .await?;

    println!("submit outcome: {:#?}", outcome);

    println!("Contract Account ID: {}", contract.id());
    Ok(())
}
