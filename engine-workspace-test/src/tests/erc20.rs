use aurora_engine_types::U256;

use libsecp256k1::PublicKey;
use near_primitives::account;
use near_primitives::test_utils;
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
    let source_secp_sk =
        libsecp256k1::SecretKey::parse(&pad_to_bytes32(source_account_seed.as_bytes()).unwrap())?;
    let source_near_sk = workspaces::types::SecretKey::from_seed(SECP256K1, source_account_seed);
    
    // Set account tx to submit
    contract
    .batch()
    // Add new full access key for aurora transaction
    .add_key(
        source_near_sk.public_key(),
        workspaces::types::AccessKey::full_access(),
    )
    .transact()
    .await?;

    worker.fast_forward(1).await?;
    
    let contract_sk = contract.as_account().secret_key();
    let source_secp_pk_ser =  PublicKey::from_secret_key(&source_secp_sk).serialize().to_ascii_lowercase();
    let source_secp_pk_str = String::from_utf8_lossy(&source_secp_pk_ser);
    println!("{:?}", contract.id());
    println!("{:?} == {:?}", contract_sk, source_near_sk);
    println!("{:?} == {:?}", contract_sk.public_key(), source_secp_pk_str);

    // Build transaction
    let erc20_deploy = ERC20Constructor::load();

    let transaction = erc20_deploy.deploy("Test Token", "TEST", U256([0, 0, 0, 1u64]));

    // Sign transaction

    let signed_tx = sign_transaction(transaction, Some(MAINNET_CHAIN_ID.into()), &source_secp_sk);

    let encoded_tx = rlp::encode(&signed_tx).to_vec();

    // Encode outcome
    let outcome = contract
        .call("submit")
        .args(encoded_tx)
        .transact()
        .await?;

    println!("submit outcome: {:#?}", outcome);

    println!("Contract Account ID: {}", contract.id());
    
    Ok(())
}
