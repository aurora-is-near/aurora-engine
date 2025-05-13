use crate::utils;
use crate::utils::solidity::erc20::{ERC20Constructor, ERC20};
/// Simulation tests for exit to NEAR precompile.
/// Note: `AuroraRunner` is not suitable for these tests because
/// it does not execute promises; but `aurora-workspaces` does.
use crate::utils::AuroraRunner;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::{FungibleTokenMetadata, WithdrawSerializeType};
use aurora_engine_types::types::Address;
use aurora_engine_types::U256;
use aurora_engine_workspace::account::Account;
use aurora_engine_workspace::{types::NearToken, EngineContract, RawContract};
use serde_json::json;

const FT_PATH: &str = "src/tests/res/fungible_token.wasm";
const STORAGE_AMOUNT: NearToken = NearToken::from_near(50);
const AURORA_ETH_CONNECTOR: &str = "aurora_eth_connector";

/// Deploy Aurora smart contract with external eth-connector.
pub async fn deploy_engine_with_code(code: Vec<u8>) -> EngineContract {
    let chain_id = AuroraRunner::get_default_chain_id();
    let contract = aurora_engine_workspace::EngineContractBuilder::new()
        .unwrap()
        .with_chain_id(chain_id)
        .with_code(code)
        .with_root_balance(NearToken::from_near(10000))
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
    let contract = contract_account.deploy(&ETH_CONNECTOR_WASM).await?;
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
    let nep141_account_id = nep_141_account.parse().unwrap();
    let result = aurora
        .deploy_erc20_token(nep141_account_id)
        .max_gas()
        .transact()
        .await
        .unwrap();
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

static ETH_CONNECTOR_WASM: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    let manifest_path = std::env::current_dir()
        .unwrap()
        .join("../engine-tests-connector/etc/aurora-eth-connector")
        .join("eth-connector")
        .join("Cargo.toml");
    let artifact = cargo_near_build::build(cargo_near_build::BuildOpts {
        manifest_path: Some(manifest_path.try_into().unwrap()),
        no_abi: true,
        no_locked: true,
        features: Some("integration-test,migration".to_owned()),
        ..Default::default()
    })
    .unwrap();

    std::fs::read(artifact.path.into_std_path_buf())
        .map_err(|e| anyhow::anyhow!("failed to read the wasm file: {e}"))
        .unwrap()
});
