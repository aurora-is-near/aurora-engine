use core::ffi;
use std::slice;

use super::state::{State, STATE};

// #############
// # Registers #
// #############

#[unsafe(no_mangle)]
extern "C" fn read_register(register_id: u64, ptr: u64) {
    STATE.with_borrow(|state| state.read_register(register_id, ptr));
}

#[unsafe(no_mangle)]
extern "C" fn register_len(register_id: u64) -> u64 {
    STATE.with_borrow(|state| state.register_len(register_id))
}

// ###############
// # Context API #
// ###############

#[unsafe(no_mangle)]
extern "C" fn current_account_id(register_id: u64) {
    STATE.with_borrow(|state| state.current_account_id(register_id));
}

#[unsafe(no_mangle)]
extern "C" fn signer_account_id(register_id: u64) {
    STATE.with_borrow(|state| state.signer_account_id(register_id));
}

#[unsafe(no_mangle)]
pub extern "C" fn signer_account_pk(register_id: u64) {
    let _ = register_id;
    unimplemented!();
}

#[unsafe(no_mangle)]
extern "C" fn predecessor_account_id(register_id: u64) {
    STATE.with_borrow(|state| state.predecessor_account_id(register_id));
}

#[unsafe(no_mangle)]
extern "C" fn input(register_id: u64) {
    STATE.with_borrow(|state| state.input(register_id));
}

#[unsafe(no_mangle)]
extern "C" fn block_index() -> u64 {
    STATE.with_borrow(State::block_index)
}

#[unsafe(no_mangle)]
extern "C" fn block_timestamp() -> u64 {
    STATE.with_borrow(State::block_timestamp)
}

#[unsafe(no_mangle)]
pub extern "C" fn epoch_height() -> u64 {
    unimplemented!()
}

#[unsafe(no_mangle)]
pub extern "C" fn storage_usage() -> u64 {
    unimplemented!()
}

// #################
// # Economics API #
// #################

#[unsafe(no_mangle)]
pub extern "C" fn account_balance(balance_ptr: u64) {
    let _ = balance_ptr;
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn attached_deposit(balance_ptr: u64) {
    STATE.with_borrow(|state| state.attached_deposit(balance_ptr));
}

#[unsafe(no_mangle)]
extern "C" fn prepaid_gas() -> u64 {
    STATE.with_borrow(State::prepaid_gas)
}

#[unsafe(no_mangle)]
extern "C" fn used_gas() -> u64 {
    STATE.with_borrow(State::used_gas)
}

// ############
// # Math API #
// ############

#[unsafe(no_mangle)]
extern "C" fn random_seed(register_id: u64) {
    STATE.with_borrow(|state| state.random_seed(register_id));
}

#[unsafe(no_mangle)]
extern "C" fn sha256(value_len: u64, value_ptr: u64, register_id: u64) {
    STATE.with_borrow(|state| {
        state.digest::<sha2::Sha256>(value_len, value_ptr, register_id);
    });
}

#[unsafe(no_mangle)]
extern "C" fn keccak256(value_len: u64, value_ptr: u64, register_id: u64) {
    STATE.with_borrow(|state| {
        state.digest::<sha3::Keccak256>(value_len, value_ptr, register_id);
    });
}

#[unsafe(no_mangle)]
extern "C" fn ripemd160(value_len: u64, value_ptr: u64, register_id: u64) {
    STATE.with_borrow(|state| {
        state.digest::<ripemd::Ripemd160>(value_len, value_ptr, register_id);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ecrecover(
    hash_len: u64,
    hash_ptr: u64,
    sig_len: u64,
    sig_ptr: u64,
    v: u64,
    malleability_flag: u64,
    register_id: u64,
) -> u64 {
    if malleability_flag == 0 {
        STATE.with_borrow(|state| {
            state
                .ecrecover(hash_len, hash_ptr, sig_len, sig_ptr, v, register_id)
                .is_ok()
                .into()
        })
    } else {
        unimplemented!()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alt_bn128_g1_sum(value_len: u64, value_ptr: u64, register_id: u64) {
    let _ = (value_len, value_ptr, register_id);
    unimplemented!()
}

#[unsafe(no_mangle)]
pub extern "C" fn alt_bn128_g1_multiexp(value_len: u64, value_ptr: u64, register_id: u64) {
    let _ = (value_len, value_ptr, register_id);
    unimplemented!()
}

#[unsafe(no_mangle)]
pub extern "C" fn alt_bn128_pairing_check(value_len: u64, value_ptr: u64) {
    let _ = (value_len, value_ptr);
    unimplemented!()
}

// #####################
// # Miscellaneous API #
// #####################

#[unsafe(no_mangle)]
extern "C" fn value_return(value_len: u64, value_ptr: u64) {
    STATE.with_borrow(|state| state.value_return(value_len, value_ptr));
}

#[unsafe(no_mangle)]
pub const extern "C" fn panic() {
    panic!()
}

#[unsafe(no_mangle)]
pub extern "C" fn panic_utf8(len: u64, ptr: *const ffi::c_char) {
    let len = usize::try_from(len).expect("pointer size must be wide enough");
    let str =
        unsafe { std::str::from_utf8_unchecked(slice::from_raw_parts(ptr.cast::<u8>(), len)) };
    panic!("{str}");
}

#[unsafe(no_mangle)]
pub extern "C" fn log_utf8(len: u64, ptr: *const ffi::c_char) {
    let len = usize::try_from(len).expect("pointer size must be wide enough");
    let str =
        unsafe { std::str::from_utf8_unchecked(slice::from_raw_parts(ptr.cast::<u8>(), len)) };
    println!("{str}");
}

#[unsafe(no_mangle)]
pub const extern "C" fn log_utf16(len: u64, ptr: u64) {
    let _ = (len, ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
pub const extern "C" fn abort(msg_ptr: u32, filename_ptr: u32, line: u32, col: u32) {
    let _ = (msg_ptr, filename_ptr, line, col);
}

// ################
// # Promises API #
// ################

#[unsafe(no_mangle)]
pub const extern "C" fn promise_create(
    account_id_len: u64,
    account_id_ptr: u64,
    method_name_len: u64,
    method_name_ptr: u64,
    arguments_len: u64,
    arguments_ptr: u64,
    amount_ptr: u64,
    gas: u64,
) -> u64 {
    let _ = (account_id_len, account_id_ptr);
    let _ = (method_name_len, method_name_ptr);
    let _ = (arguments_len, arguments_ptr);
    let _ = (amount_ptr, gas);
    // TODO:
    0
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_then(
    promise_index: u64,
    account_id_len: u64,
    account_id_ptr: u64,
    method_name_len: u64,
    method_name_ptr: u64,
    arguments_len: u64,
    arguments_ptr: u64,
    amount_ptr: u64,
    gas: u64,
) -> u64 {
    let _ = promise_index;
    let _ = (account_id_len, account_id_ptr);
    let _ = (method_name_len, method_name_ptr);
    let _ = (arguments_len, arguments_ptr);
    let _ = (amount_ptr, gas);
    // TODO:
    0
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_and(promise_idx_ptr: u64, promise_idx_count: u64) -> u64 {
    let _ = (promise_idx_ptr, promise_idx_count);
    // TODO:
    0
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64 {
    let _ = (account_id_len, account_id_ptr);
    // TODO:
    0
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_then(
    promise_index: u64,
    account_id_len: u64,
    account_id_ptr: u64,
) -> u64 {
    let _ = promise_index;
    let _ = (account_id_len, account_id_ptr);
    // TODO:
    0
}

// #######################
// # Promise API actions #
// #######################

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_create_account(promise_index: u64) {
    let _ = promise_index;
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_deploy_contract(
    promise_index: u64,
    code_len: u64,
    code_ptr: u64,
) {
    let _ = promise_index;
    let _ = (code_len, code_ptr);
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_function_call(
    promise_index: u64,
    method_name_len: u64,
    method_name_ptr: u64,
    arguments_len: u64,
    arguments_ptr: u64,
    amount_ptr: u64,
    gas: u64,
) {
    let _ = promise_index;
    let _ = (method_name_len, method_name_ptr);
    let _ = (arguments_len, arguments_ptr);
    let _ = (amount_ptr, gas);
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_transfer(promise_index: u64, amount_ptr: u64) {
    let _ = promise_index;
    let _ = amount_ptr;
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_stake(
    promise_index: u64,
    amount_ptr: u64,
    public_key_len: u64,
    public_key_ptr: u64,
) {
    let _ = promise_index;
    let _ = amount_ptr;
    let _ = (public_key_len, public_key_ptr);
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_add_key_with_full_access(
    promise_index: u64,
    public_key_len: u64,
    public_key_ptr: u64,
    nonce: u64,
) {
    let _ = promise_index;
    let _ = (public_key_len, public_key_ptr);
    let _ = nonce;
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_add_key_with_function_call(
    promise_index: u64,
    public_key_len: u64,
    public_key_ptr: u64,
    nonce: u64,
    allowance_ptr: u64,
    receiver_id_len: u64,
    receiver_id_ptr: u64,
    method_names_len: u64,
    method_names_ptr: u64,
) {
    let _ = promise_index;
    let _ = (public_key_len, public_key_ptr);
    let _ = nonce;
    let _ = allowance_ptr;
    let _ = (receiver_id_len, receiver_id_ptr);
    let _ = (method_names_len, method_names_ptr);
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_delete_key(
    promise_index: u64,
    public_key_len: u64,
    public_key_ptr: u64,
) {
    let _ = promise_index;
    let _ = (public_key_len, public_key_ptr);
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_batch_action_delete_account(
    promise_index: u64,
    beneficiary_id_len: u64,
    beneficiary_id_ptr: u64,
) {
    let _ = promise_index;
    let _ = (beneficiary_id_len, beneficiary_id_ptr);
}

// #######################
// # Promise API results #
// #######################

#[unsafe(no_mangle)]
extern "C" fn promise_results_count() -> u64 {
    STATE.with_borrow(State::promise_results_count)
}

#[unsafe(no_mangle)]
extern "C" fn promise_result(result_idx: u64, register_id: u64) -> u64 {
    STATE.with_borrow(|state| state.promise_result(result_idx, register_id))
}

#[unsafe(no_mangle)]
pub const extern "C" fn promise_return(promise_id: u64) {
    let _ = promise_id;
    // unimplemented!()
}

// ###############
// # Storage API #
// ###############

#[unsafe(no_mangle)]
pub extern "C" fn storage_write(
    key_len: u64,
    key_ptr: u64,
    value_len: u64,
    value_ptr: u64,
    register_id: u64,
) -> u64 {
    STATE.with_borrow(|state| {
        state.storage_write(key_len, key_ptr, value_len, value_ptr, register_id)
    })
}

#[unsafe(no_mangle)]
extern "C" fn storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
    STATE.with_borrow(|state| state.storage_read(key_len, key_ptr, register_id))
}

#[unsafe(no_mangle)]
extern "C" fn storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
    STATE.with_borrow(|state| state.storage_remove(key_len, key_ptr, register_id))
}

#[unsafe(no_mangle)]
extern "C" fn storage_has_key(key_len: u64, key_ptr: u64) -> u64 {
    STATE.with_borrow(|state| state.storage_has_key(key_len, key_ptr))
}

#[unsafe(no_mangle)]
pub extern "C" fn storage_iter_prefix(prefix_len: u64, prefix_ptr: u64) -> u64 {
    let _ = (prefix_len, prefix_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
pub extern "C" fn storage_iter_range(
    start_len: u64,
    start_ptr: u64,
    end_len: u64,
    end_ptr: u64,
) -> u64 {
    let _ = (start_len, start_ptr);
    let _ = (end_len, end_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
pub extern "C" fn storage_iter_next(
    iterator_id: u64,
    key_register_id: u64,
    value_register_id: u64,
) -> u64 {
    let _ = (iterator_id, key_register_id, value_register_id);
    unimplemented!()
}

// ###############
// # Validator API #
// ###############

#[unsafe(no_mangle)]
pub extern "C" fn validator_stake(account_id_len: u64, account_id_ptr: u64, stake_ptr: u64) {
    let _ = (account_id_len, account_id_ptr, stake_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
pub extern "C" fn validator_total_stake(stake_ptr: u64) {
    let _ = stake_ptr;
    unimplemented!()
}
