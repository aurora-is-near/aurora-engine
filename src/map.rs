use crate::prelude::Vec;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::sdk;
use crate::storage::{bytes_to_key, KeyPrefix};

/// An non-iterable implementation of a map that stores its content directly on the trie.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct LookupMap {
    key_prefix: KeyPrefix,
}

impl Default for LookupMap {
    fn default() -> Self {
        Self::new(KeyPrefix::RelayerEvmAddressMap)
    }
}

impl LookupMap {
    /// Create a new map. Use `key_prefix` as a unique prefix for keys.
    pub fn new(key_prefix: KeyPrefix) -> Self {
        Self { key_prefix }
    }

    /// Build key for this map scope
    fn raw_key_to_storage_key(&self, key_raw: &[u8]) -> Vec<u8> {
        bytes_to_key(self.key_prefix, key_raw)
    }

    /// Returns `true` if the serialized key is present in the map.
    #[allow(dead_code)]
    pub fn contains_key_raw(&self, key_raw: &[u8]) -> bool {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        sdk::storage_has_key(&storage_key)
    }

    /// Returns the serialized value corresponding to the serialized key.
    #[allow(dead_code)]
    pub fn get_raw(&self, key_raw: &[u8]) -> Option<Vec<u8>> {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        sdk::read_storage(&storage_key)
    }

    /// Inserts a serialized key-value pair into the map.
    pub fn insert_raw(&mut self, key_raw: &[u8], value_raw: &[u8]) {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        sdk::write_storage(&storage_key, value_raw);
    }

    /// Removes a serialized key from the map, returning the serialized value at the key if the key
    /// was previously in the map.
    #[allow(dead_code)]
    pub fn remove_raw(&mut self, key_raw: &[u8]) {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        sdk::remove_storage(&storage_key);
    }
}
