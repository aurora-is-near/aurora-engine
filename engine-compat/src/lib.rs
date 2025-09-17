#![no_std]

extern crate alloc;

/// The contract must compile with feature `contract`. This code is only for clippy and udeps.
#[cfg(not(feature = "contract"))]
pub mod noop_allocator {
    use core::alloc::{GlobalAlloc, Layout};

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

    #[cfg(not(test))]
    #[panic_handler]
    const fn on_panic(info: &core::panic::PanicInfo) -> ! {
        let _ = info;
        loop {}
    }
}

#[cfg(feature = "contract")]
mod dbg {
    use alloc::format;
    use aurora_engine_sdk::{
        io::{StorageIntermediate, IO},
        near_runtime::Runtime as Rt,
    };
    use borsh::{BorshDeserialize, BorshSerialize};
    use engine_standalone_tracing::{
        sputnik::{self, TransactionTraceBuilder},
        types::call_tracer::CallTracer,
        TraceKind,
    };

    pub struct Runtime;

    impl Runtime {
        pub fn read<A>() -> Option<A>
        where
            A: BorshDeserialize,
        {
            match Rt
                .read_storage(b"borealis/argument")
                .map(|v| v.to_value())
                .transpose()
            {
                Ok(v) => v,
                Err(err) => aurora_engine_sdk::panic_utf8(err.as_ref()),
            }
        }

        pub fn write<R>(key: &[u8], response: R)
        where
            R: BorshSerialize,
        {
            match borsh::to_vec(&response) {
                Ok(bytes) => drop(Rt.write_storage(key, &bytes)),
                Err(err) => {
                    let msg = format!("write response: {err}").into_bytes();
                    aurora_engine_sdk::panic_utf8(&msg);
                }
            }
        }

        pub fn trace<F, R>(f: F) -> R
        where
            F: FnOnce() -> R,
        {
            match Self::read::<TraceKind>() {
                None => f(),
                Some(TraceKind::CallFrame) => {
                    let mut listener = CallTracer::default();
                    let r = sputnik::traced_call(&mut listener, f);
                    Self::write(b"borealis/call_frame_tracing", listener);
                    r
                }
                Some(TraceKind::Transaction) => {
                    let mut listener = TransactionTraceBuilder::default();
                    let r = sputnik::traced_call(&mut listener, f);
                    let trace_log = listener.finish().logs().clone();
                    Self::write(b"borealis/transaction_tracing", trace_log);
                    r
                }
            }
        }
    }
}

#[cfg(feature = "contract")]
mod contract {
    use aurora_engine::contract_methods::{self, ContractError};
    use aurora_engine_sdk::{near_runtime::Runtime, types::SdkUnwrap};

    use super::dbg;

    #[no_mangle]
    extern "C" fn borealis_wrapper_submit() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        dbg::Runtime::trace(|| {
            contract_methods::evm_transactions::submit(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap()
        });
    }

    #[no_mangle]
    pub extern "C" fn borealis_wrapper_call() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        dbg::Runtime::trace(|| {
            contract_methods::evm_transactions::call(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap();
        });
    }

    #[no_mangle]
    pub extern "C" fn borealis_wrapper_ft_on_transfer() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        let result = dbg::Runtime::trace(|| {
            contract_methods::connector::ft_on_transfer(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap()
        });
        dbg::Runtime::write(b"borealis/submit_result", result);
    }

    #[no_mangle]
    pub extern "C" fn borealis_wrapper_exit_to_near_precompile_callback() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        let maybe_result = dbg::Runtime::trace(|| {
            contract_methods::connector::exit_to_near_precompile_callback(io, &env, &mut handler)
                .map_err(ContractError::msg)
                .sdk_unwrap();
        });
        dbg::Runtime::write(b"borealis/submit_result", maybe_result);
    }
}
