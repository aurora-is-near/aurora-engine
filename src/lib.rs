#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(core_intrinsics))]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

pub mod types;

#[cfg(feature = "contract")]
mod sdk;

#[cfg(feature = "contract")]
mod contract {
    #[global_allocator]
    static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

    #[panic_handler]
    #[no_mangle]
    pub unsafe fn on_panic(_info: &::core::panic::PanicInfo) -> ! {
        ::core::intrinsics::abort();
    }

    #[alloc_error_handler]
    #[no_mangle]
    pub unsafe fn on_alloc_error(_: core::alloc::Layout) -> ! {
        ::core::intrinsics::abort();
    }

    #[no_mangle]
    pub extern "C" fn get_version() {
        crate::sdk::return_output("0.0.0".as_bytes())
    }

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
