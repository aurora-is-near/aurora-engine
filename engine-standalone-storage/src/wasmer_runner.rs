use aurora_engine::parameters::TransactionExecutionResult;
use aurora_engine_types::borsh::BorshDeserialize;

use aurora_engine_sdk::env::Fixed;
use engine_standalone_tracing::{types::call_tracer::CallTracer, TraceKind, TraceLog};
use sha3::digest::{FixedOutput, Update};
use thiserror::Error;

use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Imports, Instance, Memory, MemoryView, Module,
    Store,
};

use crate::{Diff, DiffValue, Storage};

pub struct WasmerRunner {
    store: Store,
    env: FunctionEnv<WasmEnv>,
    imports: Imports,
    instance: Option<Instance>,
}

pub struct WasmEnv {
    state: state::State,
    storage: Storage,
    memory: Option<Memory>,
}

fn with_env<T>(
    mut env: FunctionEnvMut<WasmEnv>,
    f: impl FnOnce(&mut state::State, &MemoryView<'_>, &Storage) -> T,
) -> Option<T> {
    let (data, store) = env.data_and_store_mut();
    data.memory
        .as_ref()
        .map(|memory| f(&mut data.state, &memory.view(&store), &data.storage))
}

#[derive(Debug, Error)]
pub enum WasmInitError {
    #[error("Wasmer compile error: {0}")]
    CompileError(#[from] wasmer::CompileError),
    #[error("Wasmer instantiation error: {0}")]
    InstantiationError(#[from] wasmer::InstantiationError),
    #[error("Wasmer export memory error: {0}")]
    ExportError(#[from] wasmer::ExportError),
}

#[derive(Debug, Error)]
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

impl WasmerRunner {
    pub fn new(storage: Storage) -> Self {
        let mut store = Store::default();

        let state = WasmEnv {
            state: state::State::default(),
            storage,
            memory: None,
        };
        let env = FunctionEnv::new(&mut store, state);

        fn read_register(env: FunctionEnvMut<WasmEnv>, register_id: u64, ptr: u64) {
            with_env(env, |state, memory, _storage| {
                state.read_register(memory, register_id, ptr)
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
            with_env(env, |state, memory, _storage| {
                state.attached_deposit(memory, balance_ptr);
            });
        }

        fn digest<D: Default + Update + FixedOutput>(
            env: FunctionEnvMut<WasmEnv>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            with_env(env, |state, memory, _storage| {
                state.digest::<D>(memory, value_len, value_ptr, register_id);
            });
        }

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
            with_env(env, |state, memory, _storage| {
                let res =
                    state.ecrecover(memory, hash_len, hash_ptr, sig_len, sig_ptr, v, register_id);
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
            with_env(env, |state, memory, _storage| {
                state.alt_bn128_g1_sum(memory, value_len, value_ptr, register_id);
            });
        }

        fn alt_bn128_g1_multiexp(
            env: FunctionEnvMut<WasmEnv>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            with_env(env, |state, memory, _storage| {
                state.alt_bn128_g1_multiexp(memory, value_len, value_ptr, register_id);
            });
        }

        fn alt_bn128_pairing_check(
            env: FunctionEnvMut<WasmEnv>,
            value_len: u64,
            value_ptr: u64,
        ) -> u64 {
            with_env(env, |state, memory, _storage| {
                state.alt_bn128_pairing_check(memory, value_len, value_ptr)
            })
            .unwrap_or_default()
        }

        fn value_return(env: FunctionEnvMut<WasmEnv>, value_len: u64, value_ptr: u64) {
            with_env(env, |state, memory, _storage| {
                state.value_return(memory, value_len, value_ptr);
            });
        }

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
        let instance = Instance::new(&mut self.store, &module, &self.imports)?;
        let memory = instance.exports.get_memory("memory")?.clone();
        self.env.as_mut(&mut self.store).memory = Some(memory);

        self.instance = Some(instance);
        Ok(())
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
            .map(|_| self.env.as_mut(&mut self.store).state.take_output())
            .and_then(|v| String::from_utf8(v).map_err(|_| WasmRuntimeError::DeserializeResult))
    }

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
                let state = &self.env.as_mut(&mut self.store).state;
                let mut diff = state.get_transaction_diff();

                // Deserialize the execution result
                let mut value = diff
                    .get(b"borealis/result")
                    .and_then(DiffValue::value)
                    .ok_or(WasmRuntimeError::DeserializeResult)?;
                type R = Result<Option<TransactionExecutionResult>, String>;
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

pub mod state {
    #![allow(clippy::as_conversions)]

    use std::{borrow::Cow, iter};

    use aurora_engine_precompiles::{
        alt_bn256::{Bn256Add, Bn256Mul, Bn256Pair},
        Byzantium, Precompile,
    };
    use aurora_engine_sdk::env::Fixed;
    use aurora_engine_types::{borsh, H160, U256};
    use aurora_evm::Context;
    use engine_standalone_tracing::TraceKind;
    use sha2::digest::{FixedOutput, Update};
    use wasmer::MemoryView;

    use crate::{Diff, DiffValue, Storage};

    pub struct State {
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
        transaction_diff: Diff,
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
                transaction_diff: Diff::default(),
            }
        }
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                inner: StateInner::default(),
                registers: iter::repeat_with(Register::default)
                    .take(REGISTERS_NUMBER)
                    .collect(),
            }
        }
    }

    impl State {
        #[must_use]
        pub fn take_output(&self) -> Vec<u8> {
            self.inner.output.clone()
        }

        #[must_use]
        pub fn get_transaction_diff(&self) -> Diff {
            self.inner.transaction_diff.clone()
        }

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
            self.inner = StateInner {
                env: Some(env),
                input,
                output: vec![],
                promise_data,
                bound_block_height: block_height,
                bound_tx_position: transaction_position,
                transaction_diff: Diff::default(),
            };

            self.inner.transaction_diff.modify(
                b"borealis/method".to_vec(),
                borsh::to_vec(method_name).expect("must serialize string"),
            );
            if let Some(v) = &trace_kind {
                self.inner.transaction_diff.modify(
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

        /// Near API

        pub fn read_register(&self, memory: &MemoryView<'_>, register_id: u64, ptr: u64) {
            self.read_reg(register_id, |reg| {
                if let Some(reg) = &reg.0 {
                    if let Err(err) = memory.write(ptr, &*reg) {
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
            input.remove(0);
            input.remove(64);
            // endianness
            input[0x00..0x20].reverse();
            input[0x20..0x40].reverse();
            input[0x40..0x60].reverse();
            input[0x60..0x80].reverse();
            let precompile = Bn256Add::<Byzantium>::new();
            let context = Context {
                address: H160::default(),
                caller: H160::default(),
                apparent_value: U256::default(),
            };
            let mut output = precompile
                .run(&input, None, &context, false)
                .inspect_err(|err| eprintln!("{err:?}"))
                .map_or(vec![0; 0x40], |x| x.output);
            // swap endianness
            output[0x00..0x20].reverse();
            output[0x20..0x40].reverse();
            Self::set_reg(&mut self.registers, register_id, output.into());
        }

        pub fn alt_bn128_g1_multiexp(
            &mut self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            let mut input = self.get_data(memory, value_ptr, value_len);
            input[0x00..0x20].reverse();
            input[0x20..0x40].reverse();
            input[0x40..0x60].reverse();

            let precompile = Bn256Mul::<Byzantium>::new();
            let context = Context {
                address: H160::default(),
                caller: H160::default(),
                apparent_value: U256::default(),
            };
            let mut output = precompile
                .run(&input, None, &context, false)
                .inspect_err(|err| eprintln!("{err:?}"))
                .map_or(vec![0; 0x40], |x| x.output);
            // swap endianness
            output[0x00..0x20].reverse();
            output[0x20..0x40].reverse();
            Self::set_reg(&mut self.registers, register_id, output.into());
        }

        pub fn alt_bn128_pairing_check(
            &mut self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
        ) -> u64 {
            let mut input = self.get_data(memory, value_ptr, value_len);
            dbg!(hex::encode(&input));
            input.chunks_mut(0x20).for_each(<[u8]>::reverse);
            for pair in input.chunks_mut(0xc0) {
                let mut b = [0; 0x20];
                b.clone_from_slice(&pair[0x40..][..0x20]);
                let mut c = [0; 0x20];
                c.clone_from_slice(&pair[0x60..][..0x20]);
                pair[0x40..][..0x20].clone_from_slice(&c);
                pair[0x60..][..0x20].clone_from_slice(&b);

                // is there more ergonomic swap of subslices?
                let mut b = [0; 0x20];
                b.clone_from_slice(&pair[0x80..][..0x20]);
                let mut c = [0; 0x20];
                c.clone_from_slice(&pair[0xa0..][..0x20]);
                pair[0x80..][..0x20].clone_from_slice(&c);
                pair[0xa0..][..0x20].clone_from_slice(&b);
            }

            let precompile = Bn256Pair::<Byzantium>::new();
            let context = Context {
                address: H160::default(),
                caller: H160::default(),
                apparent_value: U256::default(),
            };
            let output = precompile
                .run(&input, None, &context, false)
                .inspect_err(|err| eprintln!("{err:?}"))
                .map_or(vec![0; 0x20], |x| x.output);

            dbg!(hex::encode(&output));
            if output == [0; 0x20] {
                0
            } else {
                1
            }
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

        pub fn storage_write(
            &mut self,
            memory: &MemoryView<'_>,
            db: &Storage,
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

            self.inner
                .transaction_diff
                .modify(key.to_vec(), value.to_vec());
            res
        }

        pub fn storage_read(
            &mut self,
            memory: &MemoryView<'_>,
            db: &Storage,
            key_len: u64,
            key_ptr: u64,
            register_id: u64,
        ) -> u64 {
            let key = self.get_data(memory, key_ptr, key_len);

            let lock = &self.inner;
            if let Some(diff) = lock.transaction_diff.get(&key) {
                return diff.value().map_or(0, |bytes| {
                    Self::set_reg(&mut self.registers, register_id, bytes.into());
                    1
                });
            }

            if let Ok(value) = db.read_by_key(&key, lock.bound_block_height, lock.bound_tx_position)
            {
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
            db: &Storage,
            key_len: u64,
            key_ptr: u64,
            register_id: u64,
        ) -> u64 {
            // fetch original value into register
            let res = self.storage_read(memory, db, key_len, key_ptr, register_id);

            let key = self.get_data(memory, key_ptr, key_len);
            self.inner.transaction_diff.delete(key);
            res
        }

        pub fn storage_has_key(
            &self,
            memory: &MemoryView<'_>,
            db: &Storage,
            key_len: u64,
            key_ptr: u64,
        ) -> u64 {
            let key = self.get_data(memory, key_ptr, key_len);
            let lock = &self.inner;
            if let Some(value) = lock.transaction_diff.get(&key) {
                return matches!(value, DiffValue::Modified(..)).into();
            }

            db.read_by_key(&key, lock.bound_block_height, lock.bound_tx_position)
                .map_or(0, |diff| u64::from(diff.value().is_some()))
        }
    }
}
