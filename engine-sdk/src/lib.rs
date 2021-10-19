#![feature(array_methods)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![cfg_attr(feature = "log", feature(panic_info_message))]

use crate::prelude::{vec, Address, PromiseResult, Vec, H256, STORAGE_PRICE_PER_BYTE};
pub use types::keccak;

pub mod error;
pub mod io;
pub mod near_runtime;
mod prelude;
pub mod types;

use near_runtime::exports;

const ECRECOVER_MESSAGE_SIZE: u64 = 32;
const ECRECOVER_SIGNATURE_LENGTH: u64 = 64;
const ECRECOVER_MALLEABILITY_FLAG: u64 = 1;

const GAS_FOR_STATE_MIGRATION: u64 = 100_000_000_000_000;

#[allow(dead_code)]
pub fn block_timestamp() -> u64 {
    // NEAR timestamp is in nanoseconds
    let timestamp_ns = unsafe { exports::block_timestamp() };
    timestamp_ns / 1_000_000_000 // convert to seconds for Ethereum compatibility
}

pub fn block_index() -> u64 {
    unsafe { exports::block_index() }
}

#[allow(dead_code)]
pub fn panic() {
    unsafe { exports::panic() }
}

pub fn panic_utf8(bytes: &[u8]) -> ! {
    unsafe {
        exports::panic_utf8(bytes.len() as u64, bytes.as_ptr() as u64);
    }
    unreachable!()
}

#[allow(dead_code)]
pub fn log_utf8(bytes: &[u8]) {
    unsafe {
        exports::log_utf8(bytes.len() as u64, bytes.as_ptr() as u64);
    }
}

pub fn predecessor_account_id() -> Vec<u8> {
    unsafe {
        exports::predecessor_account_id(1);
        let bytes: Vec<u8> = vec![0u8; exports::register_len(1) as usize];
        exports::read_register(1, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

#[allow(dead_code)]
pub fn signer_account_id() -> Vec<u8> {
    unsafe {
        exports::signer_account_id(1);
        let bytes: Vec<u8> = vec![0u8; exports::register_len(1) as usize];
        exports::read_register(1, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

#[allow(dead_code)]
pub fn signer_account_pk() -> Vec<u8> {
    unsafe {
        exports::signer_account_pk(1);
        let bytes: Vec<u8> = vec![0u8; exports::register_len(1) as usize];
        exports::read_register(1, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Calls environment sha256 on given input.
pub fn sha256(input: &[u8]) -> H256 {
    unsafe {
        exports::sha256(input.len() as u64, input.as_ptr() as u64, 1);
        let bytes = H256::zero();
        exports::read_register(1, bytes.0.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Calls environment ripemd160 on given input.
pub fn ripemd160(input: &[u8]) -> [u8; 20] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::ripemd160(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 20];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

/// Recover address from message hash and signature.
pub fn ecrecover(hash: H256, signature: &[u8]) -> Result<Address, ECRecoverErr> {
    unsafe {
        let hash_ptr = hash.as_ptr() as u64;
        let sig_ptr = signature.as_ptr() as u64;
        const RECOVER_REGISTER_ID: u64 = 1;
        const KECCACK_REGISTER_ID: u64 = 2;
        let result = exports::ecrecover(
            ECRECOVER_MESSAGE_SIZE,
            hash_ptr,
            ECRECOVER_SIGNATURE_LENGTH,
            sig_ptr,
            signature[64] as u64,
            ECRECOVER_MALLEABILITY_FLAG,
            RECOVER_REGISTER_ID,
        );
        if result == (true as u64) {
            // The result from the ecrecover call is in a register; we can use this
            // register directly for the input to keccak256. This is why the length is
            // set to `u64::MAX`.
            exports::keccak256(u64::MAX, RECOVER_REGISTER_ID, KECCACK_REGISTER_ID);
            let keccak_hash_bytes = [0u8; 32];
            exports::read_register(KECCACK_REGISTER_ID, keccak_hash_bytes.as_ptr() as u64);
            Ok(Address::from_slice(&keccak_hash_bytes[12..]))
        } else {
            Err(ECRecoverErr)
        }
    }
}

/// Returns account id of the current account.
pub fn current_account_id() -> Vec<u8> {
    unsafe {
        exports::current_account_id(1);
        let bytes: Vec<u8> = vec![0u8; exports::register_len(1) as usize];
        exports::read_register(1, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Deploy code from given key in place of the current key.
pub fn self_deploy(code_key: &[u8]) {
    unsafe {
        // Load current account id into register 0.
        exports::current_account_id(0);
        // Use register 0 as the destination for the promise.
        let promise_id = exports::promise_batch_create(u64::MAX as _, 0);
        // Remove code from storage and store it in register 1.
        exports::storage_remove(code_key.len() as _, code_key.as_ptr() as _, 1);
        exports::promise_batch_action_deploy_contract(promise_id, u64::MAX, 1);
        promise_batch_action_function_call(
            promise_id,
            b"state_migration",
            &[],
            0,
            GAS_FOR_STATE_MIGRATION,
        )
    }
}

#[allow(dead_code)]
pub fn log(data: &str) {
    log_utf8(data.as_bytes())
}

#[macro_export]
macro_rules! log {
    ($e: expr) => {
        #[cfg(feature = "log")]
        $crate::log($e)
    };
}

#[allow(unused)]
pub fn prepaid_gas() -> u64 {
    unsafe { exports::prepaid_gas() }
}

pub fn promise_create(
    account_id: &[u8],
    method_name: &[u8],
    arguments: &[u8],
    amount: u128,
    gas: u64,
) -> u64 {
    unsafe {
        exports::promise_create(
            account_id.len() as _,
            account_id.as_ptr() as _,
            method_name.len() as _,
            method_name.as_ptr() as _,
            arguments.len() as _,
            arguments.as_ptr() as _,
            &amount as *const u128 as _,
            gas,
        )
    }
}

pub fn promise_then(
    promise_idx: u64,
    account_id: &[u8],
    method_name: &[u8],
    arguments: &[u8],
    amount: u128,
    gas: u64,
) -> u64 {
    unsafe {
        exports::promise_then(
            promise_idx,
            account_id.len() as _,
            account_id.as_ptr() as _,
            method_name.len() as _,
            method_name.as_ptr() as _,
            arguments.len() as _,
            arguments.as_ptr() as _,
            &amount as *const u128 as _,
            gas,
        )
    }
}

pub fn promise_return(promise_idx: u64) {
    unsafe {
        exports::promise_return(promise_idx);
    }
}

pub fn promise_results_count() -> u64 {
    unsafe { exports::promise_results_count() }
}

pub fn promise_result(result_idx: u64) -> PromiseResult {
    unsafe {
        match exports::promise_result(result_idx, 0) {
            0 => PromiseResult::NotReady,
            1 => {
                let bytes: Vec<u8> = vec![0; exports::register_len(0) as usize];
                exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
                PromiseResult::Successful(bytes)
            }
            2 => PromiseResult::Failed,
            _ => panic_utf8(b"ERR_PROMISE_RETURN_CODE"),
        }
    }
}

pub fn assert_private_call() {
    assert_eq!(
        predecessor_account_id(),
        current_account_id(),
        "ERR_PRIVATE_CALL"
    );
}

pub fn attached_deposit() -> u128 {
    unsafe {
        let data = [0u8; core::mem::size_of::<u128>()];
        exports::attached_deposit(data.as_ptr() as u64);
        u128::from_le_bytes(data)
    }
}

pub fn assert_one_yocto() {
    assert_eq!(attached_deposit(), 1, "ERR_1YOCTO_ATTACH")
}

pub fn promise_batch_action_transfer(promise_index: u64, amount: u128) {
    unsafe {
        exports::promise_batch_action_transfer(promise_index, &amount as *const u128 as _);
    }
}

pub fn storage_byte_cost() -> u128 {
    STORAGE_PRICE_PER_BYTE
}

pub fn promise_batch_create(account_id: &[u8]) -> u64 {
    unsafe { exports::promise_batch_create(account_id.len() as _, account_id.as_ptr() as _) }
}

pub fn promise_batch_action_function_call(
    promise_idx: u64,
    method_name: &[u8],
    arguments: &[u8],
    amount: u128,
    gas: u64,
) {
    unsafe {
        exports::promise_batch_action_function_call(
            promise_idx,
            method_name.len() as _,
            method_name.as_ptr() as _,
            arguments.len() as _,
            arguments.as_ptr() as _,
            &amount as *const u128 as _,
            gas,
        )
    }
}

pub struct ECRecoverErr;

impl ECRecoverErr {
    pub fn as_str(&self) -> &'static str {
        "ERR_ECRECOVER"
    }
}

impl AsRef<[u8]> for ECRecoverErr {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}
