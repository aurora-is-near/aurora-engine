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
}

impl crate::io::StorageIntermediate for RegisterIndex {
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
        fn random_seed(register_id: u64);
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
