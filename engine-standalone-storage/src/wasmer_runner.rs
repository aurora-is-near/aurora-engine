#![allow(clippy::needless_pass_by_value)]

use std::{ops::Deref, sync::Arc};

use aurora_engine::parameters::TransactionExecutionResult;
use aurora_engine_types::borsh::BorshDeserialize;

use aurora_engine_sdk::env::Fixed;
use engine_standalone_tracing::{types::call_tracer::CallTracer, TraceKind, TraceLog};
use sha3::digest::{FixedOutput, Update};
use thiserror::Error;

use rocksdb::DB;
use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Imports, Instance, Memory, MemoryView, Module,
    Store,
};

use crate::{Diff, DiffValue};

pub struct WasmerRunner {
    store: Store,
    env: FunctionEnv<WasmEnv>,
    imports: Imports,
    instance: Option<Instance>,
}

pub struct DerefDB(pub WasmerRunner);

impl DerefDB {
    pub fn new(db: Arc<DB>) -> Self {
        Self(WasmerRunner::new(db))
    }
}

impl Deref for DerefDB {
    type Target = Arc<DB>;

    fn deref(&self) -> &Self::Target {
        &self.0.env.as_ref(&self.0.store).db
    }
}

pub struct WasmEnv {
    state: NearState,
    db: Arc<DB>,
    memory: Option<Memory>,
}

fn with_env<T>(
    mut env: FunctionEnvMut<WasmEnv>,
    f: impl FnOnce(&mut NearState, &MemoryView<'_>, &DB) -> T,
) -> Option<T> {
    let (data, store) = env.data_and_store_mut();
    data.memory
        .as_ref()
        .map(|memory| f(&mut data.state, &memory.view(&store), &data.db))
}

#[derive(Debug, Error)]
pub enum WasmInitError {
    #[error("Wasmer compile error: {0}")]
    CompileError(#[from] wasmer::CompileError),
    #[error("Wasmer instantiation error: {0}")]
    InstantiationError(#[from] Box<wasmer::InstantiationError>),
    #[error("Wasmer export memory error: {0}")]
    ExportError(#[from] wasmer::ExportError),
}

#[derive(Debug, Error, Clone)]
pub enum WasmRuntimeError {
    #[error("Wasmer runtime error: {0}")]
    Inner(#[from] wasmer::RuntimeError),
    #[error("Wasmer export error: {0}")]
    Export(#[from] wasmer::ExportError),
    #[error("Contract code not set")]
    ContractCodeNotSet,
    #[error("Deserialize transaction execution result")]
    DeserializeResult,
    #[error("Deserialize transaction tracing result")]
    DeserializeTracing,
}

#[derive(Debug)]
pub struct WasmerRuntimeOutcome {
    pub diff: Diff,
    pub maybe_result: Result<Option<TransactionExecutionResult>, String>,
    pub trace_log: Option<Vec<TraceLog>>,
    pub call_tracer: Option<CallTracer>,
    pub custom_debug_info: Option<Vec<u8>>,
    pub output: Vec<u8>,
}

fn read_register(env: FunctionEnvMut<WasmEnv>, register_id: u64, ptr: u64) {
    with_env(env, |state, memory, _db| {
        state.read_register(memory, register_id, ptr);
    });
}

fn register_len(env: FunctionEnvMut<WasmEnv>, register_id: u64) -> u64 {
    env.data().state.register_len(register_id)
}

fn current_account_id(mut env: FunctionEnvMut<WasmEnv>, register_id: u64) {
    env.data_mut().state.current_account_id(register_id);
}

fn signer_account_id(mut env: FunctionEnvMut<WasmEnv>, register_id: u64) {
    env.data_mut().state.signer_account_id(register_id);
}

fn predecessor_account_id(mut env: FunctionEnvMut<WasmEnv>, register_id: u64) {
    env.data_mut().state.predecessor_account_id(register_id);
}

fn attached_deposit(env: FunctionEnvMut<WasmEnv>, balance_ptr: u64) {
    with_env(env, |state, memory, _db| {
        state.attached_deposit(memory, balance_ptr);
    });
}

fn digest<D: Default + Update + FixedOutput>(
    env: FunctionEnvMut<WasmEnv>,
    value_len: u64,
    value_ptr: u64,
    register_id: u64,
) {
    with_env(env, |state, memory, _db| {
        state.digest::<D>(memory, value_len, value_ptr, register_id);
    });
}

#[allow(clippy::too_many_arguments)]
fn ecrecover(
    env: FunctionEnvMut<WasmEnv>,
    hash_len: u64,
    hash_ptr: u64,
    sig_len: u64,
    sig_ptr: u64,
    v: u64,
    _flag: u64,
    register_id: u64,
) -> u64 {
    with_env(env, |state, memory, _db| {
        let res = state.ecrecover(memory, hash_len, hash_ptr, sig_len, sig_ptr, v, register_id);
        u64::from(res.is_ok())
    })
    .unwrap_or_default()
}

fn alt_bn128_g1_sum(
    env: FunctionEnvMut<WasmEnv>,
    value_len: u64,
    value_ptr: u64,
    register_id: u64,
) {
    with_env(env, |state, memory, _db| {
        state.alt_bn128_g1_sum(memory, value_len, value_ptr, register_id);
    });
}

fn alt_bn128_g1_multiexp(
    env: FunctionEnvMut<WasmEnv>,
    value_len: u64,
    value_ptr: u64,
    register_id: u64,
) {
    with_env(env, |state, memory, _db| {
        state.alt_bn128_g1_multiexp(memory, value_len, value_ptr, register_id);
    });
}

fn alt_bn128_pairing_check(env: FunctionEnvMut<WasmEnv>, value_len: u64, value_ptr: u64) -> u64 {
    with_env(env, |state, memory, _db| {
        state.alt_bn128_pairing_check(memory, value_len, value_ptr)
    })
    .unwrap_or_default()
}

fn value_return(env: FunctionEnvMut<WasmEnv>, value_len: u64, value_ptr: u64) {
    with_env(env, |state, memory, _db| {
        state.value_return(memory, value_len, value_ptr);
    });
}

impl WasmerRunner {
    #[allow(clippy::too_many_lines)]
    pub fn new(db: Arc<DB>) -> Self {
        let mut store = Store::default();

        let state = WasmEnv {
            state: NearState::default(),
            db,
            memory: None,
        };
        let env = FunctionEnv::new(&mut store, state);

        let imports = imports! {
            "env" => {
                // Registers
                "read_register" => Function::new_typed_with_env(&mut store, &env, read_register),
                "register_len" => Function::new_typed_with_env(&mut store, &env, register_len),
                // Context API
                "current_account_id" => Function::new_typed_with_env(&mut store, &env, current_account_id),
                "signer_account_id" => Function::new_typed_with_env(&mut store, &env, signer_account_id),
                "signer_account_pk" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _register_id: u64| {
                    eprintln!("LOG: the host function `signer_account_pk` is not implemented");
                }),
                "predecessor_account_id" => Function::new_typed_with_env(&mut store, &env, predecessor_account_id),
                "input" => Function::new_typed_with_env(&mut store, &env, |mut env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data_mut().state.input(register_id)),
                "block_index" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.block_index()),
                "block_timestamp" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.block_timestamp()),
                "epoch_height" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>| {
                    eprintln!("LOG: the host function `epoch_height` is not implemented");
                    0u64
                }),
                "storage_usage" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>| {
                    eprintln!("LOG: the host function `storage_usage` is not implemented");
                    0u64
                }),
                // Economics API
                "account_balance" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _balance_ptr: u64| {
                    eprintln!("LOG: the host function `account_balance` is not implemented");
                }),
                "attached_deposit" => Function::new_typed_with_env(&mut store, &env, attached_deposit),
                "prepaid_gas" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.prepaid_gas()),
                "used_gas" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.used_gas()),
                // Math API
                "random_seed" => Function::new_typed_with_env(&mut store, &env, |mut env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data_mut().state.random_seed(register_id)),
                "sha256" => Function::new_typed_with_env(&mut store, &env, digest::<sha2::Sha256>),
                "keccak256" => Function::new_typed_with_env(&mut store, &env, digest::<sha3::Keccak256>),
                "ripemd160" => Function::new_typed_with_env(&mut store, &env, digest::<ripemd::Ripemd160>),
                "ecrecover" => Function::new_typed_with_env(&mut store, &env, ecrecover),
                "alt_bn128_g1_sum" => Function::new_typed_with_env(&mut store, &env, alt_bn128_g1_sum),
                "alt_bn128_g1_multiexp" => Function::new_typed_with_env(&mut store, &env, alt_bn128_g1_multiexp),
                "alt_bn128_pairing_check" => Function::new_typed_with_env(&mut store, &env, alt_bn128_pairing_check),
                // Miscellaneous API
                "value_return" => Function::new_typed_with_env(&mut store, &env, value_return),
                "panic" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>| {
                    eprintln!("LOG: panic called from wasm");
                }),
                "panic_utf8" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, len: u64, ptr: u64| with_env(env, |st, memory, _| st.panic_utf8(memory, len, ptr)).unwrap_or_default()),
                "log_utf8" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, len: u64, ptr: u64| with_env(env, |st, memory, _| st.log_utf8(memory, len, ptr)).unwrap_or_default()),
                "log_utf16" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _len: u64, _ptr: u64| {
                    eprintln!("LOG: the host function `log_utf16` is not implemented");
                }),
                "abort" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, msg_ptr: u32, filename_ptr: u32, line: u32, col: u32| {
                    let _ = (msg_ptr, filename_ptr, line, col);
                    eprintln!("LOG: abort called from wasm");
                }),
                // Promises API
                "promise_create" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _account_id_len: u64, _account_id_ptr: u64, _method_name_len: u64, _method_name_ptr: u64, _arguments_len: u64, _arguments_ptr: u64, _amount_ptr: u64, _gas: u64| {
                    eprintln!("LOG: the host function `promise_create` is not implemented");
                    0u64
                }),
                "promise_then" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _account_id_len: u64, _account_id_ptr: u64, _method_name_len: u64, _method_name_ptr: u64, _arguments_len: u64, _arguments_ptr: u64, _amount_ptr: u64, _gas: u64| {
                    eprintln!("LOG: the host function `promise_then` is not implemented");
                    0u64
                }),
                "promise_and" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_idx_ptr: u64, _promise_idx_count: u64| {
                    eprintln!("LOG: the host function `promise_and` is not implemented");
                    0u64
                }),
                "promise_batch_create" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _account_id_len: u64, _account_id_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_create` is not implemented");
                    0u64
                }),
                "promise_batch_then" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _account_id_len: u64, _account_id_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_then` is not implemented");
                    0u64
                }),
                // Promise API actions
                "promise_batch_action_create_account" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_create_account` is not implemented");
                }),
                "promise_batch_action_deploy_contract" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _code_len: u64, _code_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_deploy_contract` is not implemented");
                }),
                "promise_batch_action_function_call" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _method_name_len: u64, _method_name_ptr: u64, _arguments_len: u64, _arguments_ptr: u64, _amount_ptr: u64, _gas: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_function_call` is not implemented");
                }),
                "promise_batch_action_transfer" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _amount_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_transfer` is not implemented");
                }),
                "promise_batch_action_stake" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _amount_ptr: u64, _public_key_len: u64, _public_key_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_stake` is not implemented");
                }),
                "promise_batch_action_add_key_with_full_access" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _public_key_len: u64, _public_key_ptr: u64, _nonce: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_add_key_with_full_access` is not implemented");
                }),
                "promise_batch_action_add_key_with_function_call" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _public_key_len: u64, _public_key_ptr: u64, _nonce: u64, _allowance_ptr: u64, _receiver_id_len: u64, _receiver_id_ptr: u64, _method_names_len: u64, _method_names_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_add_key_with_function_call` is not implemented");
                }),
                "promise_batch_action_delete_key" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _public_key_len: u64, _public_key_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_delete_key` is not implemented");
                }),
                "promise_batch_action_delete_account" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _beneficiary_id_len: u64, _beneficiary_id_ptr: u64| {
                    eprintln!("LOG: the host function `promise_batch_action_delete_account` is not implemented");
                }),
                // Promise API results
                "promise_results_count" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.promise_results_count()),
                "promise_result" => Function::new_typed_with_env(&mut store, &env, |mut env: FunctionEnvMut<WasmEnv>, result_idx: u64, register_id: u64| env.data_mut().state.promise_result(result_idx, register_id)),
                "promise_return" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, promise_id: u64| {
                    let _ = promise_id;
                    // No-op in standalone mode
                }),
                // Storage API
                "storage_write" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, key_len: u64, key_ptr: u64, value_len: u64, value_ptr: u64, register_id: u64| with_env(env, |st, memory, storage| st.storage_write(memory, storage, key_len, key_ptr, value_len, value_ptr, register_id)).unwrap_or_default()),
                "storage_read" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, key_len: u64, key_ptr: u64, register_id: u64| with_env(env, |st, memory, storage| st.storage_read(memory, storage, key_len, key_ptr, register_id)).unwrap_or_default()),
                "storage_remove" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, key_len: u64, key_ptr: u64, register_id: u64| with_env(env, |st, memory, storage| st.storage_remove(memory, storage, key_len, key_ptr, register_id)).unwrap_or_default()),
                "storage_has_key" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, key_len: u64, key_ptr: u64| with_env(env, |st, memory, storage| st.storage_has_key(memory, storage, key_len, key_ptr)).unwrap_or_default()),
                "storage_iter_prefix" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _prefix_len: u64, _prefix_ptr: u64| {
                    eprintln!("LOG: the host function `storage_iter_prefix` is not implemented");
                    0u64
                }),
                "storage_iter_range" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _from_len: u64, _from_ptr: u64, _to_len: u64, _to_ptr: u64| {
                    eprintln!("LOG: the host function `storage_iter_range` is not implemented");
                    0u64
                }),
                "storage_iter_next" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _iterator_id: u64, _key_register_id: u64, _value_register_id: u64| {
                    eprintln!("LOG: the host function `storage_iter_next` is not implemented");
                    0u64
                }),
                // Validator API
                "validator_stake" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _account_id_len: u64, _account_id_ptr: u64, _stake_ptr: u64| {
                    eprintln!("LOG: the host function `validator_stake` is not implemented");
                }),
                "validator_total_stake" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _stake_ptr: u64| {
                    eprintln!("LOG: the host function `validator_total_stake` is not implemented");
                }),
            }
        };

        Self {
            store,
            env,
            imports,
            instance: None,
        }
    }

    pub fn set_code(&mut self, code: Vec<u8>) -> Result<(), WasmInitError> {
        let module = Module::new(&self.store, code)?;
        let instance = Instance::new(&mut self.store, &module, &self.imports).map_err(Box::new)?;
        let memory = instance.exports.get_memory("memory")?.clone();
        self.env.as_mut(&mut self.store).memory = Some(memory);

        self.instance = Some(instance);
        Ok(())
    }

    #[must_use]
    pub fn take_cached_diff(&mut self) -> Diff {
        self.env.as_mut(&mut self.store).state.take_cached_diff()
    }

    pub fn initialized(&self) -> bool {
        self.instance.is_some()
    }

    /// must call on on-chain variant of the contract
    pub fn get_version(&mut self) -> Result<String, WasmRuntimeError> {
        self.instance
            .as_ref()
            .ok_or(WasmRuntimeError::ContractCodeNotSet)?
            .exports
            .get_typed_function::<(), ()>(&self.store, "get_version")?
            .call(&mut self.store)
            .map_err(WasmRuntimeError::from)
            .map(|()| self.env.as_mut(&mut self.store).state.take_output())
            .and_then(|v| String::from_utf8(v).map_err(|_| WasmRuntimeError::DeserializeResult))
    }

    /// must call on standalone variant of the contract
    pub fn get_version_at_wrapper(&mut self) -> Result<String, WasmRuntimeError> {
        let out = self.call_contract("get_version", None, &[], Fixed::default(), 0, 0, vec![])?;
        String::from_utf8(out.output).map_err(|_| WasmRuntimeError::DeserializeResult)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn call_contract(
        &mut self,
        method_name: &str,
        trace_kind: Option<TraceKind>,
        promise_data: &[Option<Vec<u8>>],
        env: Fixed,
        block_height: u64,
        transaction_position: u16,
        input: Vec<u8>,
    ) -> Result<WasmerRuntimeOutcome, WasmRuntimeError> {
        {
            self.env.as_mut(&mut self.store).state.init(
                method_name,
                trace_kind,
                block_height,
                transaction_position,
                input,
                env,
                promise_data.into(),
            );
        }
        self.instance
            .as_ref()
            .ok_or(WasmRuntimeError::ContractCodeNotSet)?
            .exports
            .get_typed_function::<(), ()>(&self.store, "execute")?
            .call(&mut self.store)
            .map_err(WasmRuntimeError::from)
            .and_then(|()| {
                type R = Result<Option<TransactionExecutionResult>, String>;

                let state = &mut self.env.as_mut(&mut self.store).state;
                let mut diff = state.get_transaction_diff();

                // Deserialize the execution result
                let mut value = diff
                    .get(b"borealis/result")
                    .and_then(DiffValue::value)
                    .ok_or(WasmRuntimeError::DeserializeResult)?;
                let maybe_result = <R as BorshDeserialize>::deserialize(&mut value)
                    .map_err(|_| WasmRuntimeError::DeserializeResult)?;

                // Deserialize tracing info if present
                let trace_log = diff
                    .get(b"borealis/transaction_tracing")
                    .and_then(DiffValue::value)
                    .map(|mut value| {
                        BorshDeserialize::deserialize(&mut value)
                            .map_err(|_| WasmRuntimeError::DeserializeTracing)
                    })
                    .transpose()?;
                let call_tracer = diff
                    .get(b"borealis/call_frame_tracing")
                    .and_then(DiffValue::value)
                    .map(|mut value| {
                        BorshDeserialize::deserialize(&mut value)
                            .map_err(|_| WasmRuntimeError::DeserializeTracing)
                    })
                    .transpose()?;
                let custom_debug_info = diff
                    .get(b"borealis/custom_debug_info")
                    .and_then(DiffValue::value)
                    .map(<[u8]>::to_vec);
                diff.retain(|key, _| !key.starts_with(b"borealis/"));
                let output = state.take_output();

                Ok(WasmerRuntimeOutcome {
                    diff,
                    maybe_result,
                    trace_log,
                    call_tracer,
                    custom_debug_info,
                    output,
                })
            })
    }
}

pub use self::state::NearState;
mod state {
    #![allow(clippy::as_conversions)]

    use std::{borrow::Cow, iter, mem};

    use aurora_engine_sdk::env::Fixed;
    use aurora_engine_types::borsh;
    use engine_standalone_tracing::TraceKind;
    use rocksdb::DB;
    use sha2::digest::{FixedOutput, Update};
    use wasmer::MemoryView;

    use crate::{Diff, DiffValue};

    pub struct NearState {
        inner: StateInner,
        registers: Vec<Register>,
    }

    #[derive(Default)]
    struct Register(Option<Vec<u8>>);

    struct StateInner {
        env: Option<Fixed>,
        input: Vec<u8>,
        output: Vec<u8>,
        promise_data: Box<[Option<Vec<u8>>]>,

        bound_block_height: u64,
        bound_tx_position: u16,
        cached_diff: Diff,
        current_diff: Diff,
    }

    const REGISTERS_NUMBER: usize = 6;

    impl Default for StateInner {
        fn default() -> Self {
            Self {
                env: None,
                input: vec![],
                output: vec![],
                promise_data: Box::new([]),
                bound_block_height: 0,
                bound_tx_position: 0,
                cached_diff: Diff::default(),
                current_diff: Diff::default(),
            }
        }
    }

    impl Default for NearState {
        fn default() -> Self {
            Self {
                inner: StateInner::default(),
                registers: iter::repeat_with(Register::default)
                    .take(REGISTERS_NUMBER)
                    .collect(),
            }
        }
    }

    impl NearState {
        #[must_use]
        pub fn take_output(&mut self) -> Vec<u8> {
            mem::take(&mut self.inner.output)
        }

        #[must_use]
        pub fn get_transaction_diff(&mut self) -> Diff {
            let current_diff = mem::take(&mut self.inner.current_diff);
            for (k, v) in &current_diff {
                match v {
                    DiffValue::Deleted => self.inner.cached_diff.delete(k.clone()),
                    DiffValue::Modified(v) => self.inner.cached_diff.modify(k.clone(), v.clone()),
                }
            }
            current_diff
        }

        #[must_use]
        pub fn take_cached_diff(&mut self) -> Diff {
            mem::take(&mut self.inner.cached_diff)
        }

        #[allow(clippy::too_many_arguments)]
        pub fn init(
            &mut self,
            method_name: &str,
            trace_kind: Option<TraceKind>,
            block_height: u64,
            transaction_position: u16,
            input: Vec<u8>,
            env: Fixed,
            promise_data: Box<[Option<Vec<u8>>]>,
        ) {
            self.registers
                .iter_mut()
                .for_each(|reg| *reg = Register::default());
            self.inner.env = Some(env);
            self.inner.input = input;
            self.inner.output.clear();
            self.inner.promise_data = promise_data;
            self.inner.bound_block_height = block_height;
            self.inner.bound_tx_position = transaction_position;

            self.inner.current_diff.modify(
                b"borealis/method".to_vec(),
                borsh::to_vec(method_name).expect("must serialize string"),
            );
            if let Some(v) = &trace_kind {
                self.inner.current_diff.modify(
                    b"borealis/trace_kind".to_vec(),
                    borsh::to_vec(&v).expect("must serialize trivial enum"),
                );
            }
        }

        #[allow(clippy::significant_drop_tightening)]
        fn read_reg<F, T>(&self, register_id: u64, mut op: F) -> T
        where
            F: FnMut(&Register) -> T,
        {
            let index = usize::try_from(register_id).expect("pointer size must be wide enough");
            let reg = self
                .registers
                .get(index)
                .unwrap_or_else(|| panic!("no such register {register_id}"));
            op(reg)
        }

        fn set_reg(registers: &mut [Register], register_id: u64, data: Cow<[u8]>) {
            let index = usize::try_from(register_id).expect("pointer size must be wide enough");

            *registers
                .get_mut(index)
                .unwrap_or_else(|| panic!("no such register {register_id}")) =
                Register(Some(data.into_owned()));
        }

        /// The lifetime is static because it comes from the caller.
        /// This function is supposed to be external, so the caller has the highest possible lifetime.
        fn get_data(&self, memory: &MemoryView<'_>, ptr: u64, len: u64) -> Vec<u8> {
            if len == u64::MAX {
                self.read_reg(ptr, |reg| {
                    reg.0.as_ref().expect("register must exist").clone()
                })
            } else {
                let len = usize::try_from(len).expect("pointer size must be wide enough");
                let mut data = vec![0; len];
                if let Err(err) = memory.read(ptr, &mut data) {
                    eprintln!("LOG: panic called from wasm: {err}");
                }
                data
            }
        }

        // Near API

        pub fn read_register(&self, memory: &MemoryView<'_>, register_id: u64, ptr: u64) {
            self.read_reg(register_id, |reg| {
                if let Some(reg) = &reg.0 {
                    if let Err(err) = memory.write(ptr, reg) {
                        eprintln!(
                            "LOG: panic called from wasm: `read_register` {register_id} failed with: {err}"
                        );
                    }
                }
            });
        }

        pub fn register_len(&self, register_id: u64) -> u64 {
            self.read_reg(register_id, |reg| {
                reg.0.as_ref().map_or(u64::MAX, |reg| {
                    reg.len()
                        .try_into()
                        .expect("pointer size must be wide enough")
                })
            })
        }

        pub fn current_account_id(&mut self, register_id: u64) {
            let Some(env) = &mut self.inner.env else {
                panic!("environment is not set");
            };
            Self::set_reg(
                &mut self.registers,
                register_id,
                env.current_account_id.as_bytes().into(),
            );
        }

        pub fn signer_account_id(&mut self, register_id: u64) {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            Self::set_reg(
                &mut self.registers,
                register_id,
                env.signer_account_id.as_bytes().into(),
            );
        }

        pub fn predecessor_account_id(&mut self, register_id: u64) {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            Self::set_reg(
                &mut self.registers,
                register_id,
                env.predecessor_account_id.as_bytes().into(),
            );
        }

        pub fn input(&mut self, register_id: u64) {
            let input = &self.inner.input;
            Self::set_reg(&mut self.registers, register_id, input.into());
        }

        pub fn block_index(&self) -> u64 {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            env.block_height
        }

        pub fn block_timestamp(&self) -> u64 {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            env.block_timestamp.nanos()
        }

        pub fn attached_deposit(&self, memory: &MemoryView<'_>, balance_ptr: u64) {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            if let Err(err) = memory.write(balance_ptr, env.attached_deposit.to_ne_bytes().as_ref())
            {
                eprintln!("LOG: panic called from wasm: `attached_deposit` failed with: {err}");
            }
        }

        pub fn prepaid_gas(&self) -> u64 {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            env.prepaid_gas.as_u64()
        }

        pub fn used_gas(&self) -> u64 {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            env.used_gas.as_u64()
        }

        pub fn random_seed(&mut self, register_id: u64) {
            let Some(env) = &self.inner.env else {
                panic!("environment is not set");
            };
            Self::set_reg(
                &mut self.registers,
                register_id,
                env.random_seed.as_bytes().into(),
            );
        }

        pub fn digest<D: Default + Update + FixedOutput>(
            &mut self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            let data = self.get_data(memory, value_ptr, value_len);
            let hash = D::default().chain(data).finalize_fixed();
            Self::set_reg(&mut self.registers, register_id, hash.as_slice().into());
        }

        #[allow(clippy::too_many_arguments)]
        pub fn ecrecover(
            &mut self,
            memory: &MemoryView<'_>,
            hash_len: u64,
            hash_ptr: u64,
            sig_len: u64,
            sig_ptr: u64,
            v: u64,
            register_id: u64,
        ) -> Result<(), ()> {
            let hash = self.get_data(memory, hash_ptr, hash_len);
            let hash = libsecp256k1::Message::parse_slice(&hash).map_err(|_| ())?;
            let sig = self.get_data(memory, sig_ptr, sig_len);
            let sig = libsecp256k1::Signature::parse_standard_slice(&sig).map_err(|_| ())?;
            let bit = match v {
                0..=26 => u8::try_from(v).expect("checked above"),
                _ => u8::try_from(v - 27).map_err(drop)?,
            };
            let recovery_id = libsecp256k1::RecoveryId::parse(bit).map_err(|_| ())?;

            let public_key = libsecp256k1::recover(&hash, &sig, &recovery_id).map_err(|_| ())?;
            Self::set_reg(
                &mut self.registers,
                register_id,
                public_key.serialize()[1..].into(),
            );

            Ok(())
        }

        pub fn alt_bn128_g1_sum(
            &mut self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            let mut input = self.get_data(memory, value_ptr, value_len);
            input.resize(0x82, 0);
            input.remove(0);
            input.remove(64);
            let mut output = aurora_engine_sdk::alt_bn128_g1_sum(
                input[..0x40].try_into().unwrap(),
                input[0x40..0x80].try_into().unwrap(),
            )
            .unwrap_or([0; 0x40]);
            output[0x00..0x20].reverse();
            output[0x20..0x40].reverse();
            Self::set_reg(&mut self.registers, register_id, Cow::Borrowed(&output));
        }

        pub fn alt_bn128_g1_multiexp(
            &mut self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            let mut input = self.get_data(memory, value_ptr, value_len);
            input.resize(0x60, 0);
            let mut output = aurora_engine_sdk::alt_bn128_g1_scalar_multiple(
                input[..0x40].try_into().unwrap(),
                input[0x40..].try_into().unwrap(),
            )
            .unwrap_or([0; 0x40]);
            // swap endianness
            output[0x00..0x20].reverse();
            output[0x20..0x40].reverse();
            Self::set_reg(&mut self.registers, register_id, Cow::Borrowed(&output));
        }

        pub fn alt_bn128_pairing_check(
            &self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
        ) -> u64 {
            let input = self.get_data(memory, value_ptr, value_len);
            let pairs = input.chunks(0xc0).map(|chunk| {
                let chunk = if chunk.len() < 0xc0 {
                    let mut v = chunk.to_vec();
                    v.resize(0xc0, 0);
                    Cow::Owned(v)
                } else {
                    Cow::Borrowed(chunk)
                };
                (
                    chunk[..0x40].try_into().unwrap(),
                    chunk[0x40..].try_into().unwrap(),
                )
            });
            aurora_engine_sdk::alt_bn128_pairing(pairs)
                .unwrap_or_default()
                .into()
        }

        pub fn value_return(&mut self, memory: &MemoryView<'_>, value_len: u64, value_ptr: u64) {
            let data = self.get_data(memory, value_ptr, value_len);
            self.inner.output = data;
        }

        pub fn panic_utf8(&self, memory: &MemoryView<'_>, len: u64, ptr: u64) {
            let data = self.get_data(memory, ptr, len);
            let message = String::from_utf8_lossy(&data);
            eprintln!("LOG: panic called from wasm: {message}");
        }

        pub fn log_utf8(&self, memory: &MemoryView<'_>, len: u64, ptr: u64) {
            let data = self.get_data(memory, ptr, len);
            let message = String::from_utf8_lossy(&data);
            eprintln!("LOG: {message}");
        }

        pub fn promise_results_count(&self) -> u64 {
            u64::try_from(self.inner.promise_data.len()).unwrap_or_default()
        }

        pub fn promise_result(&mut self, result_idx: u64, register_id: u64) -> u64 {
            let i = usize::try_from(result_idx).expect("index too big");
            let Some(data) = self.inner.promise_data.get(i) else {
                // not ready
                return 0;
            };
            let Some(data) = data else {
                // failed
                return 2;
            };
            // ready
            Self::set_reg(&mut self.registers, register_id, data.as_slice().into());
            1
        }

        #[allow(clippy::too_many_arguments)]
        pub fn storage_write(
            &mut self,
            memory: &MemoryView<'_>,
            db: &DB,
            key_len: u64,
            key_ptr: u64,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) -> u64 {
            // preserve the register value
            let value = self.get_data(memory, value_ptr, value_len);

            // fetch original value into register
            let res = self.storage_read(memory, db, key_len, key_ptr, register_id);

            let key = self.get_data(memory, key_ptr, key_len);
            self.inner.current_diff.modify(key, value);
            res
        }

        pub fn storage_read(
            &mut self,
            memory: &MemoryView<'_>,
            db: &DB,
            key_len: u64,
            key_ptr: u64,
            register_id: u64,
        ) -> u64 {
            let key = self.get_data(memory, key_ptr, key_len);

            let lock = &self.inner;
            if let Some(diff) = None
                .or_else(|| lock.current_diff.get(&key))
                .or_else(|| lock.cached_diff.get(&key))
            {
                return diff.value().map_or(0, |bytes| {
                    Self::set_reg(&mut self.registers, register_id, bytes.into());
                    1
                });
            }

            if let Ok(value) = read_db(db, &key, lock.bound_block_height, lock.bound_tx_position) {
                return value.value().map_or(0, |bytes| {
                    Self::set_reg(&mut self.registers, register_id, bytes.into());
                    1
                });
            }

            0
        }

        pub fn storage_remove(
            &mut self,
            memory: &MemoryView<'_>,
            db: &DB,
            key_len: u64,
            key_ptr: u64,
            register_id: u64,
        ) -> u64 {
            // fetch original value into register
            let res = self.storage_read(memory, db, key_len, key_ptr, register_id);
            let key = self.get_data(memory, key_ptr, key_len);
            self.inner.current_diff.delete(key);
            res
        }

        pub fn storage_has_key(
            &self,
            memory: &MemoryView<'_>,
            db: &DB,
            key_len: u64,
            key_ptr: u64,
        ) -> u64 {
            let key = self.get_data(memory, key_ptr, key_len);
            let lock = &self.inner;
            if let Some(value) = None
                .or_else(|| lock.current_diff.get(&key))
                .or_else(|| lock.cached_diff.get(&key))
            {
                return matches!(value, DiffValue::Modified(..)).into();
            }

            read_db(db, &key, lock.bound_block_height, lock.bound_tx_position)
                .map_or(0, |diff| u64::from(diff.value().is_some()))
        }
    }

    fn read_db(
        db: &DB,
        key: &[u8],
        bound_block_height: u64,
        transaction_position: u16,
    ) -> Result<DiffValue, crate::Error> {
        let upper_bound =
            crate::construct_engine_key(key, bound_block_height, transaction_position);
        let lower_bound = crate::construct_storage_key(crate::StoragePrefix::Engine, key);
        let mut opt = rocksdb::ReadOptions::default();
        opt.set_iterate_upper_bound(upper_bound);
        opt.set_iterate_lower_bound(lower_bound);

        let mut iter = db.iterator_opt(rocksdb::IteratorMode::End, opt);
        // TODO: error kind
        let (_, value) = iter.next().ok_or(crate::Error::NoBlockAtHeight(0))??;
        Ok(DiffValue::try_from_bytes(&value).expect("diff value is invalid"))
    }
}
