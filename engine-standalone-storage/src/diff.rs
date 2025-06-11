use aurora_engine_types::borsh::{self, BorshDeserialize, BorshSerialize};
use std::{
    collections::{btree_map, BTreeMap},
    fmt,
};

#[derive(Default, Clone, BorshDeserialize, BorshSerialize, PartialEq, Eq)]
#[borsh(crate = "aurora_engine_types::borsh")]
/// Collection of Engine state keys which changed by executing a transaction.
pub struct Diff(BTreeMap<Vec<u8>, DiffValue>);

impl fmt::Debug for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_struct("Diff");
        for (k, v) in &self.0 {
            if let Some(v) = v.value() {
                f.field(&hex::encode(k), &hex::encode(v));
            } else {
                f.field(&hex::encode(k), &"none");
            }
        }
        f.finish()
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize, PartialEq, Eq)]
#[borsh(crate = "aurora_engine_types::borsh")]
pub enum DiffValue {
    Modified(Vec<u8>),
    Deleted,
}

impl DiffValue {
    #[must_use]
    pub fn value(&self) -> Option<&[u8]> {
        match self {
            Self::Deleted => None,
            Self::Modified(new_value) => Some(new_value.as_slice()),
        }
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn take_value(self) -> Option<Vec<u8>> {
        match self {
            Self::Deleted => None,
            Self::Modified(new_value) => Some(new_value),
        }
    }

    pub fn try_to_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        borsh::to_vec(&self)
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
        self.0.clear();
    }

    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<&DiffValue> {
        self.0.get(key)
    }

    pub fn take(&mut self, key: &[u8]) -> Option<DiffValue> {
        self.0.remove(key)
    }

    pub fn iter(&self) -> btree_map::Iter<Vec<u8>, DiffValue> {
        self.0.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn try_to_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        borsh::to_vec(&self)
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        Self::try_from_slice(bytes)
    }
}

impl<'diff> IntoIterator for &'diff Diff {
    type Item = (&'diff Vec<u8>, &'diff DiffValue);
    type IntoIter = btree_map::Iter<'diff, Vec<u8>, DiffValue>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
