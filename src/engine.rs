#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use crate::types::{FunctionCallArgs, ViewCallArgs};
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

    pub fn set_code(address: &H160, code: &[u8]) {}

    pub fn remove_code(address: &H160) {}

    pub fn get_code(address: &H160) -> Vec<u8> {
        [].to_vec() // TODO
    }

    pub fn set_nonce(address: &H160, nonce: &U256) {}

    pub fn remove_nonce(address: &H160) {}

    pub fn get_nonce(address: &H160) -> U256 {
        U256::zero() // TODO
    }

    pub fn set_balance(address: &H160, balance: &U256) {}

    pub fn remove_balance(address: &H160) {}

    pub fn get_balance(address: &H160) -> U256 {
        U256::zero() // TODO
    }

    pub fn remove_storage(address: &H160, key: &H256) {}

    pub fn set_storage(address: &H160, key: &H256, value: &H256) {}

    pub fn get_storage(address: &H160, key: &H256) -> H256 {
        H256::zero() // TODO
    }

    pub fn is_account_empty(address: &H160) -> bool {
        true
    }

    pub fn remove_all_storage(_address: &H160) {}

    pub fn remove_account_if_empty(address: &H160) {}

    pub fn remove_account(address: &H160) {
        Self::remove_nonce(address);
        Self::remove_balance(address);
        Self::remove_code(address);
        Self::remove_all_storage(address);
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
