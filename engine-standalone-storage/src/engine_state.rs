use std::sync::{Mutex, OnceLock};

use rocksdb::DB;

use aurora_engine_sdk::io::{StorageIntermediate, IO};
use near_crypto::PublicKey;
use near_primitives_core::{
    hash::CryptoHash,
    types::{AccountId, Balance, Gas, GasWeight},
};
use near_vm_runner::logic::{
    mocks::mock_external::{MockAction, MockedValuePtr},
    types::ReceiptIndex,
    External, StorageAccessTracker, VMLogicError, ValuePtr,
};

use crate::diff::{Diff, DiffValue};
use crate::StoragePrefix;

#[derive(Debug)]
pub enum EngineStorageValue<'a> {
    Slice(&'a [u8]),
    Vec(Vec<u8>),
}

impl AsRef<[u8]> for EngineStorageValue<'_> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Slice(slice) => slice,
            Self::Vec(bytes) => bytes,
        }
    }
}

impl StorageIntermediate for EngineStorageValue<'_> {
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref());
    }
}

#[derive(Copy, Clone)]
pub struct EngineStateAccess<'db, 'input, 'output> {
    input: &'input [u8],
    bound_block_height: u64,
    bound_tx_position: u16,
    transaction_diff: &'output Mutex<Diff>,
    output: &'output OnceLock<Vec<u8>>,
    db: &'db DB,
}

impl<'db, 'input, 'output> EngineStateAccess<'db, 'input, 'output> {
    pub const fn new(
        input: &'input [u8],
        bound_block_height: u64,
        bound_tx_position: u16,
        transaction_diff: &'output Mutex<Diff>,
        output: &'output OnceLock<Vec<u8>>,
        db: &'db DB,
    ) -> Self {
        Self {
            input,
            bound_block_height,
            bound_tx_position,
            transaction_diff,
            output,
            db,
        }
    }

    #[must_use]
    pub fn get_transaction_diff(&self) -> Diff {
        self.transaction_diff.lock().unwrap().clone()
    }

    fn construct_engine_read(&self, key: &[u8]) -> rocksdb::ReadOptions {
        let upper_bound =
            super::construct_engine_key(key, self.bound_block_height, self.bound_tx_position);
        let lower_bound = super::construct_storage_key(StoragePrefix::Engine, key);
        let mut opt = rocksdb::ReadOptions::default();
        opt.set_iterate_upper_bound(upper_bound);
        opt.set_iterate_lower_bound(lower_bound);
        opt
    }
}

impl<'db, 'input: 'db, 'output: 'db> IO for EngineStateAccess<'db, 'input, 'output> {
    type StorageValue = EngineStorageValue<'db>;

    fn read_input(&self) -> Self::StorageValue {
        EngineStorageValue::Slice(self.input)
    }

    fn return_output(&mut self, value: &[u8]) {
        self.output.set(value.to_vec()).unwrap_or_default();
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        if let Some(diff) = self.transaction_diff.lock().unwrap().get(key) {
            return diff
                .value()
                .map(|bytes| EngineStorageValue::Vec(bytes.to_vec()));
        }

        let opt = self.construct_engine_read(key);
        let mut iter = self.db.iterator_opt(rocksdb::IteratorMode::End, opt);
        let value = iter.next().and_then(|maybe_elem| {
            maybe_elem
                .ok()
                .map(|(_, value)| DiffValue::try_from_bytes(&value).expect("diff value is invalid"))
        })?;
        value.take_value().map(EngineStorageValue::Vec)
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        self.read_storage(key).is_some()
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        let original_value = self.read_storage(key);

        self.transaction_diff
            .lock()
            .unwrap()
            .modify(key.to_vec(), value.to_vec());

        original_value
    }

    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        self.write_storage(key, value.as_ref())
    }

    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue> {
        let original_value = self.read_storage(key);

        self.transaction_diff.lock().unwrap().delete(key.to_vec());

        original_value
    }
}

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
        Ok(self.io.write_storage(key, value).map(|v| v.to_vec()))
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
