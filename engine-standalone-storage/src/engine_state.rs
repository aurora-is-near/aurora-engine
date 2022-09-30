use aurora_engine_sdk::io::{StorageIntermediate, IO};
use rocksdb::DB;
use std::cell::{Cell, RefCell};

use crate::diff::{Diff, DiffValue};
use crate::StoragePrefix;

#[derive(Debug)]
pub enum EngineStorageValue<'a> {
    Slice(&'a [u8]),
    Vec(Vec<u8>),
}

impl<'a> AsRef<[u8]> for EngineStorageValue<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Slice(slice) => slice,
            Self::Vec(bytes) => bytes,
        }
    }
}

impl<'a> StorageIntermediate for EngineStorageValue<'a> {
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }
}

#[derive(Copy, Clone)]
pub struct EngineStateAccess<'db, 'input, 'output> {
    input: &'input [u8],
    bound_block_height: u64,
    bound_tx_position: u16,
    transaction_diff: &'output RefCell<Diff>,
    output: &'output Cell<Vec<u8>>,
    db: &'db DB,
}

impl<'db, 'input, 'output> EngineStateAccess<'db, 'input, 'output> {
    pub fn new(
        input: &'input [u8],
        bound_block_height: u64,
        bound_tx_position: u16,
        transaction_diff: &'output RefCell<Diff>,
        output: &'output Cell<Vec<u8>>,
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

    pub fn get_transaction_diff(&self) -> Diff {
        self.transaction_diff.borrow().clone()
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
        self.output.set(value.to_vec())
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        if let Some(diff) = self.transaction_diff.borrow().get(key) {
            return diff
                .value()
                .map(|bytes| EngineStorageValue::Vec(bytes.to_vec()));
        }

        let opt = self.construct_engine_read(key);
        let mut iter = self.db.iterator_opt(rocksdb::IteratorMode::End, opt);
        let value = iter.next().and_then(|maybe_elem| {
            maybe_elem
                .ok()
                .map(|(_, value)| DiffValue::try_from_bytes(&value).unwrap())
        })?;
        value.take_value().map(EngineStorageValue::Vec)
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        self.read_storage(key).is_some()
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        let original_value = self.read_storage(key);

        self.transaction_diff
            .borrow_mut()
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

        self.transaction_diff.borrow_mut().delete(key.to_vec());

        original_value
    }
}
