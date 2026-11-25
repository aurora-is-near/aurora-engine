use crate::transaction::{CallTransaction, ViewTransaction};
use aurora_engine_types::account_id::AccountId;
use std::str::FromStr;

use crate::contract::RawContract;
use aurora_engine_types::public_key::PublicKey;
use near_workspaces::AccessKey;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::NearToken;

#[derive(Debug, Clone)]
pub struct Account {
    inner: near_workspaces::Account,
}

impl Account {
    pub(crate) const fn from_inner(inner: near_workspaces::Account) -> Self {
        Self { inner }
    }

    pub fn call<F: AsRef<str>>(&self, contract_id: &AccountId, function: F) -> CallTransaction {
        let contract_id = near_workspaces::AccountId::from_str(contract_id.as_ref()).unwrap();
        let transaction = self.inner.call(&contract_id, function.as_ref());

        CallTransaction::new(transaction)
    }

    pub fn view<F: AsRef<str>>(&self, contract_id: &AccountId, function: F) -> ViewTransaction<'_> {
        let contract_id = near_workspaces::AccountId::from_str(contract_id.as_ref()).unwrap();
        let transaction = self.inner.view(&contract_id, function.as_ref());

        ViewTransaction::new(transaction)
    }

    pub async fn deploy(&self, wasm: &[u8]) -> anyhow::Result<RawContract> {
        let contract = self.inner.deploy(wasm).await?.into_result()?;
        Ok(RawContract::from_contract(contract))
    }

    #[must_use]
    pub fn id(&self) -> AccountId {
        self.inner.id().as_str().parse().unwrap()
    }

    pub async fn create_subaccount(&self, name: &str, balance: NearToken) -> anyhow::Result<Self> {
        self.inner
            .create_subaccount(name)
            .initial_balance(balance)
            .transact()
            .await?
            .into_result()
            .map(|inner| Self { inner })
            .map_err(Into::into)
    }

    /// Adds an access key to this account that may only call `method_names` of `receiver_id`.
    pub async fn add_function_call_key(
        &self,
        public_key: near_workspaces::types::PublicKey,
        receiver_id: &AccountId,
        method_names: &[&str],
    ) -> anyhow::Result<ExecutionFinalResult> {
        let receiver_id = near_workspaces::AccountId::from_str(receiver_id.as_ref())?;
        let access_key = AccessKey::function_call_access(&receiver_id, method_names, None);

        self.inner
            .batch(self.inner.id())
            .add_key(public_key, access_key)
            .transact()
            .await
            .map_err(Into::into)
    }

    pub fn public_key(&self) -> anyhow::Result<PublicKey> {
        let pk = self.inner.secret_key().public_key();
        PublicKey::from_str(serde_json::to_string(&pk)?.trim_matches('"'))
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }
}
