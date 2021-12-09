use crate::io::StorageIntermediate;
use crate::prelude::NearGas;
use crate::promise::PromiseId;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::{PromiseAction, PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::types::PromiseResult;
use aurora_engine_types::{TryFrom, H256};

/// Wrapper type for indices in NEAR's register API.
pub struct RegisterIndex(u64);

/// Singleton type used to implement the IO traits in the case of using NEAR's
/// runtime (i.e. for wasm contracts).
#[derive(Copy, Clone, Default)]
pub struct Runtime;

impl Runtime {
    const READ_STORAGE_REGISTER_ID: RegisterIndex = RegisterIndex(0);
    const INPUT_REGISTER_ID: RegisterIndex = RegisterIndex(1);
    const WRITE_REGISTER_ID: RegisterIndex = RegisterIndex(2);
    const EVICT_REGISTER_ID: RegisterIndex = RegisterIndex(3);
    const ENV_REGISTER_ID: RegisterIndex = RegisterIndex(4);
    const PROMISE_REGISTER_ID: RegisterIndex = RegisterIndex(5);

    const GAS_FOR_STATE_MIGRATION: NearGas = NearGas::new(100_000_000_000_000);

    /// Deploy code from given key in place of the current contract.
    /// Not implemented in terms of higher level traits (eg IO) for efficiency reasons.
    pub fn self_deploy(code_key: &[u8]) {
        unsafe {
            // Load current account id into register 0.
            exports::current_account_id(0);
            // Use register 0 as the destination for the promise.
            let promise_id = exports::promise_batch_create(u64::MAX as _, 0);
            // Remove code from storage and store it in register 1.
            exports::storage_remove(code_key.len() as _, code_key.as_ptr() as _, 1);
            exports::promise_batch_action_deploy_contract(promise_id, u64::MAX, 1);
            Self::promise_batch_action_function_call(
                promise_id,
                b"state_migration",
                &[],
                0,
                Self::GAS_FOR_STATE_MIGRATION.into_u64(),
            )
        }
    }

    /// Assumes a valid account ID has been written to ENV_REGISTER_ID
    /// by a previous call.
    fn read_account_id() -> AccountId {
        let bytes = Self::ENV_REGISTER_ID.to_vec();
        match AccountId::try_from(bytes) {
            Ok(account_id) => account_id,
            // the environment must give us a valid Account ID.
            Err(_) => unreachable!(),
        }
    }

    /// Convenience wrapper around `exports::promise_batch_action_function_call`
    fn promise_batch_action_function_call(
        promise_idx: u64,
        method_name: &[u8],
        arguments: &[u8],
        amount: u128,
        gas: u64,
    ) {
        unsafe {
            exports::promise_batch_action_function_call(
                promise_idx,
                method_name.len() as _,
                method_name.as_ptr() as _,
                arguments.len() as _,
                arguments.as_ptr() as _,
                &amount as *const u128 as _,
                gas,
            )
        }
    }
}

impl StorageIntermediate for RegisterIndex {
    fn len(&self) -> usize {
        unsafe {
            let result = exports::register_len(self.0);
            // By convention, an unused register will return a length of U64::MAX
            // (see https://nomicon.io/RuntimeSpec/Components/BindingsSpec/RegistersAPI.html).
            if result < u64::MAX {
                result as usize
            } else {
                0
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        unsafe { exports::read_register(self.0, buffer.as_ptr() as u64) }
    }
}

impl crate::io::IO for Runtime {
    type StorageValue = RegisterIndex;

    fn read_input(&self) -> Self::StorageValue {
        unsafe {
            exports::input(Runtime::INPUT_REGISTER_ID.0);
        }
        Runtime::INPUT_REGISTER_ID
    }

    fn return_output(&mut self, value: &[u8]) {
        unsafe {
            exports::value_return(value.len() as u64, value.as_ptr() as u64);
        }
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_read(
                key.len() as u64,
                key.as_ptr() as u64,
                Runtime::READ_STORAGE_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::READ_STORAGE_REGISTER_ID)
            } else {
                None
            }
        }
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        unsafe { exports::storage_has_key(key.len() as _, key.as_ptr() as _) == 1 }
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_write(
                key.len() as u64,
                key.as_ptr() as u64,
                value.len() as u64,
                value.as_ptr() as u64,
                Runtime::WRITE_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::WRITE_REGISTER_ID)
            } else {
                None
            }
        }
    }

    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_write(
                key.len() as _,
                key.as_ptr() as _,
                u64::MAX,
                value.0,
                Runtime::WRITE_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::WRITE_REGISTER_ID)
            } else {
                None
            }
        }
    }

    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_remove(
                key.len() as _,
                key.as_ptr() as _,
                Runtime::EVICT_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::EVICT_REGISTER_ID)
            } else {
                None
            }
        }
    }
}

impl crate::env::Env for Runtime {
    fn signer_account_id(&self) -> AccountId {
        unsafe {
            exports::signer_account_id(Self::ENV_REGISTER_ID.0);
        }
        Self::read_account_id()
    }

    fn current_account_id(&self) -> AccountId {
        unsafe {
            exports::current_account_id(Self::ENV_REGISTER_ID.0);
        }
        Self::read_account_id()
    }

    fn predecessor_account_id(&self) -> AccountId {
        unsafe {
            exports::predecessor_account_id(Self::ENV_REGISTER_ID.0);
        }
        Self::read_account_id()
    }

    fn block_height(&self) -> u64 {
        unsafe { exports::block_index() }
    }

    fn block_timestamp(&self) -> crate::env::Timestamp {
        let ns = unsafe { exports::block_timestamp() };
        crate::env::Timestamp::new(ns)
    }

    fn attached_deposit(&self) -> u128 {
        unsafe {
            let data = [0u8; core::mem::size_of::<u128>()];
            exports::attached_deposit(data.as_ptr() as u64);
            u128::from_le_bytes(data)
        }
    }

    fn random_seed(&self) -> H256 {
        unsafe {
            exports::random_seed(0);
            let bytes = H256::zero();
            exports::read_register(0, bytes.0.as_ptr() as *const u64 as u64);
            bytes
        }
    }

    fn prepaid_gas(&self) -> u64 {
        unsafe { exports::prepaid_gas() }
    }
}

impl crate::promise::PromiseHandler for Runtime {
    fn promise_results_count(&self) -> u64 {
        unsafe { exports::promise_results_count() }
    }

    fn promise_result(&self, index: u64) -> Option<PromiseResult> {
        unsafe {
            match exports::promise_result(index, Self::PROMISE_REGISTER_ID.0) {
                0 => Some(PromiseResult::NotReady),
                1 => {
                    let bytes = Self::PROMISE_REGISTER_ID.to_vec();
                    Some(PromiseResult::Successful(bytes))
                }
                2 => Some(PromiseResult::Failed),
                _ => None,
            }
        }
    }

    fn promise_create_call(&mut self, args: &PromiseCreateArgs) -> PromiseId {
        let account_id = args.target_account_id.as_bytes();
        let method_name = args.method.as_bytes();
        let arguments = args.args.as_slice();
        let amount = args.attached_balance;
        let gas = args.attached_gas;

        let id = unsafe {
            exports::promise_create(
                account_id.len() as _,
                account_id.as_ptr() as _,
                method_name.len() as _,
                method_name.as_ptr() as _,
                arguments.len() as _,
                arguments.as_ptr() as _,
                &amount as *const u128 as _,
                gas,
            )
        };
        PromiseId::new(id)
    }

    fn promise_attach_callback(
        &mut self,
        base: PromiseId,
        callback: &PromiseCreateArgs,
    ) -> PromiseId {
        let account_id = callback.target_account_id.as_bytes();
        let method_name = callback.method.as_bytes();
        let arguments = callback.args.as_slice();
        let amount = callback.attached_balance;
        let gas = callback.attached_gas;

        let id = unsafe {
            exports::promise_then(
                base.raw(),
                account_id.len() as _,
                account_id.as_ptr() as _,
                method_name.len() as _,
                method_name.as_ptr() as _,
                arguments.len() as _,
                arguments.as_ptr() as _,
                &amount as *const u128 as _,
                gas,
            )
        };

        PromiseId::new(id)
    }

    fn promise_create_batch(&mut self, args: &PromiseBatchAction) -> PromiseId {
        let account_id = args.target_account_id.as_bytes();

        let id = unsafe {
            exports::promise_batch_create(account_id.len() as _, account_id.as_ptr() as _)
        };

        for action in args.actions.iter() {
            match action {
                PromiseAction::Transfer { amount } => unsafe {
                    let amount = *amount;
                    exports::promise_batch_action_transfer(id, &amount as *const u128 as _);
                },
                PromiseAction::DeployConotract { code } => unsafe {
                    let code = code.as_slice();
                    exports::promise_batch_action_deploy_contract(
                        id,
                        code.len() as _,
                        code.as_ptr() as _,
                    );
                },
                PromiseAction::FunctionCall {
                    name,
                    gas,
                    attached_yocto,
                    args,
                } => unsafe {
                    let method_name = name.as_bytes();
                    let arguments = args.as_slice();
                    let amount = *attached_yocto;
                    exports::promise_batch_action_function_call(
                        id,
                        method_name.len() as _,
                        method_name.as_ptr() as _,
                        arguments.len() as _,
                        arguments.as_ptr() as _,
                        &amount as *const u128 as _,
                        *gas,
                    )
                },
            }
        }

        PromiseId::new(id)
    }

    fn promise_return(&mut self, promise: PromiseId) {
        unsafe {
            exports::promise_return(promise.raw());
        }
    }
}

pub(crate) mod exports {
    #[allow(unused)]
    extern "C" {
        // #############
        // # Registers #
        // #############
        pub(crate) fn read_register(register_id: u64, ptr: u64);
        pub(crate) fn register_len(register_id: u64) -> u64;
        // ###############
        // # Context API #
        // ###############
        pub(crate) fn current_account_id(register_id: u64);
        pub(crate) fn signer_account_id(register_id: u64);
        pub(crate) fn signer_account_pk(register_id: u64);
        pub(crate) fn predecessor_account_id(register_id: u64);
        pub(crate) fn input(register_id: u64);
        // TODO #1903 fn block_height() -> u64;
        pub(crate) fn block_index() -> u64;
        pub(crate) fn block_timestamp() -> u64;
        fn epoch_height() -> u64;
        pub(crate) fn storage_usage() -> u64;
        // #################
        // # Economics API #
        // #################
        fn account_balance(balance_ptr: u64);
        pub(crate) fn attached_deposit(balance_ptr: u64);
        pub(crate) fn prepaid_gas() -> u64;
        fn used_gas() -> u64;
        // ############
        // # Math API #
        // ############
        pub(crate) fn random_seed(register_id: u64);
        pub(crate) fn sha256(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn keccak256(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn ripemd160(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn ecrecover(
            hash_len: u64,
            hash_ptr: u64,
            sig_len: u64,
            sig_ptr: u64,
            v: u64,
            malleability_flag: u64,
            register_id: u64,
        ) -> u64;
        // #####################
        // # Miscellaneous API #
        // #####################
        pub(crate) fn value_return(value_len: u64, value_ptr: u64);
        pub(crate) fn panic();
        pub(crate) fn panic_utf8(len: u64, ptr: u64);
        pub(crate) fn log_utf8(len: u64, ptr: u64);
        fn log_utf16(len: u64, ptr: u64);
        fn abort(msg_ptr: u32, filename_ptr: u32, line: u32, col: u32);
        // ################
        // # Promises API #
        // ################
        pub(crate) fn promise_create(
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        pub(crate) fn promise_then(
            promise_index: u64,
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        fn promise_and(promise_idx_ptr: u64, promise_idx_count: u64) -> u64;
        pub(crate) fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64;
        fn promise_batch_then(promise_index: u64, account_id_len: u64, account_id_ptr: u64) -> u64;
        // #######################
        // # Promise API actions #
        // #######################
        fn promise_batch_action_create_account(promise_index: u64);
        pub(crate) fn promise_batch_action_deploy_contract(
            promise_index: u64,
            code_len: u64,
            code_ptr: u64,
        );
        pub(crate) fn promise_batch_action_function_call(
            promise_index: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        );
        pub(crate) fn promise_batch_action_transfer(promise_index: u64, amount_ptr: u64);
        fn promise_batch_action_stake(
            promise_index: u64,
            amount_ptr: u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
        fn promise_batch_action_add_key_with_full_access(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
            nonce: u64,
        );
        fn promise_batch_action_add_key_with_function_call(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
            nonce: u64,
            allowance_ptr: u64,
            receiver_id_len: u64,
            receiver_id_ptr: u64,
            method_names_len: u64,
            method_names_ptr: u64,
        );
        fn promise_batch_action_delete_key(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
        fn promise_batch_action_delete_account(
            promise_index: u64,
            beneficiary_id_len: u64,
            beneficiary_id_ptr: u64,
        );
        // #######################
        // # Promise API results #
        // #######################
        pub(crate) fn promise_results_count() -> u64;
        pub(crate) fn promise_result(result_idx: u64, register_id: u64) -> u64;
        pub(crate) fn promise_return(promise_id: u64);
        // ###############
        // # Storage API #
        // ###############
        pub(crate) fn storage_write(
            key_len: u64,
            key_ptr: u64,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) -> u64;
        pub(crate) fn storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
        pub(crate) fn storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
        pub(crate) fn storage_has_key(key_len: u64, key_ptr: u64) -> u64;
        fn storage_iter_prefix(prefix_len: u64, prefix_ptr: u64) -> u64;
        fn storage_iter_range(start_len: u64, start_ptr: u64, end_len: u64, end_ptr: u64) -> u64;
        fn storage_iter_next(iterator_id: u64, key_register_id: u64, value_register_id: u64)
            -> u64;
        // ###############
        // # Validator API #
        // ###############
        fn validator_stake(account_id_len: u64, account_id_ptr: u64, stake_ptr: u64);
        fn validator_total_stake(stake_ptr: u64);
    }
}
