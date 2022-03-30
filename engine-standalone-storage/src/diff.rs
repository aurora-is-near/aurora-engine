use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{btree_map, BTreeMap};

#[derive(Debug, Default, Clone, BorshDeserialize, BorshSerialize, PartialEq, Eq)]
/// Collection of Engine state keys which changed by executing a transaction.
pub struct Diff(BTreeMap<Vec<u8>, DiffValue>);

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize, PartialEq, Eq)]
pub enum DiffValue {
    Modified(Vec<u8>),
    Deleted,
}

impl DiffValue {
    pub fn value(&self) -> Option<&[u8]> {
        match self {
            Self::Deleted => None,
            Self::Modified(new_value) => Some(new_value.as_slice()),
        }
    }

    pub fn take_value(self) -> Option<Vec<u8>> {
        match self {
            Self::Deleted => None,
            Self::Modified(new_value) => Some(new_value),
        }
    }

    pub fn try_to_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        self.try_to_vec()
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        Self::try_from_slice(bytes)
    }
}

impl Diff {
    /// Compose two Diffs into a single one. If there is a conflict between them
    /// then the value from the given (`other`) Diff is kept.
    pub fn append(&mut self, mut other: Self) {
        self.0.append(&mut other.0);
    }

    pub fn modify(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.0.insert(key, DiffValue::Modified(value));
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.0.insert(key, DiffValue::Deleted);
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    pub fn get(&self, key: &[u8]) -> Option<&DiffValue> {
        self.0.get(key)
    }

    pub fn take(&mut self, key: &[u8]) -> Option<DiffValue> {
        self.0.remove(key)
    }

    pub fn iter(&self) -> btree_map::Iter<Vec<u8>, DiffValue> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn try_to_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        self.try_to_vec()
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        Self::try_from_slice(bytes)
    }
}
