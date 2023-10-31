use crate::transaction::{CallTransaction, ViewTransaction};
use aurora_engine_types::account_id::AccountId;
use std::str::FromStr;

use crate::contract::RawContract;
use aurora_engine_types::public_key::PublicKey;
pub use near_units::parse_near;
use near_workspaces::types::NearToken;

#[derive(Debug, Clone)]
pub struct Account {
    inner: near_workspaces::Account,
}

impl Account {
    pub(crate) fn from_inner(inner: near_workspaces::Account) -> Self {
        Self { inner }
    }

    pub fn call<F: AsRef<str>>(&self, contract_id: &AccountId, function: F) -> CallTransaction {
        let contract_id = near_workspaces::AccountId::from_str(contract_id.as_ref()).unwrap();
        let transaction = self.inner.call(&contract_id, function.as_ref());

        CallTransaction::new(transaction)
    }

    pub fn view<F: AsRef<str>>(&self, contract_id: &AccountId, function: F) -> ViewTransaction {
        let contract_id = near_workspaces::AccountId::from_str(contract_id.as_ref()).unwrap();
        let transaction = self.inner.view(&contract_id, function.as_ref());

        ViewTransaction::new(transaction)
    }

    pub async fn deploy(&self, wasm: &[u8]) -> anyhow::Result<RawContract> {
        let contract = self.inner.deploy(wasm).await?.into_result()?;
        Ok(RawContract::from_contract(contract))
    }

    pub fn id(&self) -> AccountId {
        self.inner.id().as_str().parse().unwrap()
    }

    pub async fn create_subaccount(&self, name: &str, balance: u128) -> anyhow::Result<Account> {
        self.inner
            .create_subaccount(name)
            .initial_balance(NearToken::from_yoctonear(balance))
            .transact()
            .await?
            .into_result()
            .map(|inner| Account { inner })
            .map_err(Into::into)
    }

    pub fn public_key(&self) -> anyhow::Result<PublicKey> {
        let pk = self.inner.secret_key().public_key();
        PublicKey::from_str(serde_json::to_string(&pk)?.trim_matches('"'))
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }
}
