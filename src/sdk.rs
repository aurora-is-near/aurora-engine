use crate::prelude::{vec, String, Vec, H256};
use crate::types::STORAGE_PRICE_PER_BYTE;
use borsh::{BorshDeserialize, BorshSerialize};

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
        pub(crate) fn current_account_id(register_id: u64);
        fn signer_account_id(register_id: u64);
        fn signer_account_pk(register_id: u64);
        pub(crate) fn predecessor_account_id(register_id: u64);
        pub(crate) fn input(register_id: u64);
        // TODO #1903 fn block_height() -> u64;
        pub(crate) fn block_index() -> u64;
        pub(crate) fn block_timestamp() -> u64;
        fn epoch_height() -> u64;
        pub(crate) fn storage_usage() -> u64;
        // #################
        // # Economics API #
        // #################
        fn account_balance(balance_ptr: u64);
        pub(crate) fn attached_deposit(balance_ptr: u64);
        pub(crate) fn prepaid_gas() -> u64;
        fn used_gas() -> u64;
        // ############
        // # Math API #
        // ############
        fn random_seed(register_id: u64);
        pub(crate) fn sha256(value_len: u64, value_ptr: u64, register_id: u64);
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
        pub(crate) fn promise_create(
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        pub(crate) fn promise_then(
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
        pub(crate) fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64;
        fn promise_batch_then(promise_index: u64, account_id_len: u64, account_id_ptr: u64) -> u64;
        // #######################
        // # Promise API actions #
        // #######################
        fn promise_batch_action_create_account(promise_index: u64);
        pub(crate) fn promise_batch_action_deploy_contract(
            promise_index: u64,
            code_len: u64,
            code_ptr: u64,
        );
        fn promise_batch_action_function_call(
            promise_index: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        );
        pub(crate) fn promise_batch_action_transfer(promise_index: u64, amount_ptr: u64);
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
        pub(crate) fn promise_results_count() -> u64;
        pub(crate) fn promise_result(result_idx: u64, register_id: u64) -> u64;
        pub(crate) fn promise_return(promise_id: u64);
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
        pub(crate) fn storage_has_key(key_len: u64, key_ptr: u64) -> u64;
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

#[allow(dead_code)]
pub fn read_input() -> Vec<u8> {
    unsafe {
        exports::input(0);
        let bytes: Vec<u8> = vec![0; exports::register_len(0) as usize];
        exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

#[allow(dead_code)]
pub fn read_input_arr20() -> [u8; 20] {
    unsafe {
        exports::input(0);
        let bytes = [0u8; 20];
        exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Reads current input and stores in the given key keeping data in the runtime.
pub fn read_input_and_store(key: &[u8]) {
    unsafe {
        exports::input(0);
        // Store register 0 into key, store the previous value in register 1.
        exports::storage_write(key.len() as _, key.as_ptr() as _, u64::MAX, 0, 1);
    }
}

#[allow(dead_code)]
pub fn return_output(value: &[u8]) {
    unsafe {
        exports::value_return(value.len() as u64, value.as_ptr() as u64);
    }
}

#[allow(dead_code)]
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

/// Read u64 from storage at given key.
pub fn read_u64(key: &[u8]) -> Option<u64> {
    unsafe {
        if exports::storage_read(key.len() as u64, key.as_ptr() as u64, 0) == 1 {
            let result = [0u8; 8];
            exports::read_register(0, result.as_ptr() as _);
            Some(u64::from_le_bytes(result))
        } else {
            None
        }
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn remove_storage(key: &[u8]) {
    unsafe {
        exports::storage_remove(key.len() as u64, key.as_ptr() as u64, 0);
    }
}

#[allow(dead_code)]
pub fn block_timestamp() -> u64 {
    unsafe { exports::block_timestamp() }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn log_utf8(bytes: &[u8]) {
    unsafe {
        exports::log_utf8(bytes.len() as u64, bytes.as_ptr() as u64);
    }
}

#[allow(dead_code)]
pub fn predecessor_account_id() -> Vec<u8> {
    unsafe {
        exports::predecessor_account_id(1);
        let bytes: Vec<u8> = vec![0u8; exports::register_len(1) as usize];
        exports::read_register(1, bytes.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Calls environment sha256 on given input.
#[allow(dead_code)]
pub fn sha256(input: &[u8]) -> H256 {
    unsafe {
        exports::sha256(input.len() as u64, input.as_ptr() as u64, 1);
        let bytes = H256::zero();
        exports::read_register(1, bytes.0.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Calls environment keccak256 on given input.
#[allow(dead_code)]
pub fn keccak(input: &[u8]) -> H256 {
    unsafe {
        exports::keccak256(input.len() as u64, input.as_ptr() as u64, 1);
        let bytes = H256::zero();
        exports::read_register(1, bytes.0.as_ptr() as *const u64 as u64);
        bytes
    }
}

/// Calls environment panic with data encoded in hex as panic message.
#[allow(dead_code)]
pub fn panic_hex(data: &[u8]) -> ! {
    let message = crate::types::bytes_to_hex(data).into_bytes();
    unsafe { exports::panic_utf8(message.len() as _, message.as_ptr() as _) }
    unreachable!()
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
        exports::promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 1);
    }
}

pub fn save_contract<T: BorshSerialize>(key: &str, data: &T) {
    write_storage(key.as_bytes(), &data.try_to_vec().unwrap()[..]);
}

pub fn get_contract_data<T: BorshDeserialize>(key: &str) -> T {
    let data = read_storage(key.as_bytes()).expect("Failed read storage");
    T::try_from_slice(&data[..]).unwrap()
}

pub fn log(data: String) {
    log_utf8(data.as_bytes())
}

pub fn storage_usage() -> u64 {
    unsafe { exports::storage_usage() }
}

pub fn prepaid_gas() -> u64 {
    unsafe { exports::prepaid_gas() }
}

pub fn promise_create(
    account_id: String,
    method_name: &[u8],
    arguments: &[u8],
    amount: u128,
    gas: u64,
) -> u64 {
    let account_id = account_id.as_bytes();
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
    account_id: String,
    method_name: &[u8],
    arguments: &[u8],
    amount: u128,
    gas: u64,
) -> u64 {
    let account_id = account_id.as_bytes();
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

/*pub fn promise_result(result_idx: u64) -> PromiseResult {
    unsafe {
        match exports::promise_result(result_idx, 0) {
            0 => PromiseResult::NotReady,
            1 => {
                let bytes: Vec<u8> = vec![0; exports::register_len(0) as usize];
                exports::read_register(0, bytes.as_ptr() as *const u64 as u64);
                PromiseResult::Successful(bytes)
            }
            2 => PromiseResult::Failed,
            _ => panic!("{}", RETURN_CODE_ERR),
        }
    }
}*/

pub fn assert_private_call() {
    assert_eq!(
        predecessor_account_id(),
        current_account_id(),
        "Function is private"
    );
}

pub fn attached_deposit() -> u128 {
    use core::intrinsics::size_of;
    unsafe {
        let data = [0u8; size_of::<u128>()];
        exports::attached_deposit(data.as_ptr() as u64);
        u128::from_le_bytes(data)
    }
}

pub fn assert_one_yocto() {
    assert_eq!(
        attached_deposit(),
        1,
        "Requires attached deposit of exactly 1 yoctoNEAR"
    )
}

pub fn promise_batch_action_transfer(promise_index: u64, amount: u128) {
    unsafe {
        exports::promise_batch_action_transfer(promise_index, &amount as *const u128 as _);
    }
}

pub fn storage_byte_cost() -> u128 {
    STORAGE_PRICE_PER_BYTE
}

pub fn promise_batch_create(account_id: String) -> u64 {
    unsafe { exports::promise_batch_create(account_id.len() as _, account_id.as_ptr() as _) }
}

pub fn storage_has_key(key: &str) -> bool {
    unsafe { exports::storage_has_key(key.len() as u64, key.as_ptr() as u64) == 1 }
}
