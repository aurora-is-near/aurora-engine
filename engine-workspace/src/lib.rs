use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::FungibleTokenMetadata;
use aurora_engine_types::types::address::Address;
use aurora_engine_types::U256;

use crate::node::Node;

pub use crate::contract::{EngineContract, RawContract};
pub use near_units::parse_near;

pub mod account;
pub mod contract;
pub mod macros;
pub mod node;
pub mod operation;
pub mod result;
pub mod transaction;

pub mod types {
    pub use workspaces::result::{ExecutionFinalResult, ExecutionOutcome};
    pub use workspaces::types::{KeyType, SecretKey};
}

const AURORA_LOCAL_CHAIN_ID: u64 = 1313161556;
const OWNER_ACCOUNT_ID: &str = "aurora.root";
const PROVER_ACCOUNT_ID: &str = "prover.root";
const ROOT_BALANCE: u128 = parse_near!("400 N");
const CONTRACT_BALANCE: u128 = parse_near!("200 N");

#[derive(Debug)]
pub struct EngineContractBuilder {
    code: Option<Vec<u8>>,
    chain_id: [u8; 32],
    owner_id: AccountId,
    prover_id: AccountId,
    custodian_address: Address,
    upgrade_delay_blocks: u64,
    root_balance: u128,
    contract_balance: u128,
    ft_metadata: FungibleTokenMetadata,
}

impl EngineContractBuilder {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            code: None,
            chain_id: into_chain_id(AURORA_LOCAL_CHAIN_ID),
            owner_id: OWNER_ACCOUNT_ID.parse().unwrap(),
            prover_id: PROVER_ACCOUNT_ID.parse().unwrap(),
            custodian_address: Address::zero(),
            upgrade_delay_blocks: 1,
            root_balance: ROOT_BALANCE,
            contract_balance: CONTRACT_BALANCE,
            ft_metadata: FungibleTokenMetadata::default(),
        })
    }

    pub fn with_code(mut self, code: Vec<u8>) -> Self {
        self.code = Some(code);
        self
    }

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

    pub fn with_prover_id(mut self, prover_id: &str) -> anyhow::Result<Self> {
        self.prover_id = prover_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Parse account_id error: {e}"))?;
        Ok(self)
    }

    pub fn with_custodian_address(mut self, address: &str) -> anyhow::Result<Self> {
        self.custodian_address = Address::decode(address).map_err(|e| anyhow::anyhow!({ e }))?;
        Ok(self)
    }

    pub fn with_upgrade_delay_blocks(mut self, upgrade_delay_blocks: u64) -> Self {
        self.upgrade_delay_blocks = upgrade_delay_blocks;
        self
    }

    pub fn with_ft_metadata(mut self, ft_metadata: FungibleTokenMetadata) -> Self {
        self.ft_metadata = ft_metadata;
        self
    }

    pub fn with_root_balance(mut self, balance: u128) -> Self {
        self.root_balance = balance;
        self
    }

    pub fn with_contract_balance(mut self, balance: u128) -> Self {
        self.contract_balance = balance;
        self
    }

    pub async fn deploy_and_init(self) -> anyhow::Result<EngineContract> {
        let owner_id = self.owner_id.as_ref();
        let (owner, root) = owner_id.split_once('.').unwrap_or((owner_id, owner_id));
        let node = Node::new(root, self.root_balance).await?;
        let owner_acc = if owner != root {
            node.root()
                .create_subaccount(owner, self.contract_balance)
                .await?
        } else {
            node.root()
        };
        let contract = owner_acc
            .deploy(&self.code.expect("WASM wasn't set"))
            .await?;
        let engine: EngineContract = (contract, node).into();

        engine
            .new(self.chain_id, self.owner_id, self.upgrade_delay_blocks)
            .transact()
            .await
            .map_err(|e| anyhow::anyhow!("Error while initialize aurora contract: {e}"))?;

        engine
            .new_eth_connector(
                self.prover_id,
                self.custodian_address.encode(),
                self.ft_metadata,
            )
            .transact()
            .await
            .map_err(|e| anyhow::anyhow!("Error while initialize eth-connector: {e}"))?;

        Ok(engine)
    }
}

fn into_chain_id(value: u64) -> [u8; 32] {
    let chain_id = U256::from(value);
    let mut result = [0; 32];
    chain_id.to_big_endian(&mut result);

    result
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
    assert_eq!(chain_id, U256::from(into_chain_id(AURORA_LOCAL_CHAIN_ID)));
}

#[cfg(test)]
fn get_engine_code() -> anyhow::Result<Vec<u8>> {
    let path = if cfg!(feature = "mainnet-test") {
        if cfg!(feature = "ext-connector") {
            "../bin/aurora-mainnet-silo-test.wasm"
        } else {
            "../bin/aurora-mainnet-test.wasm"
        }
    } else if cfg!(feature = "testnet-test") {
        if cfg!(feature = "ext-connector") {
            "../bin/aurora-testnet-silo-test.wasm"
        } else {
            "../bin/aurora-testnet-test.wasm"
        }
    } else {
        anyhow::bail!("Requires mainnet-test or testnet-test feature provided.")
    };

    std::fs::read(path).map_err(Into::into)
}
