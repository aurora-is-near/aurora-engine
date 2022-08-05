use aurora_engine_types::U256;

use near_primitives::test_utils;
use workspaces::network::DevAccountDeployer;
use workspaces::network::Sandbox;
use workspaces::types::KeyType::SECP256K1;
use workspaces::types::SecretKey;
use workspaces::Contract;
use workspaces::Worker;

use crate::test_utils::engine::start_engine;
use crate::test_utils::engine::MAINNET_CHAIN_ID;
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
    let (worker, contract): (Worker<Sandbox>, Contract) = start_engine().await?;

    // Build account
    let source_account_seed = "source";
    let test_account =
        libsecp256k1::SecretKey::parse(&pad_to_bytes32(source_account_seed.as_bytes()).unwrap())?;
    let test_near_account = workspaces::types::SecretKey::from_seed(SECP256K1, source_account_seed);
    let account = worker.dev_create_account().await?;
    let source_acount = account
        .create_subaccount(&worker, source_account_seed)
        .keys(test_near_account)
        .transact()
        .await?
        .unwrap();
    let dest_account_seed = "dest";
    let dest_account =
        libsecp256k1::SecretKey::parse(&pad_to_bytes32(dest_account_seed.as_bytes()).unwrap())?;
    let dest_near_account = workspaces::types::SecretKey::from_seed(SECP256K1, dest_account_seed);
    let dest_acount = account
        .create_subaccount(&worker, dest_account_seed)
        .keys(dest_near_account)
        .transact()
        .await?
        .unwrap();

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

    println!("Dev Account ID: {}", contract.id());
    Ok(())
}

fn initialize_erc20() -> (test_utils::AuroraRunner, Signer, Address, ERC20) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));

    let mut signer = Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;
    let nonce = signer.use_nonce();
    let constructor = ERC20Constructor::load();
    let contract = ERC20(runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy("TestToken", "TEST", nonce.into()),
        constructor,
    ));

    (runner, signer, dest_address, contract)
}
