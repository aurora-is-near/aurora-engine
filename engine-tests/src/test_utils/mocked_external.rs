use near_vm_logic::mocks::mock_external::MockedExternal;

pub const MAINNET_AVERAGE_TRIE_DEPTH: u64 = 10;

#[derive(Clone)]
pub(crate) struct MockedExternalWithTrie {
    pub underlying: MockedExternal,
    trie_node_count: std::cell::Cell<u64>,
}

impl MockedExternalWithTrie {
    pub fn new(ext: MockedExternal) -> Self {
        Self {
            underlying: ext,
            trie_node_count: std::cell::Cell::new(0),
        }
    }

    fn increment_trie_node_count(&self, amount: u64) {
        let cell_value = self.trie_node_count.get();
        self.trie_node_count.set(cell_value + amount);
    }
}

impl near_vm_logic::External for MockedExternalWithTrie {
    fn storage_set(&mut self, key: &[u8], value: &[u8]) -> Result<(), near_vm_logic::VMLogicError> {
        self.increment_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_set(key, value)
    }

    fn storage_get<'a>(
        &'a self,
        key: &[u8],
    ) -> Result<Option<Box<dyn near_vm_logic::ValuePtr + 'a>>, near_vm_logic::VMLogicError> {
        self.increment_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_get(key)
    }

    fn storage_remove(&mut self, key: &[u8]) -> Result<(), near_vm_logic::VMLogicError> {
        self.increment_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_remove(key)
    }

    fn storage_remove_subtree(&mut self, prefix: &[u8]) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying.storage_remove_subtree(prefix)
    }

    fn storage_has_key(&mut self, key: &[u8]) -> Result<bool, near_vm_logic::VMLogicError> {
        self.underlying.storage_has_key(key)
    }

    fn create_receipt(
        &mut self,
        receipt_indices: Vec<near_vm_logic::types::ReceiptIndex>,
        receiver_id: near_primitives::types::AccountId,
    ) -> Result<near_vm_logic::types::ReceiptIndex, near_vm_logic::VMLogicError> {
        self.underlying.create_receipt(receipt_indices, receiver_id)
    }

    fn append_action_create_account(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying.append_action_create_account(receipt_index)
    }

    fn append_action_deploy_contract(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        code: Vec<u8>,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying
            .append_action_deploy_contract(receipt_index, code)
    }

    fn append_action_function_call(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        method_name: Vec<u8>,
        arguments: Vec<u8>,
        attached_deposit: near_primitives::types::Balance,
        prepaid_gas: near_primitives::types::Gas,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying.append_action_function_call(
            receipt_index,
            method_name,
            arguments,
            attached_deposit,
            prepaid_gas,
        )
    }

    fn append_action_transfer(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        amount: near_primitives::types::Balance,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying
            .append_action_transfer(receipt_index, amount)
    }

    fn append_action_stake(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        stake: near_primitives::types::Balance,
        public_key: near_vm_logic::types::PublicKey,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying
            .append_action_stake(receipt_index, stake, public_key)
    }

    fn append_action_add_key_with_full_access(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        public_key: near_vm_logic::types::PublicKey,
        nonce: u64,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying
            .append_action_add_key_with_full_access(receipt_index, public_key, nonce)
    }

    fn append_action_add_key_with_function_call(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        public_key: near_vm_logic::types::PublicKey,
        nonce: u64,
        allowance: Option<near_primitives::types::Balance>,
        receiver_id: near_primitives::types::AccountId,
        method_names: Vec<Vec<u8>>,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying.append_action_add_key_with_function_call(
            receipt_index,
            public_key,
            nonce,
            allowance,
            receiver_id,
            method_names,
        )
    }

    fn append_action_delete_key(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        public_key: near_vm_logic::types::PublicKey,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying
            .append_action_delete_key(receipt_index, public_key)
    }

    fn append_action_delete_account(
        &mut self,
        receipt_index: near_vm_logic::types::ReceiptIndex,
        beneficiary_id: near_primitives::types::AccountId,
    ) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying
            .append_action_delete_account(receipt_index, beneficiary_id)
    }

    fn get_touched_nodes_count(&self) -> u64 {
        self.trie_node_count.get()
    }

    fn validator_stake(
        &self,
        account_id: &near_primitives::types::AccountId,
    ) -> Result<Option<near_primitives::types::Balance>, near_vm_logic::VMLogicError> {
        self.underlying.validator_stake(account_id)
    }

    fn validator_total_stake(
        &self,
    ) -> Result<near_primitives::types::Balance, near_vm_logic::VMLogicError> {
        self.underlying.validator_total_stake()
    }
}
