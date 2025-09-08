#![no_std]

extern crate alloc;

#[cfg(all(
    not(feature = "contract_3_9_2"),
    not(feature = "contract_3_7_0"),
    not(feature = "contract_3_9_0")
))]
mod allocator {
    use core::alloc::{GlobalAlloc, Layout};

    #[panic_handler]
    #[cfg(not(test))]
    pub const fn on_panic(_info: &core::panic::PanicInfo) -> ! {
        loop {}
    }

    pub struct Allocator;

    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
            core::ptr::null_mut()
        }

        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
            unreachable!("should never be called, since alloc never returns non-null");
        }
    }

    #[global_allocator]
    static ALLOC: Allocator = Allocator;
}

#[cfg(feature = "contract_3_7_0")]
pub use aurora_engine_3_7_0 as aurora_engine;

#[cfg(feature = "contract_3_7_0")]
pub use aurora_engine_sdk_3_7_0 as aurora_engine_sdk;

#[cfg(feature = "contract_3_9_0")]
pub use aurora_engine_3_9_0 as aurora_engine;

#[cfg(feature = "contract_3_9_0")]
pub use aurora_engine_sdk_3_9_0 as aurora_engine_sdk;

#[cfg(feature = "contract_3_9_2")]
pub use aurora_engine_3_9_2 as aurora_engine;

#[cfg(feature = "contract_3_9_2")]
pub use aurora_engine_sdk_3_9_2 as aurora_engine_sdk;

#[cfg(any(
    feature = "contract_3_9_2",
    feature = "contract_3_7_0",
    feature = "contract_3_9_0"
))]
mod contract {
    use super::aurora_engine::{
        contract_methods::{self, ContractError},
        errors,
    };
    use super::aurora_engine_sdk::{io::IO, near_runtime::Runtime, types::SdkUnwrap};

    use engine_standalone_tracing::{sputnik, types::call_tracer::CallTracer};

    #[no_mangle]
    extern "C" fn submit_trace_tx() {
        let mut io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        let mut listener = sputnik::TransactionTraceBuilder::default();
        let submit_result = sputnik::traced_call(&mut listener, || {
            contract_methods::evm_transactions::submit(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap()
        });
        let trace_log = listener.finish().logs().clone();
        let result_bytes = borsh::to_vec(&(submit_result, trace_log))
            .map_err(|_| errors::ERR_SERIALIZE)
            .sdk_unwrap();
        io.return_output(&result_bytes);
    }

    #[no_mangle]
    extern "C" fn submit_trace_call() {
        let mut io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        let mut listener = CallTracer::default();
        let submit_result = sputnik::traced_call(&mut listener, || {
            contract_methods::evm_transactions::submit(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap()
        });
        let result_bytes = borsh::to_vec(&(submit_result, listener))
            .map_err(|_| errors::ERR_SERIALIZE)
            .sdk_unwrap();
        io.return_output(&result_bytes);
    }

    #[no_mangle]
    pub extern "C" fn borealis_wrapper_ft_on_transfer() {
        let mut io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        let result = contract_methods::connector::ft_on_transfer(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
        let result_bytes = borsh::to_vec(&result)
            .map_err(|_| errors::ERR_SERIALIZE)
            .sdk_unwrap();
        io.return_output(&result_bytes);
    }

    #[no_mangle]
    pub extern "C" fn borealis_wrapper_exit_to_near_precompile_callback() {
        let mut io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        let maybe_result =
            contract_methods::connector::exit_to_near_precompile_callback(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap();
        let result_bytes = borsh::to_vec(&maybe_result)
            .map_err(|_| errors::ERR_SERIALIZE)
            .sdk_unwrap();
        io.return_output(&result_bytes);
    }
}
