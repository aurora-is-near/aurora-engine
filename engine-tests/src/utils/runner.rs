use std::sync::Arc;

use aurora_engine_sdk::{env::Env, io::IO};
use engine_standalone_storage::AbstractContractRunner;
use near_crypto::PublicKey;
use near_parameters::{RuntimeConfig, RuntimeConfigStore};
use near_primitives_core::{
    hash::CryptoHash,
    types::{AccountId, Balance, Gas, GasWeight},
};
use near_vm_runner::{
    logic::{
        errors::VMRunnerError,
        mocks::mock_external::{MockAction, MockedValuePtr},
        types::PromiseResult,
        types::ReceiptIndex,
        External, StorageAccessTracker, VMContext, VMLogicError, VMOutcome, ValuePtr,
    },
    Contract, ContractCode,
};

use memoffset::span_of;

pub struct EngineStateVMAccess<I: IO> {
    pub io: I,
    pub action_log: Vec<MockAction>,
}

impl<I: IO> External for EngineStateVMAccess<I>
where
    I::StorageValue: AsRef<[u8]>,
{
    fn storage_set(
        &mut self,
        _access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, VMLogicError> {
        Ok(self
            .io
            .write_storage(key, value)
            .map(|v| v.as_ref().to_vec()))
    }

    fn storage_get<'a>(
        &'a self,
        _access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<Option<Box<dyn ValuePtr + 'a>>, VMLogicError> {
        Ok(self
            .io
            .read_storage(key)
            .map::<Box<dyn ValuePtr>, _>(|value| Box::new(MockedValuePtr::new(value))))
    }

    fn storage_remove(
        &mut self,
        _access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, VMLogicError> {
        Ok(self.io.remove_storage(key).map(|v| v.as_ref().to_vec()))
    }

    fn storage_has_key(
        &mut self,
        _access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<bool, VMLogicError> {
        Ok(self.io.storage_has_key(key))
    }

    fn generate_data_id(&mut self) -> CryptoHash {
        unimplemented!()
    }

    fn get_recorded_storage_size(&self) -> usize {
        0
    }

    fn validator_stake(&self, account_id: &AccountId) -> Result<Option<Balance>, VMLogicError> {
        let _ = account_id;
        unimplemented!()
    }

    fn validator_total_stake(&self) -> Result<Balance, VMLogicError> {
        unimplemented!()
    }

    fn create_action_receipt(
        &mut self,
        receipt_indices: Vec<ReceiptIndex>,
        receiver_id: AccountId,
    ) -> Result<ReceiptIndex, VMLogicError> {
        let index = self
            .action_log
            .len()
            .try_into()
            .expect("pointer size must fit in 64 bit");
        self.action_log.push(MockAction::CreateReceipt {
            receipt_indices,
            receiver_id,
        });
        Ok(index)
    }

    fn create_promise_yield_receipt(
        &mut self,
        receiver_id: AccountId,
    ) -> Result<(ReceiptIndex, CryptoHash), VMLogicError> {
        let index = self
            .action_log
            .len()
            .try_into()
            .expect("pointer size must fit in 64 bit");
        let data_id = self.generate_data_id();
        self.action_log.push(MockAction::YieldCreate {
            data_id,
            receiver_id,
        });
        Ok((index, data_id))
    }

    fn submit_promise_resume_data(
        &mut self,
        data_id: CryptoHash,
        data: Vec<u8>,
    ) -> Result<bool, VMLogicError> {
        self.action_log
            .push(MockAction::YieldResume { data_id, data });
        for action in &self.action_log {
            let MockAction::YieldCreate { data_id: did, .. } = action else {
                continue;
            };
            // FIXME: should also check that receiver_id matches current account_id, but there
            // isn't one tracked by `Self`...
            if data_id == *did {
                // NB: does not actually handle timeouts.
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn append_action_create_account(
        &mut self,
        receipt_index: ReceiptIndex,
    ) -> Result<(), VMLogicError> {
        self.action_log
            .push(MockAction::CreateAccount { receipt_index });
        Ok(())
    }

    fn append_action_deploy_contract(
        &mut self,
        receipt_index: ReceiptIndex,
        code: Vec<u8>,
    ) -> Result<(), VMLogicError> {
        self.action_log.push(MockAction::DeployContract {
            receipt_index,
            code,
        });
        Ok(())
    }

    fn append_action_function_call_weight(
        &mut self,
        receipt_index: ReceiptIndex,
        method_name: Vec<u8>,
        args: Vec<u8>,
        attached_deposit: Balance,
        prepaid_gas: Gas,
        gas_weight: GasWeight,
    ) -> Result<(), VMLogicError> {
        self.action_log.push(MockAction::FunctionCallWeight {
            receipt_index,
            method_name,
            args,
            attached_deposit,
            prepaid_gas,
            gas_weight,
        });
        Ok(())
    }

    fn append_action_transfer(
        &mut self,
        receipt_index: ReceiptIndex,
        deposit: Balance,
    ) -> Result<(), VMLogicError> {
        self.action_log.push(MockAction::Transfer {
            receipt_index,
            deposit,
        });
        Ok(())
    }

    fn append_action_stake(
        &mut self,
        receipt_index: ReceiptIndex,
        stake: Balance,
        public_key: PublicKey,
    ) {
        self.action_log.push(MockAction::Stake {
            receipt_index,
            stake,
            public_key,
        });
    }

    fn append_action_add_key_with_full_access(
        &mut self,
        receipt_index: ReceiptIndex,
        public_key: PublicKey,
        nonce: u64,
    ) {
        self.action_log.push(MockAction::AddKeyWithFullAccess {
            receipt_index,
            public_key,
            nonce,
        });
    }

    fn append_action_add_key_with_function_call(
        &mut self,
        receipt_index: ReceiptIndex,
        public_key: PublicKey,
        nonce: u64,
        allowance: Option<Balance>,
        receiver_id: AccountId,
        method_names: Vec<Vec<u8>>,
    ) -> Result<(), VMLogicError> {
        self.action_log.push(MockAction::AddKeyWithFunctionCall {
            receipt_index,
            public_key,
            nonce,
            allowance,
            receiver_id,
            method_names,
        });
        Ok(())
    }

    fn append_action_delete_key(&mut self, receipt_index: ReceiptIndex, public_key: PublicKey) {
        self.action_log.push(MockAction::DeleteKey {
            receipt_index,
            public_key,
        });
    }

    fn append_action_delete_account(
        &mut self,
        receipt_index: ReceiptIndex,
        beneficiary_id: AccountId,
    ) -> Result<(), VMLogicError> {
        self.action_log.push(MockAction::DeleteAccount {
            receipt_index,
            beneficiary_id,
        });
        Ok(())
    }

    fn get_receipt_receiver(&self, receipt_index: ReceiptIndex) -> &AccountId {
        let index: usize = receipt_index
            .try_into()
            .expect("pointer size is long enough");
        match self.action_log.get(index) {
            Some(MockAction::CreateReceipt { receiver_id, .. }) => receiver_id,
            _ => panic!("not a valid receipt index!"),
        }
    }
}

pub struct ContractRunner {
    contract: CodeWrapper,
    runtime_config: Arc<RuntimeConfig>,
}

pub struct Context {
    balance: u128,
    storage_usage: u64,
}

impl Context {
    #[must_use]
    pub fn serialize(&self) -> [u8; size_of::<Self>()] {
        let mut out = <[u8; size_of::<Self>()]>::default();
        out[span_of!(Self, balance)].clone_from_slice(&self.balance.to_le_bytes());
        out[span_of!(Self, storage_usage)].clone_from_slice(&self.storage_usage.to_le_bytes());
        out
    }

    #[must_use]
    pub fn deserialize(v: [u8; size_of::<Self>()]) -> Self {
        let balance = u128::from_le_bytes(v[span_of!(Self, balance)].try_into().unwrap());
        let storage_usage =
            u64::from_le_bytes(v[span_of!(Self, storage_usage)].try_into().unwrap());
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

const BUNDLED_CONTRACT: &[u8] = include_bytes!("../../../bin/aurora-engine-traced.wasm");

impl ContractRunner {
    pub fn bundled() -> Self {
        Self::new(BUNDLED_CONTRACT.to_vec(), None)
    }

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

impl AbstractContractRunner for ContractRunner {
    type Error = VMRunnerError;

    fn call_contract<E, I>(
        &self,
        method: &str,
        promise_data: Vec<Option<Vec<u8>>>,
        env: &E,
        io: I,
        more_ctx: &mut [u8; 32],
        override_input: Option<Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, Self::Error>
    where
        E: Env,
        I: IO + Send,
        I::StorageValue: AsRef<[u8]>,
    {
        let mut ctx = Context::deserialize(*more_ctx);
        if ctx.storage_usage == 0 {
            ctx.storage_usage = 100;
        }
        let promise_results = promise_data
            .iter()
            .cloned()
            .map(|data| data.map_or(PromiseResult::Failed, PromiseResult::Successful))
            .collect::<Vec<_>>()
            .into();

        let input = override_input.unwrap_or_else(|| io.read_input().as_ref().to_vec());
        let mut ext = EngineStateVMAccess {
            io,
            action_log: vec![],
        };

        let vm_outcome = self.call(method, input, promise_results, env, &mut ext, &mut ctx)?;
        let output = vm_outcome.return_data.as_value();
        if let Some(data) = &output {
            ext.io.return_output(data);
        }
        *more_ctx = ctx.serialize();
        Ok(output)
    }
}
