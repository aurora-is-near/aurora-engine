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
    use crate::parameters::{
        BeginBlockArgs, BeginChainArgs, FunctionCallArgs, GetStorageAtArgs, ViewCallArgs,
    };
    use crate::prelude::{vec, Address, H256, U256};
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
        let (status, result) = Engine::deploy_code_with_input(&mut engine, &input);
        // TODO: charge for storage
        process_exit_reason(status, &result.0)
    }

    #[no_mangle]
    pub extern "C" fn call() {
        let input = sdk::read_input();
        let args = FunctionCallArgs::try_from_slice(&input).unwrap();
        let mut engine = Engine::new(*CHAIN_ID, predecessor_address());
        let (status, result) = Engine::call_with_args(&mut engine, args);
        // TODO: charge for storage
        process_exit_reason(status, &result)
    }

    #[no_mangle]
    pub extern "C" fn raw_call() {
        use crate::transaction::EthSignedTransaction;
        use rlp::{Decodable, Rlp};

        let input = sdk::read_input();
        let signed_transaction = EthSignedTransaction::decode(&Rlp::new(&input))
            .or_else(|_| Err(sdk::panic_utf8(b"invalid transaction")))
            .unwrap();

        // Validate the chain ID, if provided inside the signature:
        if let Some(chain_id) = signed_transaction.chain_id() {
            if U256::from(chain_id) != *CHAIN_ID {
                return sdk::panic_utf8(b"invalid chain ID");
            }
        }

        // Retrieve the signer of the transaction:
        let sender = match signed_transaction.sender() {
            Some(sender) => sender,
            None => return sdk::panic_utf8(b"invalid ECDSA signature"),
        };

        // Figure out what kind of a transaction this is, and execute it:
        let mut engine = Engine::new(*CHAIN_ID, predecessor_address());
        let value = signed_transaction.transaction.value;
        let data = signed_transaction.transaction.data;
        if let Some(receiver) = signed_transaction.transaction.to {
            let (status, result) = if data.is_empty() {
                // Execute a balance transfer:
                (
                    Engine::transfer(&mut engine, sender, receiver, value),
                    vec![],
                )
            } else {
                // Execute a contract call:
                Engine::call(&mut engine, sender, receiver, value, data)
                // TODO: charge for storage
            };
            process_exit_reason(status, &result)
        } else {
            // Execute a contract deployment:
            let (status, result) = Engine::deploy_code(&mut engine, sender, value, &data);
            // TODO: charge for storage
            process_exit_reason(status, &result.0)
        }
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
        let (status, result) = Engine::view_with_args(&mut engine, args);
        process_exit_reason(status, &result)
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

    fn process_exit_reason(status: ExitReason, result: &[u8]) {
        match status {
            ExitReason::Succeed(_) => sdk::return_output(result),
            ExitReason::Revert(_) => sdk::panic_hex(&result),
            ExitReason::Error(_error) => sdk::panic_utf8(b"error"), // TODO
            ExitReason::Fatal(_error) => sdk::panic_utf8(b"fatal error"), // TODO
        }
    }
}
