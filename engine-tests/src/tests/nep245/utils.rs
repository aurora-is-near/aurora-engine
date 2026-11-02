//! Helpers for the NEP-245 tests.
use std::path::Path;
use std::sync::LazyLock;

use aurora_engine_sdk::types::near_account_to_evm_address;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh;
use aurora_engine_types::parameters::connector::{BridgedToken, Nep245Token};
use aurora_engine_types::parameters::engine::{DeployErc20TokenArgs, TransactionStatus};
use aurora_engine_types::types::PromiseResult;
use aurora_engine_workspace::account::Account;
use aurora_engine_workspace::types::{ExecutionFinalResult, NearToken};
use aurora_engine_workspace::{EngineContract, RawContract};
use near_sdk::json_types::U128;
use serde_json::json;

use crate::prelude::{Address, U256};
use crate::utils;
use crate::utils::AuroraRunner;
use crate::utils::solidity::erc20::{ERC20, ERC20Constructor};
use crate::utils::workspace::{create_sub_account, deploy_engine};

/// Defuse token IDs look like `nep141:wrap.near`, so the tests use the same shape.
pub const TOKEN_ID: &str = "nep141:token.near";
/// Amount of [`TOKEN_ID`] minted to the user by [`setup`].
pub const TOTAL_SUPPLY: u128 = 1_000_000;

const MT_TOKEN_ACCOUNT: &str = "mt_token";
const USER_ACCOUNT: &str = "user";
const ACCOUNT_BALANCE: NearToken = NearToken::from_near(50);

static MT_TOKEN_WASM: LazyLock<Vec<u8>> = LazyLock::new(|| compile_fixture("mt-token"));

fn compile_fixture(name: &str) -> Vec<u8> {
    let artifact_path = utils::rust::compile(Path::new("../etc/tests").join(name));
    std::fs::read(artifact_path).unwrap()
}

/// The engine with a NEP-245 token bridged to an ERC-20 contract.
pub struct Nep245Context {
    pub aurora: EngineContract,
    pub mt_token: RawContract,
    /// Owns [`TOTAL_SUPPLY`] of [`TOKEN_ID`] on NEAR.
    pub user: Account,
    pub erc20: ERC20,
}

impl Nep245Context {
    /// The NEP-245 token bridged by [`setup`].
    pub fn token(&self) -> Nep245Token {
        Nep245Token {
            account_id: self.mt_token.id(),
            token_id: TOKEN_ID.into(),
        }
    }

    /// EVM address of the user when it calls the engine from NEAR.
    pub fn user_address(&self) -> Address {
        near_account_to_evm_address(&self.user.id())
    }

    /// Deposits `amount` of [`TOKEN_ID`] from the user to `recipient` on Aurora.
    pub async fn deposit(&self, amount: u128, recipient: Address) -> anyhow::Result<()> {
        let result = mt_transfer_call(
            &self.user,
            &self.mt_token,
            &self.aurora.id(),
            TOKEN_ID,
            amount,
            recipient.encode().as_str(),
        )
        .await?;
        anyhow::ensure!(result.is_success(), "deposit failed: {result:?}");
        let used: Vec<U128> = result.json()?;
        anyhow::ensure!(used == [U128(amount)], "unexpected used amounts: {used:?}");
        Ok(())
    }

    pub async fn mt_balance_of(&self, account_id: &AccountId) -> u128 {
        mt_balance_of(&self.mt_token, account_id, TOKEN_ID).await
    }

    pub async fn erc20_balance_of(&self, address: Address) -> U256 {
        let tx = self.erc20.balance_of(address, U256::zero());
        evm_view_u256(&self.aurora, self.erc20.0.address, tx.data).await
    }

    /// Checks that the ERC-20 supply equals the NEP-245 balance locked on the engine account.
    pub async fn assert_erc20_backed(&self) {
        self.assert_locked_surplus(0).await;
    }

    /// Checks that the engine account locks `surplus` NEP-245 tokens beyond the ERC-20 supply.
    pub async fn assert_locked_surplus(&self, surplus: u128) {
        let locked = self.mt_balance_of(&self.aurora.id()).await;
        let tx = self.erc20.total_supply(U256::zero());
        let supply = evm_view_u256(&self.aurora, self.erc20.0.address, tx.data).await;
        assert_eq!(
            U256::from(locked),
            supply + surplus,
            "NEP-245 tokens locked on the engine must exceed the ERC-20 supply by {surplus}"
        );
    }
}

/// Deploys the engine and the mock NEP-245 contract, mints [`TOTAL_SUPPLY`] of [`TOKEN_ID`]
/// to a user, and deploys the ERC-20 contract for the token.
pub async fn setup() -> Nep245Context {
    let aurora = deploy_engine().await;
    let mt_token = deploy_mt_token(&aurora).await.unwrap();
    let user = create_sub_account(&aurora.root(), USER_ACCOUNT, ACCOUNT_BALANCE)
        .await
        .unwrap();
    let metadata = json!({ "name": "Token", "symbol": "TKN", "decimals": "6" });
    mint(
        &mt_token,
        &user.id(),
        TOKEN_ID,
        TOTAL_SUPPLY,
        Some(metadata),
    )
    .await
    .unwrap();
    let token = Nep245Token {
        account_id: mt_token.id(),
        token_id: TOKEN_ID.into(),
    };
    let erc20 = deploy_erc20_from_nep245(token, &aurora).await.unwrap();

    Nep245Context {
        aurora,
        mt_token,
        user,
        erc20,
    }
}

pub async fn deploy_mt_token(aurora: &EngineContract) -> anyhow::Result<RawContract> {
    let account = create_sub_account(&aurora.root(), MT_TOKEN_ACCOUNT, ACCOUNT_BALANCE).await?;
    account.deploy(&MT_TOKEN_WASM).await
}

/// Mints NEP-245 tokens and replaces the token metadata if `metadata` is given.
pub async fn mint(
    mt_token: &RawContract,
    account_id: &AccountId,
    token_id: &str,
    amount: u128,
    metadata: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let result = mt_token
        .call("mint")
        .args_json(json!({
            "account_id": account_id,
            "token_id": token_id,
            "amount": U128(amount),
            "metadata": metadata,
        }))
        .transact()
        .await?;
    anyhow::ensure!(result.is_success(), "mint failed: {result:?}");
    Ok(())
}

/// Makes the mock NEP-245 contract reject transfers to `account_id`.
pub async fn set_rejected_receiver(
    mt_token: &RawContract,
    account_id: &AccountId,
    rejected: bool,
) -> anyhow::Result<()> {
    let result = mt_token
        .call("set_rejected_receiver")
        .args_json(json!({ "account_id": account_id, "rejected": rejected }))
        .transact()
        .await?;
    anyhow::ensure!(
        result.is_success(),
        "set_rejected_receiver failed: {result:?}"
    );
    Ok(())
}

pub async fn mt_balance_of(mt_token: &RawContract, account_id: &AccountId, token_id: &str) -> u128 {
    mt_token
        .view("mt_balance_of")
        .args_json(json!({ "account_id": account_id, "token_id": token_id }))
        .await
        .unwrap()
        .json::<U128>()
        .unwrap()
        .0
}

pub async fn mt_transfer_call(
    sender: &Account,
    mt_token: &RawContract,
    receiver_id: &AccountId,
    token_id: &str,
    amount: u128,
    msg: &str,
) -> anyhow::Result<ExecutionFinalResult> {
    sender
        .call(&mt_token.id(), "mt_transfer_call")
        .args_json(json!({
            "receiver_id": receiver_id,
            "token_id": token_id,
            "amount": U128(amount),
            "msg": msg,
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await
}

pub async fn mt_batch_transfer_call(
    sender: &Account,
    mt_token: &RawContract,
    receiver_id: &AccountId,
    tokens: &[(&str, u128)],
    msg: &str,
) -> anyhow::Result<ExecutionFinalResult> {
    let (token_ids, amounts): (Vec<_>, Vec<_>) = tokens
        .iter()
        .map(|&(token_id, amount)| (token_id, U128(amount)))
        .unzip();
    sender
        .call(&mt_token.id(), "mt_batch_transfer_call")
        .args_json(json!({
            "receiver_id": receiver_id,
            "token_ids": token_ids,
            "amounts": amounts,
            "msg": msg,
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await
}

/// Checks that a receipt executed by `executor_id` failed with an error containing `error`.
pub fn assert_receipt_failed(result: &ExecutionFinalResult, executor_id: &AccountId, error: &str) {
    let failures: Vec<_> = result
        .receipt_failures()
        .into_iter()
        .map(|outcome| {
            // `Display` of the workspace error shows only its kind, so read the
            // inner `TxExecutionError` carrying the panic message.
            let message = outcome
                .clone()
                .into_result()
                .unwrap_err()
                .into_inner()
                .map_or_else(|error| error.to_string(), |error| error.to_string());
            (outcome.executor_id.to_string(), message)
        })
        .collect();
    assert!(
        failures
            .iter()
            .any(|(executor, message)| executor == executor_id.as_ref() && message.contains(error)),
        "expected a receipt on {executor_id} to fail with {error}, got {failures:?}"
    );
}

pub async fn deploy_erc20_from_nep245(
    token: Nep245Token,
    aurora: &EngineContract,
) -> anyhow::Result<ERC20> {
    let result = aurora
        .deploy_erc20_token(DeployErc20TokenArgs::Nep245(token))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(result.is_success(), "deploy_erc20_token failed");
    let abi = ERC20Constructor::load().0.abi;
    Ok(ERC20(utils::solidity::DeployedContract {
        abi,
        address: result.into_value(),
    }))
}

async fn evm_view_u256(aurora: &EngineContract, address: Address, input: Vec<u8>) -> U256 {
    let status = aurora
        .view(Address::zero(), address, U256::zero(), input)
        .await
        .unwrap()
        .result;
    match status {
        TransactionStatus::Succeed(bytes) => U256::from_big_endian(&bytes),
        status => panic!("Unexpected status {status:?}"),
    }
}

/// Deploys the ERC-20 contract for `token` on the runner, replaying the metadata promise
/// as a token without metadata.
pub fn deploy_erc20_wasm(runner: &mut AuroraRunner, token: Nep245Token) -> Address {
    let owner = runner.aurora_account_id.clone();
    let args = borsh::to_vec(&DeployErc20TokenArgs::Nep245(token.clone())).unwrap();
    runner.call("deploy_erc20_token", &owner, args).unwrap();

    runner.promise_results = vec![PromiseResult::Successful(b"[null]".to_vec())];
    let args = borsh::to_vec(&BridgedToken::Nep245(token)).unwrap();
    let outcome = runner
        .call("deploy_erc20_token_callback", &owner, args)
        .unwrap();
    runner.promise_results.clear();

    let bytes: Vec<u8> = borsh::from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    Address::from_array(bytes.try_into().unwrap())
}

/// JSON arguments of `mt_on_transfer` sending `amount` of every token in `token_ids`.
pub fn mt_on_transfer_args(
    sender_id: &str,
    token_ids: &[&str],
    amount: u128,
    msg: &str,
) -> Vec<u8> {
    json!({
        "sender_id": sender_id,
        "previous_owner_ids": vec![sender_id; token_ids.len()],
        "token_ids": token_ids,
        "amounts": vec![amount.to_string(); token_ids.len()],
        "msg": msg,
    })
    .to_string()
    .into_bytes()
}

/// Input of the bridged ERC-20 `withdrawToNear(bytes,uint256)` function.
pub fn withdraw_to_near_input(recipient: &str, amount: u128) -> Vec<u8> {
    let args = ethabi::encode(&[
        ethabi::Token::Bytes(recipient.as_bytes().to_vec()),
        ethabi::Token::Uint(amount.into()),
    ]);
    [
        aurora_engine::xcc::WITHDRAW_TO_NEAR_SELECTOR.as_slice(),
        &args,
    ]
    .concat()
}
