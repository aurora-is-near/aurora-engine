use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage::{address_to_key, bytes_to_key, storage_to_key, KeyPrefix};
use aurora_engine_types::types::Address;
use aurora_engine_types::{Vec, H160, H256, U256};

pub fn get_storage<I: IO>(io: &I, address: &Address, key: &H256, generation: u32) -> H256 {
    io.read_storage(storage_to_key(address, key, generation).as_ref())
        .and_then(|value| {
            if value.len() == 32 {
                let mut buf = [0u8; 32];
                value.copy_to_slice(&mut buf);
                Some(H256(buf))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Get EVM code from contract storage
pub fn get_code<I: IO>(io: &I, address: &revm::primitives::Address) -> Vec<u8> {
    io.read_storage(&address_to_key(KeyPrefix::Code, &from_address(address)))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

/// Get EVM code by `code_hash` from contract storage
pub fn get_code_by_code_hash<I: IO>(io: &I, code_hash: &revm::primitives::B256) -> Vec<u8> {
    io.read_storage(&storage_key(code_hash.0.as_slice()))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

/// Convert REVM `Address` to Engine `Address`
fn from_address(address: &revm::primitives::Address) -> Address {
    Address::new(H160::from(address.0 .0))
}

/// Get balance from contract storage
pub fn get_balance<I: IO>(io: &I, address: &revm::primitives::Address) -> revm::primitives::U256 {
    let addr = from_address(address);
    let mut raw: Vec<u8> = Vec::new();
    io.read_u256(&address_to_key(KeyPrefix::Balance, &addr))
        .unwrap_or_else(|_| U256::zero())
        .to_big_endian(&mut raw);
    revm::primitives::U256::from_be_slice(&raw)
}

/// Get nonce from contract storage
pub fn get_nonce<I: IO>(io: &I, address: &revm::primitives::Address) -> u64 {
    io.read_u256(&address_to_key(KeyPrefix::Nonce, &from_address(address)))
        .unwrap_or_else(|_| U256::zero())
        .as_u64()
}

/// Contract storage key for `CodeHash`
fn storage_key(value: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(1 + value.len());
    bytes.extend_from_slice(value);
    bytes_to_key(KeyPrefix::CodeHash, &bytes)
}
