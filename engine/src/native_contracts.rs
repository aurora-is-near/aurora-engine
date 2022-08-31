use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::{
    storage::{self, KeyPrefix},
    types::Address,
    Vec,
};
use borsh::{BorshDeserialize, BorshSerialize};

const KEY_BYTES: &[u8] = b"native_contracts";

#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub enum ContractType {
    Erc20,
}

pub fn read_native_contracts<I: IO>(io: &I) -> Vec<(Address, ContractType)> {
    let key = storage::bytes_to_key(KeyPrefix::Config, KEY_BYTES);
    io.read_storage(&key)
        .map(|v| v.to_value().unwrap())
        .unwrap_or_default()
}

pub fn insert_native_contract<I: IO>(io: &mut I, address: Address, kind: ContractType) {
    let key = storage::bytes_to_key(KeyPrefix::Config, KEY_BYTES);
    let mut current_value: Vec<(Address, ContractType)> = io
        .read_storage(&key)
        .map(|v| v.to_value().unwrap())
        .unwrap_or_default();
    current_value.push((address, kind));
    io.write_borsh(&key, &current_value);
}
