#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use crate::sdk;
use crate::types::{
    address_to_key, bytes_to_hex, log_to_bytes, storage_to_key, u256_to_arr, FunctionCallArgs,
    KeyPrefix, ViewCallArgs,
};
use evm::backend::{Apply, ApplyBackend, Basic, Log};
use evm::{ExitReason, ExitSucceed};
use primitive_types::{H160, H256, U256};

pub struct Engine {
    chain_id: U256,
    origin: H160,
}

impl Engine {
    pub fn new(chain_id: u64, origin: H160) -> Self {
        Self {
            chain_id: U256::from(chain_id),
            origin,
        }
    }

    pub fn set_code(address: &H160, code: &[u8]) {
        sdk::write_storage(&address_to_key(KeyPrefix::Code, address), code);
    }

    pub fn remove_code(address: &H160) {
        sdk::remove_storage(&address_to_key(KeyPrefix::Code, address))
    }

    pub fn get_code(address: &H160) -> Vec<u8> {
        sdk::read_storage(&address_to_key(KeyPrefix::Code, address)).unwrap_or_else(Vec::new)
    }

    pub fn set_nonce(address: &H160, nonce: &U256) {
        sdk::write_storage(
            &address_to_key(KeyPrefix::Nonce, address),
            &u256_to_arr(nonce),
        );
    }

    pub fn remove_nonce(address: &H160) {
        sdk::remove_storage(&address_to_key(KeyPrefix::Nonce, address))
    }

    pub fn get_nonce(address: &H160) -> U256 {
        sdk::read_storage(&address_to_key(KeyPrefix::Nonce, address))
            .map(|value| U256::from_big_endian(&value))
            .unwrap_or_else(U256::zero)
    }

    pub fn set_balance(address: &H160, balance: &U256) {
        sdk::write_storage(
            &address_to_key(KeyPrefix::Balance, address),
            &u256_to_arr(balance),
        );
    }

    pub fn remove_balance(address: &H160) {
        sdk::remove_storage(&address_to_key(KeyPrefix::Balance, address))
    }

    pub fn get_balance(address: &H160) -> U256 {
        sdk::read_storage(&address_to_key(KeyPrefix::Balance, address))
            .map(|value| U256::from_big_endian(&value))
            .unwrap_or_else(U256::zero)
    }

    pub fn remove_storage(address: &H160, key: &H256) {
        sdk::remove_storage(&storage_to_key(address, key));
    }

    pub fn set_storage(address: &H160, key: &H256, value: &H256) {
        sdk::write_storage(&storage_to_key(address, key), &value.0);
    }

    pub fn get_storage(address: &H160, key: &H256) -> H256 {
        sdk::read_storage(&storage_to_key(address, key))
            .map(|value| H256::from_slice(&value))
            .unwrap_or_else(H256::default)
    }

    pub fn is_account_empty(address: &H160) -> bool {
        let balance = Self::get_balance(address);
        let nonce = Self::get_nonce(address);
        let code_len = Self::get_code(address).len();
        balance == U256::zero() && nonce == U256::zero() && code_len == 0
    }

    /// Removes all storage for the given address.
    pub fn remove_all_storage(_address: &H160) {
        // FIXME: there is presently no way to prefix delete trie state.
    }

    /// Removes an account.
    pub fn remove_account(address: &H160) {
        Self::remove_nonce(address);
        Self::remove_balance(address);
        Self::remove_code(address);
        Self::remove_all_storage(address);
    }

    /// Removes an account if it is empty.
    pub fn remove_account_if_empty(address: &H160) {
        if Self::is_account_empty(address) {
            Self::remove_account(address);
        }
    }

    pub fn deploy_code(&self, input: &[u8]) -> (ExitReason, H160) {
        (ExitReason::Succeed(ExitSucceed::Stopped), H160::zero()) // TODO
    }

    pub fn call(&self, input: &[u8]) -> (ExitReason, Vec<u8>) {
        (ExitReason::Succeed(ExitSucceed::Stopped), [].to_vec()) // TODO
    }

    pub fn view(&self, args: ViewCallArgs) -> (ExitReason, Vec<u8>) {
        (ExitReason::Succeed(ExitSucceed::Stopped), [].to_vec()) // TODO
    }
}

impl evm::backend::Backend for Engine {
    fn gas_price(&self) -> U256 {
        todo!() // TODO
    }

    fn origin(&self) -> H160 {
        todo!() // TODO
    }

    fn block_hash(&self, _: U256) -> H256 {
        todo!() // TODO
    }

    fn block_number(&self) -> U256 {
        todo!() // TODO
    }

    fn block_coinbase(&self) -> H160 {
        todo!() // TODO
    }

    fn block_timestamp(&self) -> U256 {
        todo!() // TODO
    }

    fn block_difficulty(&self) -> U256 {
        todo!() // TODO
    }

    fn block_gas_limit(&self) -> U256 {
        todo!() // TODO
    }

    fn chain_id(&self) -> U256 {
        todo!() // TODO
    }

    fn exists(&self, _: H160) -> bool {
        todo!() // TODO
    }

    fn basic(&self, _: H160) -> Basic {
        todo!() // TODO
    }

    fn code(&self, _: H160) -> Vec<u8> {
        todo!() // TODO
    }

    fn storage(&self, _: H160, _: H256) -> H256 {
        todo!() // TODO
    }

    fn original_storage(&self, _: H160, _: H256) -> Option<H256> {
        todo!() // TODO
    }
}

impl ApplyBackend for Engine {
    fn apply<A, I, L>(&mut self, _: A, _: L, _: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
        todo!() // TODO
    }
}
