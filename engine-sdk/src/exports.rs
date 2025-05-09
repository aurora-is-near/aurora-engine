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
    pub(crate) fn used_gas() -> u64;
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
    pub(crate) fn alt_bn128_g1_sum(value_len: u64, value_ptr: u64, register_id: u64);
    pub(crate) fn alt_bn128_g1_multiexp(value_len: u64, value_ptr: u64, register_id: u64);
    pub(crate) fn alt_bn128_pairing_check(value_len: u64, value_ptr: u64) -> u64;
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
    pub(crate) fn promise_and(promise_idx_ptr: u64, promise_idx_count: u64) -> u64;
    pub(crate) fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64;
    pub(crate) fn promise_batch_then(
        promise_index: u64,
        account_id_len: u64,
        account_id_ptr: u64,
    ) -> u64;
    // #######################
    // # Promise API actions #
    // #######################
    pub(crate) fn promise_batch_action_create_account(promise_index: u64);
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
    pub(crate) fn promise_batch_action_stake(
        promise_index: u64,
        amount_ptr: u64,
        public_key_len: u64,
        public_key_ptr: u64,
    );
    pub(crate) fn promise_batch_action_add_key_with_full_access(
        promise_index: u64,
        public_key_len: u64,
        public_key_ptr: u64,
        nonce: u64,
    );
    pub(crate) fn promise_batch_action_add_key_with_function_call(
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
    pub(crate) fn promise_batch_action_delete_key(
        promise_index: u64,
        public_key_len: u64,
        public_key_ptr: u64,
    );
    pub(crate) fn promise_batch_action_delete_account(
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
    fn storage_iter_next(iterator_id: u64, key_register_id: u64, value_register_id: u64) -> u64;
    // ###############
    // # Validator API #
    // ###############
    fn validator_stake(account_id_len: u64, account_id_ptr: u64, stake_ptr: u64);
    fn validator_total_stake(stake_ptr: u64);
}
