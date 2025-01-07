//! This module contains `CachedIO`, a light wrapper over any IO instance
//! which will cache the input read and output written by the underlying instance.
//! It has no impact on the storage access functions of the trait.
//! The purpose of this struct is to capture the input and output from the underlying
//! IO instance for the purpose of passing it to `Hashchain::add_block_tx`.

use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::Vec;
use core::cell::RefCell;

#[derive(Debug, Clone, Copy)]
pub struct CachedIO<'cache, I> {
    inner: I,
    cache: &'cache RefCell<IOCache>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrappedInput<T> {
    Input(Vec<u8>),
    Wrapped(T),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IOCache {
    pub input: Vec<u8>,
    pub output: Vec<u8>,
}

impl<'cache, I> CachedIO<'cache, I> {
    pub const fn new(io: I, cache: &'cache RefCell<IOCache>) -> Self {
        Self { inner: io, cache }
    }
}

impl IOCache {
    pub fn set_input(&mut self, value: Vec<u8>) {
        self.input = value;
    }

    pub fn set_output(&mut self, value: Vec<u8>) {
        self.output = value;
    }
}

impl<T: StorageIntermediate> StorageIntermediate for WrappedInput<T> {
    fn len(&self) -> usize {
        match self {
            Self::Input(bytes) => bytes.len(),
            Self::Wrapped(x) => x.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Input(bytes) => bytes.is_empty(),
            Self::Wrapped(x) => x.is_empty(),
        }
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        match self {
            Self::Input(bytes) => buffer.copy_from_slice(bytes),
            Self::Wrapped(x) => x.copy_to_slice(buffer),
        }
    }
}

impl<'cache, I: IO> IO for CachedIO<'cache, I> {
    type StorageValue = WrappedInput<I::StorageValue>;

    fn read_input(&self) -> Self::StorageValue {
        let input = self.inner.read_input().to_vec();
        self.cache.borrow_mut().set_input(input.clone());
        WrappedInput::Input(input)
    }

    fn return_output(&mut self, value: &[u8]) {
        self.cache.borrow_mut().set_output(value.to_vec());
        self.inner.return_output(value);
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        self.inner.read_storage(key).map(WrappedInput::Wrapped)
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        self.inner.storage_has_key(key)
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        self.inner
            .write_storage(key, value)
            .map(WrappedInput::Wrapped)
    }

    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        match value {
            WrappedInput::Wrapped(x) => self
                .inner
                .write_storage_direct(key, x)
                .map(WrappedInput::Wrapped),
            WrappedInput::Input(bytes) => self
                .inner
                .write_storage(key, &bytes)
                .map(WrappedInput::Wrapped),
        }
    }

    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue> {
        self.inner.remove_storage(key).map(WrappedInput::Wrapped)
    }
}
