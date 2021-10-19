pub use crate::prelude::{bytes_to_key, BorshDeserialize, BorshSerialize, KeyPrefixU8, Vec};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::near_runtime::Runtime;

/// An non-iterable implementation of a map that stores its content directly on the trie.
/// Use `key_prefix` as a unique prefix for keys.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct LookupMap<I: IO + Default, const K: KeyPrefixU8> {
    #[borsh_skip]
    pub io: I,
}

impl<const K: KeyPrefixU8> Default for LookupMap<Runtime, K> {
    fn default() -> Self {
        Self { io: Runtime }
    }
}

impl<I: IO + Default, const K: KeyPrefixU8> LookupMap<I, K> {
    /// Create a new map.
    pub fn new(io: I) -> Self {
        Self { io }
    }

    /// Build key for this map scope
    fn raw_key_to_storage_key(&self, key_raw: &[u8]) -> Vec<u8> {
        bytes_to_key(K.into(), key_raw)
    }

    /// Returns `true` if the serialized key is present in the map.
    #[allow(dead_code)]
    pub fn contains_key_raw(&self, key_raw: &[u8]) -> bool {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        self.io.storage_has_key(&storage_key)
    }

    /// Returns the serialized value corresponding to the serialized key.
    #[allow(dead_code)]
    pub fn get_raw(&self, key_raw: &[u8]) -> Option<Vec<u8>> {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        self.io.read_storage(&storage_key).map(|s| s.to_vec())
    }

    /// Inserts a serialized key-value pair into the map.
    pub fn insert_raw(&mut self, key_raw: &[u8], value_raw: &[u8]) {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        self.io.write_storage(&storage_key, value_raw);
    }

    /// Removes a serialized key from the map, returning the serialized value at the key if the key
    /// was previously in the map.
    #[allow(dead_code)]
    pub fn remove_raw(&mut self, key_raw: &[u8]) -> Option<Vec<u8>> {
        let storage_key = self.raw_key_to_storage_key(key_raw);
        self.io.remove_storage(&storage_key).map(|s| s.to_vec())
    }
}

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct BijectionMap<I: IO + Default, const LR: KeyPrefixU8, const RL: KeyPrefixU8> {
    #[borsh_skip]
    pub io: I,
}

impl<I: IO + Copy + Default, const LR: KeyPrefixU8, const RL: KeyPrefixU8> BijectionMap<I, LR, RL> {
    fn left_to_right(&self) -> LookupMap<I, LR> {
        LookupMap { io: self.io }
    }

    fn right_to_left(&self) -> LookupMap<I, RL> {
        LookupMap { io: self.io }
    }

    pub fn insert(&self, value_left: &[u8], value_right: &[u8]) {
        self.left_to_right().insert_raw(value_left, value_right);
        self.right_to_left().insert_raw(value_right, value_left);
    }

    pub fn lookup_left(&self, value_left: &[u8]) -> Option<Vec<u8>> {
        self.left_to_right().get_raw(value_left)
    }

    #[allow(dead_code)]
    pub fn lookup_right(&self, value_right: &[u8]) -> Option<Vec<u8>> {
        self.right_to_left().get_raw(value_right)
    }

    #[allow(dead_code)]
    pub fn remove_left(&self, value_left: &[u8]) {
        if let Some(value_right) = self.left_to_right().remove_raw(value_left) {
            self.right_to_left().remove_raw(value_right.as_slice());
        }
    }

    #[allow(dead_code)]
    pub fn remove_right(&self, value_right: &[u8]) {
        if let Some(value_left) = self.right_to_left().remove_raw(value_right) {
            self.left_to_right().remove_raw(value_left.as_slice());
        }
    }
}
