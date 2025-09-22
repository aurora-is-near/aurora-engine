use std::{fs, sync::Arc};

use aurora_engine_sdk::{env::Env, io::IO};
use engine_standalone_storage::AbstractContractRunner;
use near_crypto::PublicKey;
use near_parameters::{RuntimeConfigStore, RuntimeFeesConfig};
use near_primitives_core::{
    hash::CryptoHash,
    types::{AccountId, Balance, Gas, GasWeight},
};
use near_vm_runner::{
    logic::{
        errors::VMRunnerError,
        mocks::mock_external::{MockAction, MockedValuePtr},
        types::{PromiseResult, ReceiptIndex},
        External, StorageAccessTracker, VMContext, VMLogicError, VMOutcome, ValuePtr,
    },
    Contract, ContractCode, MockContractRuntimeCache,
};

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

    fn append_action_deploy_global_contract(
        &mut self,
        _receipt_index: ReceiptIndex,
        _code: Vec<u8>,
        _mode: near_vm_runner::logic::types::GlobalContractDeployMode,
    ) -> Result<(), VMLogicError> {
        Ok(())
    }

    fn append_action_use_global_contract(
        &mut self,
        _receipt_index: ReceiptIndex,
        _contract_id: near_vm_runner::logic::types::GlobalContractIdentifier,
    ) -> Result<(), VMLogicError> {
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
    cache: MockContractRuntimeCache,
    wasm_config: Arc<near_parameters::vm::Config>,
    fees_config: Arc<RuntimeFeesConfig>,
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
    pub fn bundled() -> Self {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../bin/aurora-engine-compat.wasm"
        );
        // use runtime read to silence `cargo check` error in case the wasm file is not ready yet
        let code = fs::read(path).unwrap();
        Self::new(code, None)
    }

    pub fn new(code: Vec<u8>, hash: Option<CryptoHash>) -> Self {
        let runtime_config_store = RuntimeConfigStore::test();
        let runtime_config =
            runtime_config_store.get_config(near_primitives_core::version::PROTOCOL_VERSION);
        let fees_config = runtime_config.fees.clone();
        let mut wasm_config = runtime_config.wasm_config.clone();
        drop(runtime_config_store);
        // needed for `tests::sanity::test_solidity_pure_bench`
        Arc::get_mut(&mut wasm_config)
            .unwrap()
            .limit_config
            .max_gas_burnt = u64::MAX;

        Self {
            contract: CodeWrapper(Arc::new(ContractCode::new(code, hash))),
            cache: MockContractRuntimeCache::default(),
            wasm_config,
            fees_config,
        }
    }

    pub fn wasm_config_mut(&mut self) -> &mut near_parameters::vm::Config {
        Arc::get_mut(&mut self.wasm_config).unwrap()
    }

    pub fn call(
        &self,
        method: &str,
        input: Vec<u8>,
        promise_results: Arc<[PromiseResult]>,
        env: &impl Env,
        ext: &mut (impl External + Send),
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
        let storage_usage =
            100 + u64::try_from(self.contract.0.code().len()).expect("usize must fit in 64");
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
            account_balance: 10u128.pow(25),
            account_locked_balance: 0,
            storage_usage,
            attached_deposit: env.attached_deposit(),
            prepaid_gas: env.prepaid_gas().as_u64(),
            random_seed: env.random_seed().0.to_vec(),
            output_data_receivers: vec![],
            view_config: None,
        };

        let contract = near_vm_runner::prepare(
            &self.contract,
            self.wasm_config.clone(),
            Some(&self.cache),
            ctx.make_gas_counter(&self.wasm_config),
            method,
        );

        near_vm_runner::run(contract, ext, &ctx, self.fees_config.clone())
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
    ) -> Result<Option<Vec<u8>>, Self::Error>
    where
        E: Env,
        I: IO + Send,
        I::StorageValue: AsRef<[u8]>,
    {
        let promise_results = promise_data
            .iter()
            .cloned()
            .map(|data| data.map_or(PromiseResult::Failed, PromiseResult::Successful))
            .collect::<Vec<_>>()
            .into();

        let input = io.read_input().as_ref().to_vec();
        let mut ext = EngineStateVMAccess {
            io,
            action_log: vec![],
        };

        let vm_outcome = self.call(method, input, promise_results, env, &mut ext)?;
        let output = vm_outcome.return_data.as_value();
        if let Some(data) = &output {
            ext.io.return_output(data);
        }
        Ok(output)
    }
}
