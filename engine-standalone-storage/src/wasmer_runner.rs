use parking_lot::Mutex;

use aurora_engine_sdk::env::Fixed;
use engine_standalone_tracing::TraceKind;
use sha3::digest::{FixedOutput, Update};
use thiserror::Error;

use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Imports, Instance, Memory, MemoryView, Module,
    Store,
};

use crate::{Diff, Storage};

pub struct WasmerRunner {
    store: Store,
    env: FunctionEnv<WasmEnv>,
    imports: Imports,
    instance: Option<Instance>,
}

pub struct WasmEnv {
    state: state::State,
    storage: Storage,
    memory: Mutex<Option<Memory>>,
}

fn with_env<T>(
    mut env: FunctionEnvMut<WasmEnv>,
    f: impl FnOnce(&state::State, &MemoryView<'_>, &Storage) -> T,
) -> Option<T> {
    let (data, store) = env.data_and_store_mut();
    let lock = data.memory.lock();
    (*lock)
        .as_ref()
        .map(|memory| f(&data.state, &memory.view(&store), &data.storage))
}

#[derive(Debug, Error)]
pub enum WasmerRunnerError {
    #[error("Wasmer compile error: {0}")]
    CompileError(#[from] wasmer::CompileError),
    #[error("Wasmer instantiation error: {0}")]
    InstantiationError(#[from] wasmer::InstantiationError),
    #[error("Wasmer export memory error: {0}")]
    ExportError(#[from] wasmer::ExportError),
}

impl WasmerRunner {
    pub fn new(storage: Storage) -> Self {
        let mut store = Store::default();

        let state = WasmEnv {
            state: state::State::default(),
            storage,
            memory: Mutex::new(None),
        };
        let env = FunctionEnv::new(&mut store, state);

        fn read_register(mut env: FunctionEnvMut<WasmEnv>, register_id: u64, ptr: u64) {
            let (data, store) = env.data_and_store_mut();
            let lock = data.memory.lock();
            if let Some(memory) = &*lock {
                data.state
                    .read_register(memory.view(&store), register_id, ptr);
            }
        }

        fn attached_deposit(mut env: FunctionEnvMut<WasmEnv>, balance_ptr: u64) {
            let (data, store) = env.data_and_store_mut();
            let lock = data.memory.lock();
            if let Some(memory) = &*lock {
                data.state
                    .attached_deposit(memory.view(&store), balance_ptr);
            }
        }

        fn digest<D: Default + Update + FixedOutput>(
            mut env: FunctionEnvMut<WasmEnv>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            let (data, store) = env.data_and_store_mut();
            let lock = data.memory.lock();
            if let Some(memory) = &*lock {
                data.state
                    .digest::<D>(&memory.view(&store), value_len, value_ptr, register_id);
            }
        }

        fn ecrecover(
            mut env: FunctionEnvMut<WasmEnv>,
            hash_len: u64,
            hash_ptr: u64,
            sig_len: u64,
            sig_ptr: u64,
            v: u64,
            _fl: u64,
            register_id: u64,
        ) -> u64 {
            let (data, store) = env.data_and_store_mut();
            let lock = data.memory.lock();
            if let Some(memory) = &*lock {
                let res = data.state.ecrecover(
                    &memory.view(&store),
                    hash_len,
                    hash_ptr,
                    sig_len,
                    sig_ptr,
                    v,
                    register_id,
                );
                u64::from(res.is_ok())
            } else {
                0
            }
        }

        fn value_return(mut env: FunctionEnvMut<WasmEnv>, value_len: u64, value_ptr: u64) {
            let (data, store) = env.data_and_store_mut();
            let lock = data.memory.lock();
            if let Some(memory) = &*lock {
                data.state
                    .value_return(&memory.view(&store), value_len, value_ptr);
            }
        }

        let imports = imports! {
            "env" => {
                // Registers
                "read_register" => Function::new_typed_with_env(&mut store, &env, read_register),
                "register_len" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, register_id| env.data().state.register_len(register_id)),
                // Context API
                "current_account_id" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data().state.current_account_id(register_id)),
                "signer_account_id" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data().state.signer_account_id(register_id)),
                "signer_account_pk" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _register_id: u64| {
                    // Not implemented
                }),
                "predecessor_account_id" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data().state.predecessor_account_id(register_id)),
                "input" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data().state.input(register_id)),
                "block_index" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.block_index()),
                "block_timestamp" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.block_timestamp()),
                "epoch_height" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>| {
                    // Not implemented
                    0u64
                }),
                "storage_usage" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>| {
                    // Not implemented
                    0u64
                }),
                // Economics API
                "account_balance" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _balance_ptr: u64| {
                    // Not implemented
                }),
                "attached_deposit" => Function::new_typed_with_env(&mut store, &env, attached_deposit),
                "prepaid_gas" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.prepaid_gas()),
                "used_gas" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.used_gas()),
                // Math API
                "random_seed" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, register_id: u64| env.data().state.random_seed(register_id)),
                "sha256" => Function::new_typed_with_env(&mut store, &env, digest::<sha2::Sha256>),
                "keccak256" => Function::new_typed_with_env(&mut store, &env, digest::<sha3::Keccak256>),
                "ripemd160" => Function::new_typed_with_env(&mut store, &env, digest::<ripemd::Ripemd160>),
                "ecrecover" => Function::new_typed_with_env(&mut store, &env, ecrecover),
                "alt_bn128_g1_sum" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _value_len: u64, _value_ptr: u64, _register_id: u64| {
                    // Not implemented
                }),
                "alt_bn128_g1_multiexp" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _value_len: u64, _value_ptr: u64, _register_id: u64| {
                    // Not implemented
                }),
                "alt_bn128_pairing_check" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _value_len: u64, _value_ptr: u64| {
                    // Not implemented
                    0u64
                }),
                // Miscellaneous API
                "value_return" => Function::new_typed_with_env(&mut store, &env, value_return),
                "panic" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>| {
                    eprintln!("LOG: panic called from wasm");
                }),
                "panic_utf8" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, len: u64, ptr: u64| with_env(env, |st, memory, _| st.panic_utf8(memory, len, ptr)).unwrap_or_default()),
                "log_utf8" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, len: u64, ptr: u64| with_env(env, |st, memory, _| st.log_utf8(memory, len, ptr)).unwrap_or_default()),
                "log_utf16" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _len: u64, _ptr: u64| {
                    // Not implemented
                }),
                "abort" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, msg_ptr: u32, filename_ptr: u32, line: u32, col: u32| {
                    let _ = (msg_ptr, filename_ptr, line, col);
                    eprintln!("LOG: abort called from wasm");
                }),
                // Promises API
                "promise_create" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _account_id_len: u64, _account_id_ptr: u64, _method_name_len: u64, _method_name_ptr: u64, _arguments_len: u64, _arguments_ptr: u64, _amount_ptr: u64, _gas: u64| {
                    // Not implemented
                    0u64
                }),
                "promise_then" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _account_id_len: u64, _account_id_ptr: u64, _method_name_len: u64, _method_name_ptr: u64, _arguments_len: u64, _arguments_ptr: u64, _amount_ptr: u64, _gas: u64| {
                    // Not implemented
                    0u64
                }),
                "promise_and" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_idx_ptr: u64, _promise_idx_count: u64| {
                    // Not implemented
                    0u64
                }),
                "promise_batch_create" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _account_id_len: u64, _account_id_ptr: u64| {
                    // Not implemented
                    0u64
                }),
                "promise_batch_then" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _account_id_len: u64, _account_id_ptr: u64| {
                    // Not implemented
                    0u64
                }),
                // Promise API actions
                "promise_batch_action_create_account" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64| {
                    // Not implemented
                }),
                "promise_batch_action_deploy_contract" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _code_len: u64, _code_ptr: u64| {
                    // Not implemented
                }),
                "promise_batch_action_function_call" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _method_name_len: u64, _method_name_ptr: u64, _arguments_len: u64, _arguments_ptr: u64, _amount_ptr: u64, _gas: u64| {
                    // Not implemented
                }),
                "promise_batch_action_transfer" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _amount_ptr: u64| {
                    // Not implemented
                }),
                "promise_batch_action_stake" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _amount_ptr: u64, _public_key_len: u64, _public_key_ptr: u64| {
                    // Not implemented
                }),
                "promise_batch_action_add_key_with_full_access" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _public_key_len: u64, _public_key_ptr: u64, _nonce: u64| {
                    // Not implemented
                }),
                "promise_batch_action_add_key_with_function_call" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _public_key_len: u64, _public_key_ptr: u64, _nonce: u64, _allowance_ptr: u64, _receiver_id_len: u64, _receiver_id_ptr: u64, _method_names_len: u64, _method_names_ptr: u64| {
                    // Not implemented
                }),
                "promise_batch_action_delete_key" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _public_key_len: u64, _public_key_ptr: u64| {
                    // Not implemented
                }),
                "promise_batch_action_delete_account" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _promise_index: u64, _beneficiary_id_len: u64, _beneficiary_id_ptr: u64| {
                    // Not implemented
                }),
                // Promise API results
                "promise_results_count" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>| env.data().state.promise_results_count()),
                "promise_result" => Function::new_typed_with_env(&mut store, &env, |env: FunctionEnvMut<WasmEnv>, result_idx: u64, register_id: u64| env.data().state.promise_result(result_idx, register_id)),
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
                    // Not implemented
                    0u64
                }),
                "storage_iter_range" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _from_len: u64, _from_ptr: u64, _to_len: u64, _to_ptr: u64| {
                    // Not implemented
                    0u64
                }),
                "storage_iter_next" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _iterator_id: u64, _key_register_id: u64, _value_register_id: u64| {
                    // Not implemented
                    0u64
                }),
                // Validator API
                "validator_stake" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _account_id_len: u64, _account_id_ptr: u64, _stake_ptr: u64| {
                    // Not implemented
                }),
                "validator_total_stake" => Function::new_typed_with_env(&mut store, &env, |_env: FunctionEnvMut<WasmEnv>, _stake_ptr: u64| {
                    // Not implemented
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

    pub fn set_code(&mut self, code: Vec<u8>) -> Result<(), WasmerRunnerError> {
        let module = Module::new(&self.store, code)?;
        let instance = Instance::new(&mut self.store, &module, &self.imports)?;
        let mut state = self.env.as_mut(&mut self.store).memory.lock();
        let memory = instance.exports.get_memory("memory")?.clone();
        *state = Some(memory);

        self.instance = Some(instance);
        Ok(())
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
    ) -> Result<(Diff, Vec<u8>), wasmer::RuntimeError> {
        {
            let state = &self.env.as_mut(&mut self.store).state;
            state.reset();
            state.init(
                method_name,
                trace_kind,
                block_height,
                transaction_position,
                input,
            );
            state.set_promise_handler(promise_data.into());
            state.set_env(env);
        }
        let execute = self
            .instance
            .as_ref()
            .unwrap()
            .exports
            .get_typed_function::<(), ()>(&self.store, "execute")
            .unwrap();
        execute.call(&mut self.store).map(|()| {
            let state = &self.env.as_mut(&mut self.store).state;
            let diff = state.get_transaction_diff();
            let output = state.take_output();
            (diff, output)
        })
    }
}

pub mod state {
    #![allow(clippy::as_conversions)]

    use std::{borrow::Cow, iter};

    use aurora_engine_sdk::env::Fixed;
    use aurora_engine_types::borsh;
    use engine_standalone_tracing::TraceKind;
    use parking_lot::Mutex;
    use sha2::digest::{FixedOutput, Update};
    use wasmer::MemoryView;

    use crate::{Diff, DiffValue, Storage};

    pub struct State {
        inner: Mutex<StateInner>,
        registers: Mutex<Vec<Register>>,
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
                inner: Mutex::new(StateInner::default()),
                registers: Mutex::new(
                    iter::repeat_with(Register::default)
                        .take(REGISTERS_NUMBER)
                        .collect(),
                ),
            }
        }
    }

    impl State {
        pub fn set_env(&self, env: Fixed) {
            self.inner.lock().env = Some(env);
        }

        pub fn set_promise_handler(&self, promise_data: Box<[Option<Vec<u8>>]>) {
            self.inner.lock().promise_data = promise_data;
        }

        #[must_use]
        pub fn take_output(&self) -> Vec<u8> {
            self.inner.lock().output.clone()
        }

        #[must_use]
        pub fn get_transaction_diff(&self) -> Diff {
            self.inner.lock().transaction_diff.clone()
        }

        pub fn init(
            &self,
            method_name: &str,
            trace_kind: Option<TraceKind>,
            block_height: u64,
            transaction_position: u16,
            input: Vec<u8>,
        ) {
            let mut lock = self.inner.lock();
            lock.bound_block_height = block_height;
            lock.bound_tx_position = transaction_position;
            lock.input = input;
            lock.transaction_diff.modify(
                b"borealis/method".to_vec(),
                borsh::to_vec(method_name).expect("must serialize string"),
            );
            if let Some(v) = &trace_kind {
                lock.transaction_diff.modify(
                    b"borealis/trace_kind".to_vec(),
                    borsh::to_vec(&v).expect("must serialize trivial enum"),
                );
            }
        }

        pub fn reset(&self) {
            *self.inner.lock() = StateInner::default();
            *self.registers.lock() = iter::repeat_with(Register::default)
                .take(REGISTERS_NUMBER)
                .collect();
        }

        #[allow(clippy::significant_drop_tightening)]
        fn read_reg<F, T>(&self, register_id: u64, mut op: F) -> T
        where
            F: FnMut(&Register) -> T,
        {
            let index = usize::try_from(register_id).expect("pointer size must be wide enough");
            let registers = self.registers.lock();
            let reg = registers
                .get(index)
                .unwrap_or_else(|| panic!("no such register {register_id}"));
            op(reg)
        }

        fn set_reg(&self, register_id: u64, data: Cow<[u8]>) {
            let index = usize::try_from(register_id).expect("pointer size must be wide enough");

            let mut registers = self.registers.lock();
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

        pub fn read_register(&self, memory: MemoryView<'_>, register_id: u64, ptr: u64) {
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

        pub fn current_account_id(&self, register_id: u64) {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            self.set_reg(register_id, env.current_account_id.as_bytes().into());
        }

        pub fn signer_account_id(&self, register_id: u64) {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            self.set_reg(register_id, env.signer_account_id.as_bytes().into());
        }

        pub fn predecessor_account_id(&self, register_id: u64) {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            self.set_reg(register_id, env.predecessor_account_id.as_bytes().into());
        }

        pub fn input(&self, register_id: u64) {
            let input = &self.inner.lock().input;
            self.set_reg(register_id, input.into());
        }

        pub fn block_index(&self) -> u64 {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            env.block_height
        }

        pub fn block_timestamp(&self) -> u64 {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            env.block_timestamp.nanos()
        }

        pub fn attached_deposit(&self, memory: MemoryView<'_>, balance_ptr: u64) {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            if let Err(err) = memory.write(balance_ptr, env.attached_deposit.to_ne_bytes().as_ref())
            {
                eprintln!("LOG: panic called from wasm: `attached_deposit` failed with: {err}");
            }
        }

        pub fn prepaid_gas(&self) -> u64 {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            env.prepaid_gas.as_u64()
        }

        pub fn used_gas(&self) -> u64 {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            env.used_gas.as_u64()
        }

        pub fn random_seed(&self, register_id: u64) {
            let Some(env) = &self.inner.lock().env else {
                panic!("environment is not set");
            };
            self.set_reg(register_id, env.random_seed.as_bytes().into());
        }

        pub fn digest<D: Default + Update + FixedOutput>(
            &self,
            memory: &MemoryView<'_>,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) {
            let data = self.get_data(memory, value_ptr, value_len);
            let hash = D::default().chain(data).finalize_fixed();
            self.set_reg(register_id, hash.as_slice().into());
        }

        pub fn ecrecover(
            &self,
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
            self.set_reg(register_id, public_key.serialize()[1..].into());

            Ok(())
        }

        pub fn value_return(&self, memory: &MemoryView<'_>, value_len: u64, value_ptr: u64) {
            let data = self.get_data(memory, value_ptr, value_len);
            self.inner.lock().output = data;
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
            u64::try_from(self.inner.lock().promise_data.len()).unwrap_or_default()
        }

        pub fn promise_result(&self, result_idx: u64, register_id: u64) -> u64 {
            let i = usize::try_from(result_idx).expect("index too big");
            let lock = self.inner.lock();
            let Some(data) = lock.promise_data.get(i) else {
                // not ready
                return 0;
            };
            let Some(data) = data else {
                // failed
                return 2;
            };
            // ready
            self.set_reg(register_id, data.as_slice().into());
            1
        }

        pub fn storage_write(
            &self,
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
                .lock()
                .transaction_diff
                .modify(key.to_vec(), value.to_vec());
            res
        }

        pub fn storage_read(
            &self,
            memory: &MemoryView<'_>,
            db: &Storage,
            key_len: u64,
            key_ptr: u64,
            register_id: u64,
        ) -> u64 {
            let key = self.get_data(memory, key_ptr, key_len);

            let lock = self.inner.lock();
            if let Some(diff) = lock.transaction_diff.get(&key) {
                return diff.value().map_or(0, |bytes| {
                    self.set_reg(register_id, bytes.into());
                    1
                });
            }

            if let Ok(value) = db.read_by_key(&key, lock.bound_block_height, lock.bound_tx_position)
            {
                return value.value().map_or(0, |bytes| {
                    self.set_reg(register_id, bytes.into());
                    1
                });
            }

            0
        }

        pub fn storage_remove(
            &self,
            memory: &MemoryView<'_>,
            db: &Storage,
            key_len: u64,
            key_ptr: u64,
            register_id: u64,
        ) -> u64 {
            // fetch original value into register
            let res = self.storage_read(memory, db, key_len, key_ptr, register_id);

            let key = self.get_data(memory, key_ptr, key_len);
            self.inner.lock().transaction_diff.delete(key);
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
            let lock = self.inner.lock();
            if let Some(value) = lock.transaction_diff.get(&key) {
                return matches!(value, DiffValue::Modified(..)).into();
            }

            db.read_by_key(&key, lock.bound_block_height, lock.bound_tx_position)
                .map_or(0, |diff| u64::from(diff.value().is_some()))
        }
    }
}
