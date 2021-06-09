use crate::prelude::Vec;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::sdk;
use crate::storage::{bytes_to_key, KeyPrefixU8};

/// An non-iterable implementation of a map that stores its content directly on the trie.
/// Use `key_prefix` as a unique prefix for keys.
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct LookupMap<const K: KeyPrefixU8> {}

impl<const K: KeyPrefixU8> LookupMap<K> {
    /// Create a new map.
    pub fn new() -> Self {
        Self {}
    }

    /// Build key for this map scope
    fn raw_key_to_storage_key(&self, key_raw: &[u8]) -> Vec<u8> {
        bytes_to_key(K.into(), key_raw)
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
    pub fn remove_raw(&mut self, key_raw: &[u8]) -> Option<Vec<u8>> {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        sdk::remove_storage_with_result(&storage_key)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct BijectionMap<const LR: KeyPrefixU8, const RL: KeyPrefixU8> {}

impl<const LR: KeyPrefixU8, const RL: KeyPrefixU8> BijectionMap<LR, RL> {
    fn left_to_right() -> LookupMap<LR> {
        Default::default()
    }

    fn right_to_left() -> LookupMap<RL> {
        Default::default()
    }

    pub fn insert(&self, value_left: &[u8], value_right: &[u8]) {
        Self::left_to_right().insert_raw(value_left, value_right);
        Self::right_to_left().insert_raw(value_right, value_left);
    }

    pub fn lookup_left(&self, value_left: &[u8]) -> Option<Vec<u8>> {
        Self::left_to_right().get_raw(value_left)
    }

    #[allow(dead_code)]
    pub fn lookup_right(&self, value_right: &[u8]) -> Option<Vec<u8>> {
        Self::right_to_left().get_raw(value_right)
    }

    #[allow(dead_code)]
    pub fn remove_left(&self, value_left: &[u8]) {
        if let Some(value_right) = Self::left_to_right().remove_raw(value_left) {
            Self::right_to_left().remove_raw(value_right.as_slice());
        }
    }

    #[allow(dead_code)]
    pub fn remove_right(&self, value_right: &[u8]) {
        if let Some(value_left) = Self::right_to_left().remove_raw(value_right) {
            Self::left_to_right().remove_raw(value_left.as_slice());
        }
    }
}
