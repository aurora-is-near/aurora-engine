use aurora_engine_types::account_id::AccountId;
use near_workspaces::network::{NetworkClient, Sandbox};
use near_workspaces::types::{KeyType, SecretKey};
use near_workspaces::Worker;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::Instant;

use crate::account::Account;

#[derive(Debug, Clone)]
pub struct Node {
    root: near_workspaces::Account,
    worker: Worker<Sandbox>,
}

impl Node {
    pub async fn new(root: &str, root_balance: u128) -> anyhow::Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        let root = Self::create_root_account(&worker, root, root_balance).await?;

        Ok(Self { root, worker })
    }

    pub fn root(&self) -> Account {
        Account::from_inner(self.root.clone())
    }

    pub fn worker(&self) -> &Worker<Sandbox> {
        &self.worker
    }

    pub async fn get_balance(&self, account_id: &AccountId) -> anyhow::Result<u128> {
        let account_id = near_workspaces::AccountId::from_str(account_id.as_ref())?;

        self.worker
            .view_account(&account_id)
            .await
            .map(|d| d.balance)
            .map_err(Into::into)
    }

    async fn create_root_account(
        worker: &Worker<Sandbox>,
        root_acc_name: &str,
        balance: u128,
    ) -> anyhow::Result<near_workspaces::Account> {
        use near_workspaces::AccessKey;

        if root_acc_name == "test.near" {
            return Ok(worker.root_account()?);
        }

        let registrar = if root_acc_name.ends_with("near") {
            worker
                .import_contract(&"near".parse()?, worker)
                .transact()
                .await?
        } else {
            let testnet = near_workspaces::testnet()
                .await
                .map_err(|err| anyhow::anyhow!("Failed init testnet: {:?}", err))?;
            let registrar = "registrar".parse()?;
            worker
                .import_contract(&registrar, &testnet)
                .transact()
                .await?
        };

        Self::waiting_account_creation(worker, registrar.id()).await?;

        let sk = SecretKey::from_seed(KeyType::ED25519, "registrar");
        let root = root_acc_name.parse()?;
        registrar
            .as_account()
            .batch(&root)
            .create_account()
            .add_key(sk.public_key(), AccessKey::full_access())
            .transfer(balance)
            .transact()
            .await?
            .into_result()?;

        Ok(near_workspaces::Account::from_secret_key(root, sk, worker))
    }

    /// Waiting for the account creation
    async fn waiting_account_creation<T: NetworkClient + ?Sized>(
        worker: &Worker<T>,
        account_id: &near_workspaces::AccountId,
    ) -> anyhow::Result<()> {
        let timer = Instant::now();
        // Try to get account within 30 secs
        for _ in 0..60 {
            if worker.view_account(account_id).await.is_err() {
                tokio::time::sleep(Duration::from_millis(500)).await;
            } else {
                return Ok(());
            }
        }

        anyhow::bail!(
            "Account `{}` was not created during {} seconds",
            account_id,
            timer.elapsed().as_secs_f32()
        )
    }
}
