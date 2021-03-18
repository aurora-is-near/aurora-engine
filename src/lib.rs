#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(core_intrinsics))]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

mod parameters;
mod precompiles;
mod prelude;
mod storage;
mod transaction;
mod types;

#[cfg(feature = "contract")]
mod engine;
#[cfg(feature = "contract")]
mod sdk;

#[cfg(feature = "contract")]
mod contract {
    use crate::engine::Engine;
    use crate::parameters::{BeginBlockArgs, BeginChainArgs, GetStorageAtArgs, ViewCallArgs};
    use crate::prelude::{Address, H256, U256};
    use crate::sdk;
    use crate::types::{near_account_to_evm_address, u256_to_arr};
    use borsh::BorshDeserialize;
    use evm::ExitReason;
    use lazy_static::lazy_static;

    #[global_allocator]
    static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

    lazy_static! {
        static ref CHAIN_ID: U256 = match sdk::read_storage(b"\0chain_id") {
            Some(v) => U256::from_big_endian(v.as_slice()),
            None => match option_env!("NEAR_EVM_CHAIN") {
                Some(v) => U256::from_dec_str(v).unwrap_or_else(|_| U256::zero()),
                None => U256::from(1313161556), // NEAR BetaNet
            },
        };
    }

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
            None => include_bytes!("../VERSION"),
        };
        sdk::return_output(version)
    }

    #[no_mangle]
    pub extern "C" fn get_chain_id() {
        let mut result = [0u8; 32];
        (*CHAIN_ID).to_big_endian(&mut result);
        sdk::return_output(&result)
    }

    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let input = sdk::read_input();
        let mut engine = Engine::new(*CHAIN_ID, predecessor_address());
        let (reason, return_value) = Engine::deploy_code(&mut engine, &input);
        // TODO: charge for storage
        process_exit_reason(reason, &return_value.0)
    }

    #[no_mangle]
    pub extern "C" fn call() {
        let input = sdk::read_input();
        let mut engine = Engine::new(*CHAIN_ID, predecessor_address());
        let (reason, return_value) = Engine::call(&mut engine, &input);
        // TODO: charge for storage
        process_exit_reason(reason, &return_value)
    }

    #[no_mangle]
    pub extern "C" fn raw_call() {
        let _input = sdk::read_input();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/3
    }

    #[no_mangle]
    pub extern "C" fn meta_call() {
        let _input = sdk::read_input();
        todo!(); // TODO: https://github.com/aurora-is-near/aurora-engine/issues/4
    }

    #[no_mangle]
    pub extern "C" fn view() {
        let input = sdk::read_input();
        let args = ViewCallArgs::try_from_slice(&input).unwrap();
        let mut engine = Engine::new(*CHAIN_ID, Address::from_slice(&args.sender));
        let (reason, return_value) = Engine::view(&mut engine, args);
        process_exit_reason(reason, &return_value)
    }

    #[no_mangle]
    pub extern "C" fn get_code() {
        let address = sdk::read_input_arr20();
        let code = Engine::get_code(&Address(address));
        sdk::return_output(&code)
    }

    #[no_mangle]
    pub extern "C" fn get_balance() {
        let address = sdk::read_input_arr20();
        let balance = Engine::get_balance(&Address(address));
        sdk::return_output(&u256_to_arr(&balance))
    }

    #[no_mangle]
    pub extern "C" fn get_nonce() {
        let address = sdk::read_input_arr20();
        let nonce = Engine::get_nonce(&Address(address));
        sdk::return_output(&u256_to_arr(&nonce))
    }

    #[no_mangle]
    pub extern "C" fn get_storage_at() {
        let input = sdk::read_input();
        let args = GetStorageAtArgs::try_from_slice(&input).unwrap();
        let value = Engine::get_storage(&Address(args.address), &H256(args.key));
        sdk::return_output(&value.0)
    }

    #[no_mangle]
    pub extern "C" fn begin_chain() {
        let input = sdk::read_input();
        let args = BeginChainArgs::try_from_slice(&input).unwrap();
        sdk::write_storage(b"\0chain_id", &args.chain_id)
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/1
    }

    #[no_mangle]
    pub extern "C" fn begin_block() {
        let input = sdk::read_input();
        let _args = BeginBlockArgs::try_from_slice(&input).unwrap();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/2
    }

    fn predecessor_address() -> Address {
        near_account_to_evm_address(&sdk::predecessor_account_id())
    }

    fn process_exit_reason(reason: ExitReason, return_value: &[u8]) {
        match reason {
            ExitReason::Succeed(_) => sdk::return_output(return_value),
            ExitReason::Revert(_) => sdk::panic_hex(&return_value),
            ExitReason::Error(_error) => sdk::panic_utf8(b"error"), // TODO
            ExitReason::Fatal(_error) => sdk::panic_utf8(b"fatal error"), // TODO
        }
    }
}
