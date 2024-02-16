use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage::{address_to_key, storage_to_key, KeyPrefix};
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

pub fn get_code<I: IO>(io: &I, address: &revm::primitives::Address) -> Vec<u8> {
    let addr = from_address(address);
    io.read_storage(&address_to_key(KeyPrefix::Code, &addr))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

fn from_address(address: &revm::primitives::Address) -> Address {
    let raw = address.0 .0;
    Address::new(H160::from(raw))
}

pub fn get_balance<I: IO>(io: &I, address: &revm::primitives::Address) -> revm::primitives::U256 {
    let addr = from_address(address);
    let mut raw: Vec<u8> = Vec::new();
    io.read_u256(&address_to_key(KeyPrefix::Balance, &addr))
        .unwrap_or_else(|_| U256::zero())
        .to_big_endian(&mut raw);
    revm::primitives::U256::from_be_slice(&raw)
}

pub fn get_nonce<I: IO>(io: &I, address: &revm::primitives::Address) -> u64 {
    let addr = from_address(address);
    io.read_u256(&address_to_key(KeyPrefix::Nonce, &addr))
        .unwrap_or_else(|_| U256::zero())
        .as_u64()
}
