use crate::TransactErrorKind;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::parameters::engine::TransactionStatus;
use aurora_engine_types::storage::{address_to_key, storage_to_key, KeyPrefix};
use aurora_engine_types::types::{u256_to_arr, Address, Wei};
use aurora_engine_types::{Vec, H256, U256};
use evm::{ExitError, ExitReason};

const BLOCK_HASH_PREFIX: u8 = 0;
const BLOCK_HASH_PREFIX_SIZE: usize = 1;
const BLOCK_HEIGHT_SIZE: usize = 8;
const CHAIN_ID_SIZE: usize = 32;

/// There is one Aurora block per NEAR block height (note: when heights in NEAR are skipped
/// they are interpreted as empty blocks on Aurora). The blockhash is derived from the height
/// according to
/// ```text
/// block_hash = sha256(concat(
///     BLOCK_HASH_PREFIX,
///     block_height as u64,
///     chain_id,
///     engine_account_id,
/// ))
/// ```
#[must_use]
pub fn compute_block_hash(chain_id: [u8; 32], block_height: u64, account_id: &[u8]) -> H256 {
    debug_assert_eq!(
        BLOCK_HASH_PREFIX_SIZE,
        core::mem::size_of_val(&BLOCK_HASH_PREFIX)
    );
    debug_assert_eq!(BLOCK_HEIGHT_SIZE, core::mem::size_of_val(&block_height));
    debug_assert_eq!(CHAIN_ID_SIZE, core::mem::size_of_val(&chain_id));
    let mut data = Vec::with_capacity(
        BLOCK_HASH_PREFIX_SIZE + BLOCK_HEIGHT_SIZE + CHAIN_ID_SIZE + account_id.len(),
    );
    data.push(BLOCK_HASH_PREFIX);
    data.extend_from_slice(&chain_id);
    data.extend_from_slice(account_id);
    data.extend_from_slice(&block_height.to_be_bytes());

    aurora_engine_sdk::sha256(&data)
}

pub fn get_generation<I: IO>(io: &I, address: &Address) -> u32 {
    io.read_storage(&address_to_key(KeyPrefix::Generation, address))
        .map_or(0, |value| {
            let mut bytes = [0u8; 4];
            value.copy_to_slice(&mut bytes);
            u32::from_be_bytes(bytes)
        })
}

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

pub fn get_code<I: IO>(io: &I, address: &Address) -> Vec<u8> {
    io.read_storage(&address_to_key(KeyPrefix::Code, address))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

pub fn get_balance<I: IO>(io: &I, address: &Address) -> Wei {
    let raw = io
        .read_u256(&address_to_key(KeyPrefix::Balance, address))
        .unwrap_or_else(|_| U256::zero());
    Wei::new(raw)
}

pub fn get_nonce<I: IO>(io: &I, address: &Address) -> U256 {
    io.read_u256(&address_to_key(KeyPrefix::Nonce, address))
        .unwrap_or_else(|_| U256::zero())
}

pub fn is_account_empty<I: IO>(io: &I, address: &Address) -> bool {
    get_balance(io, address).is_zero()
        && get_nonce(io, address).is_zero()
        && get_code_size(io, address) == 0
}

pub fn get_code_size<I: IO>(io: &I, address: &Address) -> usize {
    io.read_storage_len(&address_to_key(KeyPrefix::Code, address))
        .unwrap_or(0)
}

pub fn set_nonce<I: IO>(io: &mut I, address: &Address, nonce: &U256) {
    io.write_storage(
        &address_to_key(KeyPrefix::Nonce, address),
        &u256_to_arr(nonce),
    );
}

/// Removes an account.
pub fn remove_account<I: IO + Copy>(io: &mut I, address: &Address, generation: u32) {
    remove_nonce(io, address);
    remove_balance(io, address);
    remove_code(io, address);
    remove_all_storage(io, address, generation);
}

fn remove_nonce<I: IO>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Nonce, address));
}

pub fn remove_balance<I: IO + Copy>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Balance, address));
}

pub fn remove_code<I: IO>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Code, address));
}

/// Removes all storage for the given address.
pub fn remove_all_storage<I: IO>(io: &mut I, address: &Address, generation: u32) {
    // FIXME: there is presently no way to prefix delete trie state.
    // NOTE: There is not going to be a method on runtime for this.
    //     You may need to store all keys in a list if you want to do this in a contract.
    //     Maybe you can incentivize people to delete dead old keys. They can observe them from
    //     external indexer node and then issue special cleaning transaction.
    //     Either way you may have to store the nonce per storage address root. When the account
    //     has to be deleted the storage nonce needs to be increased, and the old nonce keys
    //     can be deleted over time. That's how TurboGeth does storage.
    set_generation(io, address, generation + 1);
}

/// Increments storage generation for a given address.
pub fn set_generation<I: IO>(io: &mut I, address: &Address, generation: u32) {
    io.write_storage(
        &address_to_key(KeyPrefix::Generation, address),
        &generation.to_be_bytes(),
    );
}

pub fn remove_storage<I: IO>(io: &mut I, address: &Address, key: &H256, generation: u32) {
    io.remove_storage(storage_to_key(address, key, generation).as_ref());
}

pub fn set_storage<I: IO>(
    io: &mut I,
    address: &Address,
    key: &H256,
    value: &H256,
    generation: u32,
) {
    io.write_storage(storage_to_key(address, key, generation).as_ref(), &value.0);
}

pub fn set_balance<I: IO>(io: &mut I, address: &Address, balance: &Wei) {
    io.write_storage(
        &address_to_key(KeyPrefix::Balance, address),
        &balance.to_bytes(),
    );
}

pub fn set_code<I: IO>(io: &mut I, address: &Address, code: &[u8]) {
    io.write_storage(&address_to_key(KeyPrefix::Code, address), code);
}

pub fn exit_reason_into_result(
    exit_reason: ExitReason,
    data: Vec<u8>,
) -> Result<TransactionStatus, TransactErrorKind> {
    match exit_reason {
        ExitReason::Succeed(_) => Ok(TransactionStatus::Succeed(data)),
        ExitReason::Revert(_) => Ok(TransactionStatus::Revert(data)),
        ExitReason::Error(ExitError::OutOfOffset) => Ok(TransactionStatus::OutOfOffset),
        ExitReason::Error(ExitError::OutOfFund) => Ok(TransactionStatus::OutOfFund),
        ExitReason::Error(ExitError::OutOfGas) => Ok(TransactionStatus::OutOfGas),
        ExitReason::Error(e) => Err(e.into()),
        ExitReason::Fatal(e) => Err(e.into()),
    }
}
