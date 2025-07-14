use std::sync::Arc;

use aurora_engine_sdk::env::Env;
use near_parameters::{RuntimeConfig, RuntimeConfigStore};
use near_primitives_core::{hash::CryptoHash, types::AccountId};
use near_vm_runner::{
    logic::{errors::VMRunnerError, types::PromiseResult, External, VMContext, VMOutcome},
    Contract, ContractCode,
};

pub struct ContractRunner {
    contract: CodeWrapper,
    runtime_config: Arc<RuntimeConfig>,
}

pub struct Context {
    balance: u128,
    storage_usage: u64,
}

impl Context {
    pub const fn initial() -> Self {
        Self {
            balance: 0,
            storage_usage: 0,
        }
    }

    pub fn serialize(&self) -> [u8; size_of::<Self>()] {
        let mut out = <[u8; size_of::<Self>()]>::default();
        out[memoffset::offset_of!(Self, balance)..][..size_of::<u128>()]
            .clone_from_slice(&self.balance.to_le_bytes());
        out[memoffset::offset_of!(Self, storage_usage)..][..size_of::<u64>()]
            .clone_from_slice(&self.storage_usage.to_le_bytes());
        out
    }

    pub fn deserialize(v: [u8; size_of::<Self>()]) -> Self {
        let balance = u128::from_le_bytes(
            v[memoffset::offset_of!(Self, balance)..][..size_of::<u128>()]
                .try_into()
                .unwrap(),
        );
        let storage_usage = u64::from_le_bytes(
            v[memoffset::offset_of!(Self, storage_usage)..][..size_of::<u64>()]
                .try_into()
                .unwrap(),
        );
        Self {
            balance,
            storage_usage,
        }
    }
}

struct CodeWrapper(Arc<ContractCode>);
impl Contract for CodeWrapper {
    fn get_code(&self) -> Option<Arc<near_vm_runner::ContractCode>> {
        Some(self.0.clone())
    }

    fn hash(&self) -> near_primitives_core::hash::CryptoHash {
        *self.0.hash()
    }
}

impl ContractRunner {
    pub fn new(code: Vec<u8>, hash: Option<CryptoHash>) -> Self {
        let runtime_config_store =
            RuntimeConfigStore::for_chain_id(near_primitives_core::chains::TESTNET);
        let runtime_config =
            runtime_config_store.get_config(near_primitives_core::version::PROTOCOL_VERSION);
        Self {
            contract: CodeWrapper(Arc::new(ContractCode::new(code, hash))),
            runtime_config: runtime_config.clone(),
        }
    }

    pub fn call(
        &self,
        method: &str,
        input: Vec<u8>,
        promise_results: Arc<[PromiseResult]>,
        env: &impl Env,
        ext: &mut (impl External + Send),
        more_ctx: &mut Context,
    ) -> Result<VMOutcome, VMRunnerError> {
        let current_account_id = env
            .current_account_id()
            .to_string()
            .parse::<AccountId>()
            .expect("incompatible account id");
        let signer_account_id = env
            .signer_account_id()
            .to_string()
            .parse::<AccountId>()
            .expect("incompatible account id");
        let predecessor_account_id = env
            .predecessor_account_id()
            .to_string()
            .parse::<AccountId>()
            .expect("incompatible account id");
        let ctx = VMContext {
            current_account_id,
            signer_account_id,
            signer_account_pk: vec![],
            predecessor_account_id,
            input,
            promise_results,
            block_height: env.block_height(),
            block_timestamp: env.block_timestamp().nanos(),
            epoch_height: 0,
            account_balance: more_ctx.balance,
            account_locked_balance: 0,
            storage_usage: more_ctx.storage_usage,
            attached_deposit: env.attached_deposit(),
            prepaid_gas: env.prepaid_gas().as_u64(),
            random_seed: env.random_seed().0.to_vec(),
            output_data_receivers: vec![],
            view_config: None,
        };

        let contract = near_vm_runner::prepare(
            &self.contract,
            self.runtime_config.wasm_config.clone(),
            None,
            ctx.make_gas_counter(&self.runtime_config.wasm_config),
            method,
        );

        near_vm_runner::run(contract, ext, &ctx, self.runtime_config.fees.clone()).inspect(
            |outcome| {
                more_ctx.storage_usage = outcome.storage_usage;
                more_ctx.balance = outcome.balance;
            },
        )
    }
}
