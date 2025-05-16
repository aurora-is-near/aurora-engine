use near_crypto::PublicKey;
use near_primitives_core::hash::CryptoHash;
use near_primitives_core::types::GasWeight;
use near_vm_runner::logic::mocks::mock_external::MockedExternal;
use near_vm_runner::logic::types::{AccountId, Balance, Gas, ReceiptIndex};
use near_vm_runner::logic::{StorageAccessTracker, VMLogicError};
use std::cell::Cell;

/// Derived from mainnet data reported here: `https://hackmd.io/@birchmd/r1HRjr0P9`
/// Uses the formulas:
/// `n_T = (G_T / G_R) * (g_R / g_T)`
/// `n_c = (G_c / G_R) * (g_R / g_c)`
/// Where `n_T` is the average number of new touched trie nodes per read,
/// `n_c` is the average number of cached trie nodes read per read,
/// `G_T` is the average gas cost of touching trie node per Aurora transaction,
/// `G_c` is the average gas cost of reading cached trie node per Aurora transaction,
/// `G_R` is the average gas cost of `STORAGE_READ_BASE`  per Aurora transaction,
/// `g_R` is the `STORAGE_READ_BASE` cost (from the config),
/// `g_T` is the `TOUCHING_TRIE_NODE` cost (from the config), and
/// `g_c` is the `READ_CACHED_TRIE_NODE` cost (from the config).
pub const MAINNET_AVERAGE_TOUCHED_TRIE_PER_READ: u64 = 2;
pub const MAINNET_AVERAGE_READ_CACHED_TRIE_PER_READ: u64 = 11;
/// This is still needed because writes will touch every node in the depth, unlike reads which take advantage of caching.
pub const MAINNET_AVERAGE_TRIE_DEPTH: u64 = 13;

#[derive(Clone)]
pub struct MockedExternalWithTrie {
    pub underlying: MockedExternal,
    new_trie_node_count: Cell<u64>,
    cached_trie_node_count: Cell<u64>,
}

impl MockedExternalWithTrie {
    pub const fn new(ext: MockedExternal) -> Self {
        Self {
            underlying: ext,
            new_trie_node_count: Cell::new(0),
            cached_trie_node_count: Cell::new(0),
        }
    }

    fn increment_new_trie_node_count(&self, amount: u64) {
        let cell_value = self.new_trie_node_count.get();
        self.new_trie_node_count.set(cell_value + amount);
    }

    fn increment_cached_trie_node_count(&self, amount: u64) {
        let cell_value = self.cached_trie_node_count.get();
        self.cached_trie_node_count.set(cell_value + amount);
    }
}

impl near_vm_runner::logic::External for MockedExternalWithTrie {
    fn storage_set(
        &mut self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, VMLogicError> {
        self.increment_new_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_set(access_tracker, key, value)
    }

    fn storage_get<'a>(
        &'a self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<Option<Box<dyn near_vm_runner::logic::ValuePtr + 'a>>, VMLogicError> {
        self.increment_new_trie_node_count(MAINNET_AVERAGE_TOUCHED_TRIE_PER_READ);
        self.increment_cached_trie_node_count(MAINNET_AVERAGE_READ_CACHED_TRIE_PER_READ);
        self.underlying.storage_get(access_tracker, key)
    }

    fn storage_remove(
        &mut self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, VMLogicError> {
        self.increment_new_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_remove(access_tracker, key)
    }

    fn storage_has_key(
        &mut self,
        access_tracker: &mut dyn StorageAccessTracker,
        key: &[u8],
    ) -> Result<bool, VMLogicError> {
        self.underlying.storage_has_key(access_tracker, key)
    }

    fn generate_data_id(&mut self) -> CryptoHash {
        self.underlying.generate_data_id()
    }

    fn get_recorded_storage_size(&self) -> usize {
        self.underlying.get_recorded_storage_size()
    }

    fn validator_stake(&self, account_id: &AccountId) -> Result<Option<Balance>, VMLogicError> {
        self.underlying.validator_stake(account_id)
    }

    fn validator_total_stake(&self) -> Result<Balance, VMLogicError> {
        self.underlying.validator_total_stake()
    }

    fn create_action_receipt(
        &mut self,
        receipt_indices: Vec<ReceiptIndex>,
        receiver_id: AccountId,
    ) -> Result<ReceiptIndex, VMLogicError> {
        self.underlying
            .create_action_receipt(receipt_indices, receiver_id)
    }

    fn create_promise_yield_receipt(
        &mut self,
        receiver_id: AccountId,
    ) -> Result<(ReceiptIndex, CryptoHash), VMLogicError> {
        self.underlying.create_promise_yield_receipt(receiver_id)
    }

    fn submit_promise_resume_data(
        &mut self,
        data_id: CryptoHash,
        data: Vec<u8>,
    ) -> Result<bool, VMLogicError> {
        self.underlying.submit_promise_resume_data(data_id, data)
    }

    fn append_action_create_account(
        &mut self,
        receipt_index: ReceiptIndex,
    ) -> Result<(), VMLogicError> {
        self.underlying.append_action_create_account(receipt_index)
    }

    fn append_action_deploy_contract(
        &mut self,
        receipt_index: ReceiptIndex,
        code: Vec<u8>,
    ) -> Result<(), VMLogicError> {
        self.underlying
            .append_action_deploy_contract(receipt_index, code)
    }

    fn append_action_function_call_weight(
        &mut self,
        receipt_index: ReceiptIndex,
        method_name: Vec<u8>,
        args: Vec<u8>,
        attached_deposit: Balance,
        prepaid_gas: Gas,
        gas_weight: GasWeight,
    ) -> Result<(), VMLogicError> {
        self.underlying.append_action_function_call_weight(
            receipt_index,
            method_name,
            args,
            attached_deposit,
            prepaid_gas,
            gas_weight,
        )
    }

    fn append_action_transfer(
        &mut self,
        receipt_index: ReceiptIndex,
        amount: Balance,
    ) -> Result<(), VMLogicError> {
        self.underlying
            .append_action_transfer(receipt_index, amount)
    }

    fn append_action_stake(
        &mut self,
        receipt_index: ReceiptIndex,
        stake: Balance,
        public_key: PublicKey,
    ) {
        self.underlying
            .append_action_stake(receipt_index, stake, public_key);
    }

    fn append_action_add_key_with_full_access(
        &mut self,
        receipt_index: ReceiptIndex,
        public_key: PublicKey,
        nonce: u64,
    ) {
        self.underlying
            .append_action_add_key_with_full_access(receipt_index, public_key, nonce);
    }

    fn append_action_add_key_with_function_call(
        &mut self,
        receipt_index: ReceiptIndex,
        public_key: PublicKey,
        nonce: u64,
        allowance: Option<Balance>,
        receiver_id: AccountId,
        method_names: Vec<Vec<u8>>,
    ) -> Result<(), VMLogicError> {
        self.underlying.append_action_add_key_with_function_call(
            receipt_index,
            public_key,
            nonce,
            allowance,
            receiver_id,
            method_names,
        )
    }

    fn append_action_delete_key(&mut self, receipt_index: ReceiptIndex, public_key: PublicKey) {
        self.underlying
            .append_action_delete_key(receipt_index, public_key);
    }

    fn append_action_delete_account(
        &mut self,
        receipt_index: ReceiptIndex,
        beneficiary_id: AccountId,
    ) -> Result<(), VMLogicError> {
        self.underlying
            .append_action_delete_account(receipt_index, beneficiary_id)
    }

    fn get_receipt_receiver(&self, receipt_index: ReceiptIndex) -> &AccountId {
        self.underlying.get_receipt_receiver(receipt_index)
    }
}
