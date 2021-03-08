#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(core_intrinsics))]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

#[cfg(feature = "contract")]
mod contract {
    #[no_mangle]
    pub extern "C" fn deploy_code() {}

    #[no_mangle]
    pub extern "C" fn call() {}

    #[no_mangle]
    pub extern "C" fn raw_call() {}

    #[no_mangle]
    pub extern "C" fn meta_call() {}

    #[no_mangle]
    pub extern "C" fn view() {}

    #[no_mangle]
    pub extern "C" fn get_code() {}

    #[no_mangle]
    pub extern "C" fn get_balance() {}

    #[no_mangle]
    pub extern "C" fn get_nonce() {}

    #[no_mangle]
    pub extern "C" fn get_storage_at() {}
}
