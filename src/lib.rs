#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(core_intrinsics))]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

pub mod types;

#[cfg(feature = "contract")]
mod engine;
#[cfg(feature = "contract")]
mod sdk;

#[cfg(feature = "contract")]
mod contract {
    use crate::engine::Engine;
    use crate::sdk;
    use crate::types::{near_account_to_evm_address, GetStorageAtArgs, ViewCallArgs};
    use borsh::BorshDeserialize;
    use primitive_types::{H160, H256};

    const CHAIN_ID: u64 = 1313161556; // FIXME

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
        sdk::return_output("0.0.0".as_bytes())
    }

    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let _input = sdk::read_input();
        let mut _backend = Engine::new(CHAIN_ID, predecessor_address());
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn call() {
        let _input = sdk::read_input();
        let mut _backend = Engine::new(CHAIN_ID, predecessor_address());
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn raw_call() {
        let _input = sdk::read_input();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn meta_call() {
        let _input = sdk::read_input();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn view() {
        let input = sdk::read_input();
        let args = ViewCallArgs::try_from_slice(&input).unwrap();
        let mut _backend = Engine::new(CHAIN_ID, H160::from_slice(&args.sender));
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn get_code() {
        let _address = sdk::read_input_arr20();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn get_balance() {
        let _address = sdk::read_input_arr20();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn get_nonce() {
        let _address = sdk::read_input_arr20();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn get_storage_at() {
        let input = sdk::read_input();
        let _args = GetStorageAtArgs::try_from_slice(&input).unwrap();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn begin_chain() {
        let _input = sdk::read_input();
        // TODO
        sdk::return_output(&[]);
    }

    #[no_mangle]
    pub extern "C" fn begin_block() {
        let _input = sdk::read_input();
        // TODO
        sdk::return_output(&[]);
    }

    fn predecessor_address() -> H160 {
        near_account_to_evm_address(&sdk::predecessor_account_id())
    }
}
