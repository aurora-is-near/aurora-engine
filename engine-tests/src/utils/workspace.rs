use std::sync::LazyLock;

use aurora_engine_types::U256;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::{FungibleTokenMetadata, WithdrawSerializeType};
use aurora_engine_types::parameters::engine::DeployErc20TokenArgs;
use aurora_engine_types::types::Address;
use aurora_engine_workspace::account::Account;
use aurora_engine_workspace::{EngineContract, RawContract, types::NearToken};
use serde_json::json;
use tokio::sync::OnceCell;

use crate::utils;
/// Simulation tests for exit to NEAR precompile.
/// Note: `AuroraRunner` is not suitable for these tests because
/// it does not execute promises; but `aurora-workspaces` does.
use crate::utils::AuroraRunner;
use crate::utils::solidity::erc20::{ERC20, ERC20Constructor};

const CONNECTOR_URL: &str = "https://github.com/Near-One/aurora-eth-connector/releases/download/eth-connector-v0.6.5/eth-connector-v0-6-5.zip";
const WASM_FILE_NAME: &str = "aurora_eth_connector.wasm";
const FT_PATH: &str = "src/tests/res/fungible_token.wasm";
const FT_RECEIVER_PATH: &str = "../etc/tests/ft-receiver";
const STORAGE_AMOUNT: NearToken = NearToken::from_near(50);
const AURORA_ETH_CONNECTOR: &str = "aurora_eth_connector";
static ETH_CONNECTOR_WASM: OnceCell<Vec<u8>> = OnceCell::const_new();
static FT_RECEIVER_WASM: LazyLock<Vec<u8>> =
    LazyLock::new(|| std::fs::read(utils::rust::compile(FT_RECEIVER_PATH)).unwrap());

/// Deploy Aurora smart contract with external eth-connector.
pub async fn deploy_engine_with_code(code: Vec<u8>) -> EngineContract {
    let chain_id = AuroraRunner::get_default_chain_id();
    let contract = aurora_engine_workspace::EngineContractBuilder::new()
        .unwrap()
        .with_chain_id(chain_id)
        .with_code(code)
        .with_root_balance(NearToken::from_near(5000))
        .with_contract_balance(NearToken::from_near(1000))
        .deploy_and_init()
        .await
        .unwrap();
    init_eth_connector(&contract).await.unwrap();

    contract
}

pub async fn deploy_engine() -> EngineContract {
    deploy_engine_with_code(AuroraRunner::get_engine_code()).await
}

pub async fn deploy_engine_v331() -> EngineContract {
    deploy_engine_with_code(AuroraRunner::get_engine_v331_code()).await
}

/// Deploy and init external eth connector
async fn init_eth_connector(aurora: &EngineContract) -> anyhow::Result<()> {
    let contract_account = aurora
        .root()
        .create_subaccount(
            AURORA_ETH_CONNECTOR,
            STORAGE_AMOUNT.checked_mul(15).unwrap(),
        )
        .await?;
    let contract = contract_account
        .deploy(
            ETH_CONNECTOR_WASM
                .get_or_try_init(|| download_and_extract_wasm(CONNECTOR_URL, WASM_FILE_NAME))
                .await?,
        )
        .await?;
    let metadata = FungibleTokenMetadata::default();
    let init_args = json!({
        "metadata": metadata,
        "aurora_engine_account_id": aurora.id(),
        "owner_id": contract_account.id(),
        "controller": aurora.id()
    });

    let result = contract
        .call("new")
        .args_json(init_args)
        .max_gas()
        .transact()
        .await?;
    assert!(result.is_success());

    // By default, the contract is paused. So we need to unpause it.
    let result = contract
        .call("pa_unpause_feature")
        .args_json(json!({ "key": "ALL" }))
        .max_gas()
        .transact()
        .await?;
    assert!(result.is_success());

    let result = aurora
        .set_eth_connector_contract_account(contract_account.id(), WithdrawSerializeType::Borsh)
        .transact()
        .await?;
    assert!(result.is_success());

    Ok(())
}

pub async fn get_xcc_router_version(aurora: &EngineContract, xcc_account: &AccountId) -> u32 {
    aurora
        .root()
        .view(xcc_account, "get_version")
        .await
        .unwrap()
        .json::<u32>()
        .unwrap()
}

/// Deploys `etc/tests/ft-receiver` to the `receiver` sub-account of the root account.
/// Its `ft_on_transfer` and `mt_on_transfer` read the transfer message to decide how many
/// tokens to return: `return:N`, `panic`, or `spend:account:N` to forward tokens before
/// requesting a full refund. Other messages keep all tokens.
pub async fn deploy_ft_receiver(aurora: &EngineContract) -> anyhow::Result<RawContract> {
    let account = create_sub_account(&aurora.root(), "receiver", NearToken::from_near(10)).await?;
    account.deploy(&FT_RECEIVER_WASM).await
}

pub async fn create_sub_account(
    master_account: &Account,
    account: &str,
    balance: NearToken,
) -> anyhow::Result<Account> {
    master_account.create_subaccount(account, balance).await
}

pub async fn deploy_erc20_from_nep_141(
    nep_141_account: &str,
    aurora: &EngineContract,
) -> anyhow::Result<ERC20> {
    let nep141 = nep_141_account.parse().unwrap();
    let result = aurora
        .deploy_erc20_token(DeployErc20TokenArgs::WithMetadata(nep141))
        .max_gas()
        .transact()
        .await?;
    assert!(result.is_success());
    let address = result.into_value();
    let abi = ERC20Constructor::load().0.abi;
    Ok(ERC20(utils::solidity::DeployedContract { abi, address }))
}

pub async fn deploy_erc20_from_nep_141_legacy(
    nep_141_account: &str,
    aurora: &EngineContract,
) -> anyhow::Result<ERC20> {
    let nep141 = nep_141_account.parse().unwrap();
    let result = aurora
        .deploy_erc20_token_legacy(nep141)
        .max_gas()
        .transact()
        .await?;
    assert!(result.is_success());
    let address = result.into_value();
    let abi = ERC20Constructor::load().0.abi;
    Ok(ERC20(utils::solidity::DeployedContract { abi, address }))
}

pub async fn transfer_nep_141_to_erc_20(
    nep_141: &RawContract,
    erc20: &ERC20,
    source: &Account,
    dest: Address,
    amount: u128,
    aurora: &EngineContract,
) -> anyhow::Result<()> {
    let result = source
        .call(&nep_141.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": aurora.id(),
            "amount": amount.to_string(),
            "memo": "null",
        }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?;
    assert!(result.is_success(), "{result:?}");
    let mint_tx = erc20.mint(dest, amount.into(), 0.into());
    let result = aurora
        .call(erc20.0.address, U256::zero(), mint_tx.data)
        .transact()
        .await?;
    assert!(result.is_success());

    Ok(())
}

pub async fn nep_141_balance_of(nep_141: &RawContract, account_id: &AccountId) -> u128 {
    nep_141
        .call("ft_balance_of") // XCC requires gas
        .args_json(json!({ "account_id": account_id }))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .json::<near_sdk::json_types::U128>()
        .map(|s| s.0)
        .unwrap()
}

/// Deploys the standard FT implementation:
/// `https://github.com/near/near-sdk-rs/blob/master/examples/fungible-token/ft/src/lib.rs`
pub async fn deploy_nep_141(
    nep_141_account: &Account,
    token_owner: &Account,
    amount: u128,
    aurora: &EngineContract,
) -> anyhow::Result<RawContract> {
    let contract_bytes = std::fs::read(FT_PATH)?;
    let nep141 = nep_141_account.deploy(&contract_bytes).await?;
    let result = aurora
        .root()
        .call(&nep141.id(), "new_default_meta")
        .args_json(json!({
            "owner_id": token_owner.id(),
            "total_supply": format!("{amount}"),
        }))
        .transact()
        .await?;
    assert!(result.is_success(), "{result:?}");

    // Need to register Aurora contract so that it can receive tokens
    let result = aurora
        .root()
        .call(&nep141.id(), "storage_deposit")
        .args_json(json!({
            "account_id": aurora.id(),
        }))
        .deposit(STORAGE_AMOUNT)
        .transact()
        .await?;
    assert!(result.is_success());

    Ok(nep141)
}

pub async fn transfer_nep_141(
    nep_141: &AccountId,
    source: &Account,
    dest: &str,
    amount: u128,
) -> anyhow::Result<()> {
    let result = source
        .call(nep_141, "storage_deposit")
        .args_json(json!({
            "account_id": dest,
        }))
        .deposit(STORAGE_AMOUNT)
        .transact()
        .await?;
    assert!(result.is_success());

    let result = source
        .call(nep_141, "ft_transfer")
        .args_json(json!({
            "receiver_id": dest,
            "amount": amount.to_string(),
            "memo": "null",
        }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?;
    assert!(result.is_success());

    Ok(())
}

/// Downloads a contract from `url` into the `target/downloaded-wasm` cache and checks it
/// against the hex-encoded SHA-256 `checksum`. A cached file with that checksum is reused.
pub async fn download_wasm(url: &str, file_name: &str, checksum: &str) -> anyhow::Result<Vec<u8>> {
    let cache_dir = std::env::current_dir()?
        .join("target")
        .join("downloaded-wasm");
    let wasm_path = cache_dir.join(file_name);
    let sha256 = |bytes: &[u8]| hex::encode(aurora_engine_sdk::sha256(bytes).as_bytes());

    if let Ok(bytes) = tokio::fs::read(&wasm_path).await
        && sha256(&bytes) == checksum
    {
        return Ok(bytes);
    }

    tokio::fs::create_dir_all(&cache_dir).await?;

    let bytes = reqwest::get(url)
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec();

    anyhow::ensure!(sha256(&bytes) == checksum, "checksum mismatch for {url}");
    tokio::fs::write(&wasm_path, &bytes).await?;

    Ok(bytes)
}

/// Downloads a zip archive from `url`, extracts it into a cache directory under
/// `target/`, read the contract bytes, and returns the contract bytes.
/// Re downloads are skipped if the target file already exists.
async fn download_and_extract_wasm(url: &str, wasm_name: &str) -> anyhow::Result<Vec<u8>> {
    let cache_dir = std::env::current_dir()?
        .join("target")
        .join("downloaded-wasm");
    let wasm_path = cache_dir.join(wasm_name);

    if !wasm_path.exists() {
        std::fs::create_dir_all(&cache_dir)?;

        let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        archive.extract(&cache_dir)?;

        anyhow::ensure!(
            wasm_path.exists(),
            "`{wasm_name}` not found in the archive at {url}"
        );
    }

    tokio::fs::read(wasm_path)
        .await
        .map_err(|e| anyhow::anyhow!("failed to read the wasm file: {e}"))
}
