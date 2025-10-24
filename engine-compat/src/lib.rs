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
        pub fn read<A>(key: &[u8]) -> Option<A>
        where
            A: BorshDeserialize,
        {
            match Rt.read_storage(key).map(|v| v.to_value()).transpose() {
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
            match Self::read(b"borealis/trace_kind") {
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
mod simulate;

#[cfg(feature = "contract")]
mod contract {
    use alloc::{boxed::Box, format, string::String, vec::Vec};

    use aurora_engine::contract_methods::{self, ContractError};
    use aurora_engine_sdk::{io::IO, near_runtime::Runtime, types::SdkUnwrap};
    use aurora_engine_types::parameters::{engine::TransactionExecutionResult, silo::FixedGasArgs};

    use super::dbg;

    #[no_mangle]
    #[allow(clippy::too_many_lines)]
    pub extern "C" fn execute() {
        let mut io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;

        let Some(method) = dbg::Runtime::read::<String>(b"borealis/method") else {
            return;
        };

        let out = dbg::Runtime::trace(|| match method.as_str() {
            "submit" => contract_methods::evm_transactions::submit(io, &env, &mut handler)
                .map(TransactionExecutionResult::Submit)
                .map(Some),
            "submit_with_args" => {
                contract_methods::evm_transactions::submit_with_args(io, &env, &mut handler)
                    .map(TransactionExecutionResult::Submit)
                    .map(Some)
            }
            "call" => contract_methods::evm_transactions::call(io, &env, &mut handler)
                .map(TransactionExecutionResult::Submit)
                .map(Some),
            "deploy_code" => {
                contract_methods::evm_transactions::deploy_code(io, &env, &mut handler)
                    .map(TransactionExecutionResult::Submit)
                    .map(Some)
            }
            "deploy_erc20_token" => {
                contract_methods::connector::deploy_erc20_token(io, &env, &mut handler)
                    .map(TransactionExecutionResult::DeployErc20)
                    .map(Some)
            }
            "ft_on_transfer" => contract_methods::connector::ft_on_transfer(io, &env, &mut handler)
                .map(|maybe_output| maybe_output.map(TransactionExecutionResult::Submit)),
            "ft_transfer_call" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::ft_transfer_call(io, &env, &mut handler)
                    .map(|maybe_output| maybe_output.map(TransactionExecutionResult::Promise)),
            },
            "ft_resolve_transfer" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::ft_resolve_transfer(io, &env, &mut handler)
                    .map(|()| None),
            },
            "ft_transfer" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::ft_transfer(io, &env).map(|()| None),
            },
            "withdraw" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::withdraw(io, &env).map(|()| None),
            },
            "deposit" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::deposit(io, &env, &mut handler)
                    .map(|x| x.map(TransactionExecutionResult::Promise)),
            },
            "finish_deposit" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::finish_deposit(io, &env, &mut handler)
                    .map(|x| x.map(TransactionExecutionResult::Promise)),
            },
            "storage_deposit" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::storage_deposit(io, &env, &mut handler)
                    .map(|()| None),
            },
            "storage_unregister" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::storage_unregister(io, &env, &mut handler)
                    .map(|()| None),
            },
            "storage_withdraw" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::storage_withdraw(io, &env).map(|()| None),
            },
            "set_paused_flags" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::set_paused_flags(io, &env).map(|()| None),
            },
            "register_relayer" => {
                contract_methods::admin::register_relayer(io, &env).map(|()| None)
            }
            "set_eth_connector_contract_data" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::set_eth_connector_contract_data(io, &env)
                    .map(|()| None),
            },
            "new_eth_connector" => match () {
                #[cfg(feature = "ext-connector")]
                () => Ok(None),
                #[cfg(not(feature = "ext-connector"))]
                () => contract_methods::connector::new_eth_connector(io, &env).map(|()| None),
            },
            "unknown" => Ok(None),
            "exit_to_near_precompile_callback" => {
                contract_methods::connector::exit_to_near_precompile_callback(
                    io,
                    &env,
                    &mut handler,
                )
                .map(|maybe_output| maybe_output.map(TransactionExecutionResult::Submit))
            }
            "new" => contract_methods::admin::new(io, &env).map(|()| None),
            "set_eth_connector_contract_account" => match () {
                #[cfg(feature = "ext-connector")]
                () => contract_methods::connector::set_eth_connector_contract_account(io, &env)
                    .map(|()| None),
                #[cfg(not(feature = "ext-connector"))]
                () => Ok(None),
            },
            "factory_update" => contract_methods::xcc::factory_update(io, &env).map(|()| None),
            "factory_update_address_version" => {
                contract_methods::xcc::factory_update_address_version(io, &env, &handler)
                    .map(|()| None)
            }
            "factory_set_wnear_address" => {
                contract_methods::xcc::factory_set_wnear_address(io, &env).map(|()| None)
            }
            "fund_xcc_sub_account" => {
                contract_methods::xcc::fund_xcc_sub_account(io, &env, &mut handler).map(|()| None)
            }
            "withdraw_wnear_to_router" => {
                contract_methods::xcc::withdraw_wnear_to_router(io, &env, &mut handler)
                    .map(TransactionExecutionResult::Submit)
                    .map(Some)
            }
            "pause_precompiles" => {
                contract_methods::admin::pause_precompiles(io, &env).map(|()| None)
            }
            "resume_precompiles" => {
                contract_methods::admin::resume_precompiles(io, &env).map(|()| None)
            }
            "set_owner" => contract_methods::admin::set_owner(io, &env).map(|()| None),
            "set_upgrade_delay_blocks" => {
                contract_methods::admin::set_upgrade_delay_blocks(io, &env).map(|()| None)
            }
            "pause_contract" => contract_methods::admin::pause_contract(io, &env).map(|()| None),
            "resume_contract" => contract_methods::admin::resume_contract(io, &env).map(|()| None),
            "set_key_manager" => contract_methods::admin::set_key_manager(io, &env).map(|()| None),
            "add_relayer_key" => {
                contract_methods::admin::add_relayer_key(io, &env, &mut handler).map(|()| None)
            }
            "remove_relayer_key" => {
                contract_methods::admin::remove_relayer_key(io, &env, &mut handler).map(|()| None)
            }
            "start_hashchain" => contract_methods::admin::start_hashchain(io, &env).map(|()| None),
            "set_erc20_metadata" => {
                contract_methods::connector::set_erc20_metadata(io, &env, &mut handler)
                    .map(|_| None)
            }
            "set_fixed_gas" => {
                let args: FixedGasArgs = io.read_input_borsh().sdk_unwrap();
                contract_methods::silo::set_fixed_gas(&mut io, args.fixed_gas);
                Ok(None)
            }
            "set_silo_params" => {
                let args = io.read_input_borsh().sdk_unwrap();
                contract_methods::silo::set_silo_params(&mut io, args);
                Ok(None)
            }
            "add_entry_to_whitelist" => {
                let args = io.read_input_borsh().sdk_unwrap();
                contract_methods::silo::add_entry_to_whitelist(&io, &args);
                Ok(None)
            }
            "add_entry_to_whitelist_batch" => {
                let args = io.read_input_borsh::<Vec<_>>().sdk_unwrap();
                contract_methods::silo::add_entry_to_whitelist_batch(&io, args);
                Ok(None)
            }
            "remove_entry_from_whitelist" => {
                let args = io.read_input_borsh().sdk_unwrap();
                contract_methods::silo::remove_entry_from_whitelist(&io, &args);
                Ok(None)
            }
            "set_whitelist_status" => {
                let args = io.read_input_borsh().sdk_unwrap();
                contract_methods::silo::set_whitelist_status(&io, &args);
                Ok(None)
            }
            "mirror_erc20_token_callback" => {
                contract_methods::connector::mirror_erc20_token_callback(io, &env, &mut handler)
                    .map(|()| None)
            }
            "get_version" => contract_methods::admin::get_version(io).map(|()| None),
            "simulate_eth_call" => super::simulate::eth_call(io, env)
                .map(TransactionExecutionResult::Submit)
                .map(Some),
            _ => Err(ContractError {
                message: Box::new(format!("Unknown method: {method}")),
            }),
        });
        let result = out.map_err(|err| format!("{err:?}"));
        // the type of the response must be: `Result<Option<TransactionExecutionResult>, String>`
        dbg::Runtime::write(b"borealis/result", result);
    }
}
