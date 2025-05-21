use core::ffi;

use aurora_engine::contract_methods;
use aurora_engine_sdk::near_runtime::Runtime;

/// ADMINISTRATIVE METHODS
/// Sets the configuration for the Engine.
/// Should be called on deployment.
#[no_mangle]
pub extern "C" fn _native_new() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::new(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Get a version of the contract.
#[no_mangle]
pub extern "C" fn _native_get_version() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_version(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Get owner account id for this contract.
#[no_mangle]
pub extern "C" fn _native_get_owner() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_owner(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Set owner account id for this contract.
#[no_mangle]
pub extern "C" fn _native_set_owner() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::set_owner(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Get chain id for this contract.
#[no_mangle]
pub extern "C" fn _native_get_chain_id() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_chain_id(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_upgrade_delay_blocks() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_upgrade_delay_blocks(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_set_upgrade_delay_blocks() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::set_upgrade_delay_blocks(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_upgrade_index() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_upgrade_index(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Upgrade the contract with the provided code bytes.
#[no_mangle]
pub extern "C" fn _native_upgrade() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;

    let result = contract_methods::admin::upgrade(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Stage new code for deployment.
#[no_mangle]
pub extern "C" fn _native_stage_upgrade() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::stage_upgrade(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Called as part of the upgrade process (see `engine-sdk::self_deploy`). This function is meant
/// to make any necessary changes to the state such that it aligns with the newly deployed
/// code.
#[no_mangle]
#[allow(clippy::missing_const_for_fn)]
pub extern "C" fn _native_state_migration() -> *mut ffi::c_void {
    // TODO: currently we don't have migrations
    core::ptr::null_mut()
}

/// Resumes previously [`paused`] precompiles.
///
/// [`paused`]: pause_precompiles
#[no_mangle]
pub extern "C" fn _native_resume_precompiles() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::resume_precompiles(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Pauses a precompile.
#[no_mangle]
pub extern "C" fn _native_pause_precompiles() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::pause_precompiles(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Returns an unsigned integer where each bit set to 1 means that corresponding precompile
/// to that bit is paused and 0-bit means not paused.
#[no_mangle]
pub extern "C" fn _native_paused_precompiles() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::paused_precompiles(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Sets the flag to pause the contract.
#[no_mangle]
pub extern "C" fn _native_pause_contract() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::pause_contract(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Sets the flag to resume the contract.
#[no_mangle]
pub extern "C" fn _native_resume_contract() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::resume_contract(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// MUTATIVE METHODS
/// Deploy code into the EVM.
#[no_mangle]
pub extern "C" fn _native_deploy_code() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::deploy_code(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Call method on the EVM contract.
#[no_mangle]
pub extern "C" fn _native_call() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::call(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Process signed Ethereum transaction.
/// Must match `CHAIN_ID` to make sure it's signed for given chain vs replayed from another chain.
#[no_mangle]
pub extern "C" fn _native_submit() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::submit(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Analog of the `submit` function, but waits for the `SubmitArgs` structure rather than
/// the array of bytes representing the transaction.
#[no_mangle]
pub extern "C" fn _native_submit_with_args() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::submit_with_args(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}
