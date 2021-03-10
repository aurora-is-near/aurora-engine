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
    use crate::types::{near_account_to_evm_address, u256_to_arr, GetStorageAtArgs, ViewCallArgs};
    use borsh::BorshDeserialize;
    use evm::ExitReason;
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
        let version = match option_env!("NEAR_EVM_VERSION") {
          Some(v) => v.as_bytes(),
          None => include_bytes!("../VERSION")
        };
        sdk::return_output(version)
    }

    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let input = sdk::read_input();
        let mut engine = Engine::new(CHAIN_ID, predecessor_address());
        let (reason, return_value) = Engine::deploy_code(&mut engine, &input);
        // TODO: charge for storage
        process_exit_reason(reason, &return_value.0)
    }

    #[no_mangle]
    pub extern "C" fn call() {
        let input = sdk::read_input();
        let mut engine = Engine::new(CHAIN_ID, predecessor_address());
        let (reason, return_value) = Engine::call(&mut engine, &input);
        // TODO: charge for storage
        process_exit_reason(reason, &return_value)
    }

    #[no_mangle]
    pub extern "C" fn raw_call() {
        let _input = sdk::read_input();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/3
        sdk::return_output(&[])
    }

    #[no_mangle]
    pub extern "C" fn meta_call() {
        let _input = sdk::read_input();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/4
        sdk::return_output(&[])
    }

    #[no_mangle]
    pub extern "C" fn view() {
        let input = sdk::read_input();
        let args = ViewCallArgs::try_from_slice(&input).unwrap();
        let mut engine = Engine::new(CHAIN_ID, H160::from_slice(&args.sender));
        let (reason, return_value) = Engine::view(&mut engine, args);
        process_exit_reason(reason, &return_value)
    }

    #[no_mangle]
    pub extern "C" fn get_code() {
        let address = sdk::read_input_arr20();
        let code = Engine::get_code(&H160(address));
        sdk::return_output(&code)
    }

    #[no_mangle]
    pub extern "C" fn get_balance() {
        let address = sdk::read_input_arr20();
        let balance = Engine::get_balance(&H160(address));
        sdk::return_output(&u256_to_arr(&balance))
    }

    #[no_mangle]
    pub extern "C" fn get_nonce() {
        let address = sdk::read_input_arr20();
        let nonce = Engine::get_nonce(&H160(address));
        sdk::return_output(&u256_to_arr(&nonce))
    }

    #[no_mangle]
    pub extern "C" fn get_storage_at() {
        let input = sdk::read_input();
        let args = GetStorageAtArgs::try_from_slice(&input).unwrap();
        let value = Engine::get_storage(&H160(args.address), &H256(args.key));
        sdk::return_output(&value.0)
    }

    #[no_mangle]
    pub extern "C" fn begin_chain() {
        let _input = sdk::read_input();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/1
        sdk::return_output(&[])
    }

    #[no_mangle]
    pub extern "C" fn begin_block() {
        let _input = sdk::read_input();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/2
        sdk::return_output(&[])
    }

    fn predecessor_address() -> H160 {
        near_account_to_evm_address(&sdk::predecessor_account_id())
    }

    fn process_exit_reason(reason: ExitReason, return_value: &[u8]) {
        match reason {
            ExitReason::Succeed(_) => sdk::return_output(return_value),
            ExitReason::Revert(_) => sdk::panic_hex(&return_value),
            ExitReason::Error(error) => sdk::panic_utf8(b"error"), // TODO
            ExitReason::Fatal(error) => sdk::panic_utf8(b"fatal error"), // TODO
        }
    }
}
