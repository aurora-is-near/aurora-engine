use alloc::{vec, vec::Vec};

use primitive_types::H256;

mod exports {

    #[allow(unused)]
    extern "C" {
        // #############
        // # Registers #
        // #############
        pub(crate) fn read_register(register_id: u64, ptr: u64);
        pub(crate) fn register_len(register_id: u64) -> u64;
        // ###############
        // # Context API #
        // ###############
        fn current_account_id(register_id: u64);
        fn signer_account_id(register_id: u64);
        fn signer_account_pk(register_id: u64);
        pub(crate) fn predecessor_account_id(register_id: u64);
        pub(crate) fn input(register_id: u64);
        // TODO #1903 fn block_height() -> u64;
        pub(crate) fn block_index() -> u64;
        pub(crate) fn block_timestamp() -> u64;
        fn epoch_height() -> u64;
        fn storage_usage() -> u64;
        // #################
        // # Economics API #
        // #################
        fn account_balance(balance_ptr: u64);
        fn attached_deposit(balance_ptr: u64);
        fn prepaid_gas() -> u64;
        fn used_gas() -> u64;
        // ############
        // # Math API #
        // ############
        fn random_seed(register_id: u64);
        fn sha256(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn keccak256(value_len: u64, value_ptr: u64, register_id: u64);
        // #####################
        // # Miscellaneous API #
        // #####################
        pub(crate) fn value_return(value_len: u64, value_ptr: u64);
        pub(crate) fn panic();
        pub(crate) fn panic_utf8(len: u64, ptr: u64);
        pub(crate) fn log_utf8(len: u64, ptr: u64);
        fn log_utf16(len: u64, ptr: u64);
        fn abort(msg_ptr: u32, filename_ptr: u32, line: u32, col: u32);
        // ################
        // # Promises API #
        // ################
        fn promise_create(
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        fn promise_then(
            promise_index: u64,
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        fn promise_and(promise_idx_ptr: u64, promise_idx_count: u64) -> u64;
        fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64;
        fn promise_batch_then(promise_index: u64, account_id_len: u64, account_id_ptr: u64) -> u64;
        // #######################
        // # Promise API actions #
        // #######################
        fn promise_batch_action_create_account(promise_index: u64);
        fn promise_batch_action_deploy_contract(promise_index: u64, code_len: u64, code_ptr: u64);
        fn promise_batch_action_function_call(
            promise_index: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        );
        fn promise_batch_action_transfer(promise_index: u64, amount_ptr: u64);
        fn promise_batch_action_stake(
            promise_index: u64,
            amount_ptr: u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
        fn promise_batch_action_add_key_with_full_access(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
            nonce: u64,
        );
        fn promise_batch_action_add_key_with_function_call(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
            nonce: u64,
            allowance_ptr: u64,
            receiver_id_len: u64,
            receiver_id_ptr: u64,
            method_names_len: u64,
            method_names_ptr: u64,
        );
        fn promise_batch_action_delete_key(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
        fn promise_batch_action_delete_account(
            promise_index: u64,
            beneficiary_id_len: u64,
            beneficiary_id_ptr: u64,
        );
        // #######################
        // # Promise API results #
        // #######################
        fn promise_results_count() -> u64;
        fn promise_result(result_idx: u64, register_id: u64) -> u64;
        fn promise_return(promise_id: u64);
        // ###############
        // # Storage API #
        // ###############
        pub(crate) fn storage_write(
            key_len: u64,
            key_ptr: u64,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) -> u64;
        pub(crate) fn storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
        pub(crate) fn storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
        fn storage_has_key(key_len: u64, key_ptr: u64) -> u64;
        fn storage_iter_prefix(prefix_len: u64, prefix_ptr: u64) -> u64;
        fn storage_iter_range(start_len: u64, start_ptr: u64, end_len: u64, end_ptr: u64) -> u64;
        fn storage_iter_next(iterator_id: u64, key_register_id: u64, value_register_id: u64)
            -> u64;
        // ###############
        // # Validator API #
        // ###############
        fn validator_stake(account_id_len: u64, account_id_ptr: u64, stake_ptr: u64);
        fn validator_total_stake(stake_ptr: u64);
    }
}

pub fn read_input() -> Vec<u8> {
    unsafe {
        exports::input(0);
        let bytes: Vec<u8> = vec![0; exports::register_len(0) as usize];
        exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

pub fn read_input_arr20() -> [u8; 20] {
    unsafe {
        exports::input(0);
        let bytes = [0u8; 20];
        exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

pub fn return_output(value: &[u8]) {
    unsafe {
        exports::value_return(value.len() as u64, value.as_ptr() as u64);
    }
}

pub fn read_storage(key: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        if exports::storage_read(key.len() as u64, key.as_ptr() as u64, 0) == 1 {
            let bytes: Vec<u8> = vec![0u8; exports::register_len(0) as usize];
            exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
            Some(bytes)
        } else {
            None
        }
    }
}

pub fn write_storage(key: &[u8], value: &[u8]) {
    unsafe {
        exports::storage_write(
            key.len() as u64,
            key.as_ptr() as u64,
            value.len() as u64,
            value.as_ptr() as u64,
            0,
        );
    }
}

pub fn remove_storage(key: &[u8]) {
    unsafe {
        exports::storage_remove(key.len() as u64, key.as_ptr() as u64, 0);
    }
}

pub fn block_timestamp() -> u64 {
    unsafe { exports::block_timestamp() }
}

pub fn block_index() -> u64 {
    unsafe { exports::block_index() }
}

#[allow(dead_code)]
pub fn panic() {
    unsafe { exports::panic() }
}

#[allow(dead_code)]
pub fn panic_utf8(bytes: &[u8]) {
    unsafe {
        exports::panic_utf8(bytes.len() as u64, bytes.as_ptr() as u64);
    }
}

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

/// Calls environment keccak256 on given data.
pub fn keccak(data: &[u8]) -> H256 {
    unsafe {
        exports::keccak256(data.len() as u64, data.as_ptr() as u64, 1);
        let bytes = H256::zero();
        exports::read_register(1, bytes.0.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Calls environment panic with data encoded in hex as panic message.
pub fn panic_hex(data: &[u8]) -> ! {
    let message = crate::types::bytes_to_hex(data).into_bytes();
    unsafe { exports::panic_utf8(message.len() as _, message.as_ptr() as _) }
    unreachable!()
}
