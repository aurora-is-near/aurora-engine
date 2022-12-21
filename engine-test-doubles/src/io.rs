use aurora_engine_sdk::io::{StorageIntermediate, IO};
use std::collections::HashMap;
use std::sync::RwLock;

pub struct Value(Vec<u8>);

impl StorageIntermediate for Value {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.0)
    }
}

#[derive(Debug, Default)]
pub struct Storage {
    pub input: Vec<u8>,
    pub output: Vec<u8>,
    pub kv_store: HashMap<Vec<u8>, Vec<u8>>,
}

/// In-memory implementation of [IO].
#[derive(Debug, Clone, Copy)]
pub struct StoragePointer<'a>(pub &'a RwLock<Storage>);

impl<'a> IO for StoragePointer<'a> {
    type StorageValue = Value;

    fn read_input(&self) -> Self::StorageValue {
        Value(self.0.read().unwrap().input.clone())
    }

    fn return_output(&mut self, value: &[u8]) {
        let mut storage = self.0.write().unwrap();
        storage.output = value.to_vec();
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        self.0
            .read()
            .unwrap()
            .kv_store
            .get(key)
            .map(|v| Value(v.clone()))
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        self.0.read().unwrap().kv_store.contains_key(key)
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        let key = key.to_vec();
        let value = value.to_vec();
        let mut storage = self.0.write().unwrap();
        storage.kv_store.insert(key, value).map(Value)
    }

    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        let key = key.to_vec();
        let mut storage = self.0.write().unwrap();
        storage.kv_store.insert(key, value.0).map(Value)
    }

    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue> {
        let mut storage = self.0.write().unwrap();
        storage.kv_store.remove(key).map(Value)
    }
}
