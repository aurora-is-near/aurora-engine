use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::FungibleTokenMetadata;
use aurora_engine_types::U256;
use near_workspaces::types::NearToken;

use crate::node::Node;

pub use crate::contract::{EngineContract, RawContract};

pub mod account;
pub mod contract;
pub mod macros;
pub mod node;
pub mod operation;
pub mod result;
pub mod transaction;

pub mod types {
    pub use near_workspaces::result::{ExecutionFinalResult, ExecutionOutcome};
    pub use near_workspaces::types::{KeyType, NearToken, SecretKey};
}

const AURORA_LOCAL_CHAIN_ID: u64 = 1313161556;
const OWNER_ACCOUNT_ID: &str = "aurora.root";
const ROOT_BALANCE: NearToken = NearToken::from_near(400);
const CONTRACT_BALANCE: NearToken = NearToken::from_near(200);

#[derive(Debug)]
pub struct EngineContractBuilder {
    code: Option<Vec<u8>>,
    chain_id: [u8; 32],
    owner_id: AccountId,
    upgrade_delay_blocks: u64,
    root_balance: NearToken,
    contract_balance: NearToken,
    ft_metadata: FungibleTokenMetadata,
}

impl EngineContractBuilder {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            code: None,
            chain_id: into_chain_id(AURORA_LOCAL_CHAIN_ID),
            owner_id: OWNER_ACCOUNT_ID.parse().unwrap(),
            upgrade_delay_blocks: 1,
            root_balance: ROOT_BALANCE,
            contract_balance: CONTRACT_BALANCE,
            ft_metadata: FungibleTokenMetadata::default(),
        })
    }

    #[must_use]
    pub fn with_code(mut self, code: Vec<u8>) -> Self {
        self.code = Some(code);
        self
    }

    #[must_use]
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = into_chain_id(chain_id);
        self
    }

    pub fn with_owner_id(mut self, owner_id: &str) -> anyhow::Result<Self> {
        self.owner_id = owner_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Parse account_id error: {e}"))?;
        Ok(self)
    }

    #[must_use]
    pub const fn with_upgrade_delay_blocks(mut self, upgrade_delay_blocks: u64) -> Self {
        self.upgrade_delay_blocks = upgrade_delay_blocks;
        self
    }

    #[must_use]
    pub fn with_ft_metadata(mut self, ft_metadata: FungibleTokenMetadata) -> Self {
        self.ft_metadata = ft_metadata;
        self
    }

    #[must_use]
    pub const fn with_root_balance(mut self, balance: NearToken) -> Self {
        self.root_balance = balance;
        self
    }

    #[must_use]
    pub const fn with_contract_balance(mut self, balance: NearToken) -> Self {
        self.contract_balance = balance;
        self
    }

    pub async fn deploy_and_init(self) -> anyhow::Result<EngineContract> {
        let owner_id = self.owner_id.as_ref();
        let (owner, root) = owner_id.split_once('.').unwrap_or((owner_id, owner_id));
        let node = Node::new(root, self.root_balance).await?;
        let account = if owner == root {
            node.root()
        } else {
            node.root()
                .create_subaccount(owner, self.contract_balance)
                .await?
        };
        let public_key = account.public_key()?;
        let contract = account
            .deploy(
                &self
                    .code
                    .ok_or_else(|| anyhow::anyhow!("WASM wasn't set"))?,
            )
            .await?;
        let engine = EngineContract {
            account,
            contract,
            public_key,
            node,
        };

        engine
            .new(self.chain_id, self.owner_id, self.upgrade_delay_blocks)
            .transact()
            .await
            .map_err(|e| anyhow::anyhow!("Error while initialize aurora contract: {e}"))?;

        Ok(engine)
    }
}

fn into_chain_id(value: u64) -> [u8; 32] {
    let chain_id = U256::from(value);
    chain_id.to_big_endian()
}

#[tokio::test]
async fn test_creating_aurora_contract() {
    let code = get_engine_code().unwrap();
    let contract = EngineContractBuilder::new()
        .unwrap()
        .with_owner_id("aurora.test.near")
        .unwrap()
        .with_code(code)
        .deploy_and_init()
        .await
        .unwrap();

    let chain_id = contract.get_chain_id().await.unwrap().result;
    assert_eq!(
        chain_id,
        U256::from_big_endian(&into_chain_id(AURORA_LOCAL_CHAIN_ID))
    );
}

#[cfg(test)]
fn get_engine_code() -> anyhow::Result<Vec<u8>> {
    std::fs::read("../bin/aurora-engine-test.wasm").map_err(Into::into)
}
