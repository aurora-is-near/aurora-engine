use crate::error;
use crate::prelude::{vec, Vec};
use aurora_engine_types::U256;
use borsh::{BorshDeserialize, BorshSerialize};

/// The purpose of this trait is to represent a reference to a value that
/// could be obtained by IO, but without eagerly loading it into memory.
/// For example, the NEAR runtime registers API allows querying the length
/// of some bytes read from input or storage without loading them into the
/// wasm memory.
pub trait StorageIntermediate: Sized {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn copy_to_slice(&self, buffer: &mut [u8]);

    fn to_vec(&self) -> Vec<u8> {
        let size = self.len();
        let mut buf = vec![0u8; size];
        self.copy_to_slice(&mut buf);
        buf
    }

    fn to_value<T: BorshDeserialize>(&self) -> Result<T, error::BorshDeserializeError> {
        let bytes = self.to_vec();
        T::try_from_slice(&bytes).map_err(|_| error::BorshDeserializeError)
    }
}

/// Trait for reading/writing values from storage and a generalized `stdin`/`stdout`.
pub trait IO {
    /// A type giving a reference to a value obtained by IO without loading it
    /// into memory. For example, in the case of a wasm contract on NEAR this
    /// will correspond to a register index.
    type StorageValue: StorageIntermediate;

    /// Read bytes that were passed as input to the process. This can be thought of as a
    /// generalization of `stdin` or command-line arguments. In the case of wasm contracts
    /// on NEAR these would be the arguments to the method.
    fn read_input(&self) -> Self::StorageValue;

    /// Return a value to an external process. In the case of wasm contracts on NEAR
    /// this corresponds to the return value from the contract method.
    fn return_output(&mut self, value: &[u8]);

    /// Read the value in storage at the given key, if any.
    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue>;

    /// Check if there is a value in storage at the given key, but do not read the value.
    /// Equivalent to `self.read_storage(key).is_some()` but more efficient.
    fn storage_has_key(&self, key: &[u8]) -> bool;

    /// Write the given value to storage under the given key. Returns a reference to the old
    /// value stored at that key (if any).
    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue>;

    /// Write a `StorageIntermediate` to storage directly under the given key
    /// (without ever needing to load the value into memory).Returns a reference
    /// to the old value stored at that key (if any).
    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue>;

    /// Remove entry from storage and capture the value present at the given key (if any)
    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue>;

    /// Read the length of the bytes stored at the given key.
    fn read_storage_len(&self, key: &[u8]) -> Option<usize> {
        self.read_storage(key).map(|s| s.len())
    }

    /// Convenience function to read the input and deserialize the bytes using borsh.
    fn read_input_borsh<U: BorshDeserialize>(&self) -> Result<U, error::BorshDeserializeError> {
        self.read_input().to_value()
    }

    /// Convenience function to read the input into a 20-byte array.
    fn read_input_arr20(&self) -> Result<[u8; 20], error::IncorrectInputLength> {
        let value = self.read_input();

        if value.len() != 20 {
            return Err(error::IncorrectInputLength);
        }

        let mut buf = [0u8; 20];
        value.copy_to_slice(&mut buf);
        Ok(buf)
    }

    /// Convenience function to read the input into a 32-byte array.
    fn read_input_arr32(&self) -> Result<[u8; 32], error::IncorrectInputLength> {
        let value = self.read_input();

        if value.len() != 32 {
            return Err(error::IncorrectInputLength);
        }

        let mut buf = [0u8; 32];
        value.copy_to_slice(&mut buf);
        Ok(buf)
    }

    /// Convenience function to store the input directly in storage under the
    /// given key (without ever loading it into memory).
    fn read_input_and_store(&mut self, key: &[u8]) {
        let value = self.read_input();
        self.write_storage_direct(key, value);
    }

    /// Convenience function to read a 32-bit unsigned integer from storage
    /// (assumes little-endian encoding).
    fn read_u32(&self, key: &[u8]) -> Result<u32, error::ReadU32Error> {
        let value = self
            .read_storage(key)
            .ok_or(error::ReadU32Error::MissingValue)?;

        if value.len() != 4 {
            return Err(error::ReadU32Error::InvalidU32);
        }

        let mut result = [0u8; 4];
        value.copy_to_slice(&mut result);
        Ok(u32::from_le_bytes(result))
    }

    /// Convenience function to read a 64-bit unsigned integer from storage
    /// (assumes little-endian encoding).
    fn read_u64(&self, key: &[u8]) -> Result<u64, error::ReadU64Error> {
        let value = self
            .read_storage(key)
            .ok_or(error::ReadU64Error::MissingValue)?;

        if value.len() != 8 {
            return Err(error::ReadU64Error::InvalidU64);
        }

        let mut result = [0u8; 8];
        value.copy_to_slice(&mut result);
        Ok(u64::from_le_bytes(result))
    }

    /// Convenience function to read a 256-bit unsigned integer from storage
    /// (assumes big-endian encoding).
    fn read_u256(&self, key: &[u8]) -> Result<U256, error::ReadU256Error> {
        let value = self
            .read_storage(key)
            .ok_or(error::ReadU256Error::MissingValue)?;

        if value.len() != 32 {
            return Err(error::ReadU256Error::InvalidU256);
        }

        let mut result = [0u8; 32];
        value.copy_to_slice(&mut result);
        Ok(U256::from_big_endian(&result))
    }

    fn write_borsh<T: BorshSerialize>(
        &mut self,
        key: &[u8],
        value: &T,
    ) -> Option<Self::StorageValue> {
        let bytes = value.try_to_vec().ok()?;
        self.write_storage(key, &bytes)
    }
}
