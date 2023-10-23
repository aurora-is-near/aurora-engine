use crate::utils;
use crate::utils::solidity::erc20::{ERC20Constructor, ERC20};
/// Simulation tests for exit to NEAR precompile.
/// Note: `AuroraRunner` is not suitable for these tests because
/// it does not execute promises; but `aurora-workspaces` does.
use crate::utils::AuroraRunner;
use aurora_engine_types::account_id::AccountId;
#[cfg(feature = "ext-connector")]
use aurora_engine_types::parameters::connector::{FungibleTokenMetadata, WithdrawSerializeType};
use aurora_engine_types::types::Address;
use aurora_engine_types::U256;
use aurora_engine_workspace::account::Account;
use aurora_engine_workspace::{parse_near, EngineContract, RawContract};
use serde_json::json;

const FT_PATH: &str = "src/tests/res/fungible_token.wasm";
const STORAGE_AMOUNT: u128 = 50_000_000_000_000_000_000_000_000;
#[cfg(feature = "ext-connector")]
const AURORA_ETH_CONNECTOR: &str = "aurora_eth_connector";

/// Deploy Aurora smart contract WITHOUT init external eth-connector.
#[allow(clippy::future_not_send)]
pub async fn deploy_engine_with_code(code: Vec<u8>) -> EngineContract {
    let chain_id = AuroraRunner::get_default_chain_id();
    aurora_engine_workspace::EngineContractBuilder::new()
        .unwrap()
        .with_chain_id(chain_id)
        .with_code(code)
        .with_custodian_address("d045f7e19B2488924B97F9c145b5E51D0D895A65")
        .unwrap()
        .with_root_balance(parse_near!("10000 N"))
        .with_contract_balance(parse_near!("1000 N"))
        .deploy_and_init()
        .await
        .unwrap()
}

#[allow(clippy::let_and_return, clippy::future_not_send)]
pub async fn deploy_engine() -> EngineContract {
    let code = AuroraRunner::get_engine_code();
    let contract = deploy_engine_with_code(code).await;

    #[cfg(feature = "ext-connector")]
    init_eth_connector(&contract).await.unwrap();

    contract
}

/// Deploy and init external eth connector
#[cfg(feature = "ext-connector")]
async fn init_eth_connector(aurora: &EngineContract) -> anyhow::Result<()> {
    let contract_bytes = get_aurora_eth_connector_contract();
    let contract_account = aurora
        .root()
        .create_subaccount(AURORA_ETH_CONNECTOR, 15 * STORAGE_AMOUNT)
        .await
        .unwrap();
    let contract = contract_account.deploy(&contract_bytes).await.unwrap();
    let metadata = FungibleTokenMetadata::default();
    let init_args = json!({
        "prover_account": contract_account.id(),
        "eth_custodian_address": "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E",
        "metadata": metadata,
        "account_with_access_right": aurora.id(),
        "owner_id": aurora.id()
    });

    let result = contract
        .call("new")
        .args_json(init_args)
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

pub async fn create_sub_account(
    master_account: &Account,
    account: &str,
    balance: u128,
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
        .deposit(1)
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
        .deposit(1)
        .transact()
        .await?;
    assert!(result.is_success());

    Ok(())
}

#[cfg(feature = "ext-connector")]
fn get_aurora_eth_connector_contract() -> Vec<u8> {
    use std::path::Path;
    let contract_path = Path::new("../engine-tests-connector/etc/aurora-eth-connector");
    std::fs::read(contract_path.join("bin/aurora-eth-connector-test.wasm")).unwrap()
}
