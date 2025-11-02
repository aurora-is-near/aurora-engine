//! This module provides a type with implements IO and allows fully overriding the state of
//! specified contracts. It accomplishes this by having a short-circuit in the read function
//! which will look at a map instead of the DB for specified addresses.

use alloc::{borrow::Cow, collections::BTreeMap};

use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::{parameters::simulate::H256BorshWrapper, storage, H160, H256};

#[derive(Clone, Copy)]
pub struct StorageOverride<'state, I> {
    pub inner: I,
    pub state_override: &'state BTreeMap<H160, BTreeMap<H256BorshWrapper, H256BorshWrapper>>,
}

pub enum StorageValueOverride<'a, T> {
    Cow(Cow<'a, [u8]>),
    Original(T),
}

impl<T> StorageIntermediate for StorageValueOverride<'_, T>
where
    T: StorageIntermediate,
{
    fn len(&self) -> usize {
        match self {
            StorageValueOverride::Cow(cow) => cow.len(),
            StorageValueOverride::Original(original) => original.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            StorageValueOverride::Cow(cow) => cow.is_empty(),
            StorageValueOverride::Original(original) => original.is_empty(),
        }
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        match self {
            StorageValueOverride::Cow(cow) => {
                buffer.copy_from_slice(cow);
            }
            StorageValueOverride::Original(original) => {
                original.copy_to_slice(buffer);
            }
        }
    }
}

impl<'a, I> IO for StorageOverride<'a, I>
where
    I: IO,
{
    type StorageValue = StorageValueOverride<'a, I::StorageValue>;

    fn read_input(&self) -> Self::StorageValue {
        StorageValueOverride::Original(self.inner.read_input())
    }

    fn return_output(&mut self, value: &[u8]) {
        self.inner.return_output(value);
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        fn deconstruct_storage_key(key: &[u8]) -> Option<(H160, H256)> {
            let version = *key.first()?;
            assert!(
                version == u8::from(storage::VersionPrefix::V1),
                "Unexpected version"
            );
            if *key.get(1)? == u8::from(storage::KeyPrefix::Storage) {
                let key_len = key.len();
                // Lengths are 54 or 58 bytes, depending on if the generation is present or not
                if key_len == 54 {
                    let address = H160::from_slice(&key[2..22]);
                    let value = H256::from_slice(&key[22..54]);
                    Some((address, value))
                } else if key_len == 58 {
                    let address = H160::from_slice(&key[2..22]);
                    let value = H256::from_slice(&key[26..58]);
                    Some((address, value))
                } else {
                    panic!("Unexpected storage key length")
                }
            } else {
                None
            }
        }

        match deconstruct_storage_key(key) {
            None => self
                .inner
                .read_storage(key)
                .map(StorageValueOverride::Original),
            Some((address, index)) => self.state_override.get(&address).map_or_else(
                || {
                    self.inner
                        .read_storage(key)
                        .map(StorageValueOverride::Original)
                },
                |state_override| {
                    let index = index.into();
                    state_override
                        .get(&index)
                        .map(|value| StorageValueOverride::Cow(Cow::Borrowed(&value.0)))
                },
            ),
        }
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        self.read_storage(key).is_some()
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        self.inner
            .write_storage(key, value)
            .map(StorageValueOverride::Original)
    }

    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        match value {
            StorageValueOverride::Cow(cow) => self.inner.write_storage(key, &cow),
            StorageValueOverride::Original(original) => {
                self.inner.write_storage_direct(key, original)
            }
        }
        .map(StorageValueOverride::Original)
    }

    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue> {
        self.inner
            .remove_storage(key)
            .map(StorageValueOverride::Original)
    }
}
