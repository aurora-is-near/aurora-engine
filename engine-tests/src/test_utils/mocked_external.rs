use near_vm_logic::mocks::mock_external::MockedExternal;

/// Derived from mainnet data reported here: https://hackmd.io/@birchmd/r1HRjr0P9
/// Uses the formulas:
/// n_T = (G_T / G_R) * (g_R / g_T)
/// n_c = (G_c / G_R) * (g_R / g_c)
/// Where n_T is the average number of new touched trie nodes per read,
/// n_c is the average number of cached trie nodes read per read,
/// G_T is the average gas cost of touching trie node per Aurora transaction,
/// G_c is the average gas cost of reading cached trie node per Aurora transaction,
/// G_R is the average gas cost of `STORAGE_READ_BASE`  per Aurora transaction,
/// g_R is the `STORAGE_READ_BASE` cost (from the config),
/// g_T is the `TOUCHING_TRIE_NODE` cost (from the config), and
/// g_c is the `READ_CACHED_TRIE_NODE` cost (from the config).
pub const MAINNET_AVERAGE_TOUCHED_TRIE_PER_READ: u64 = 2;
pub const MAINNET_AVERAGE_READ_CACHED_TRIE_PER_READ: u64 = 11;
/// This is still needed because writes will touch every node in the depth, unlike reads which take advantage of caching.
pub const MAINNET_AVERAGE_TRIE_DEPTH: u64 = 13;

#[derive(Clone)]
pub struct MockedExternalWithTrie {
    pub underlying: MockedExternal,
    new_trie_node_count: std::cell::Cell<u64>,
    cached_trie_node_count: std::cell::Cell<u64>,
}

impl MockedExternalWithTrie {
    pub fn new(ext: MockedExternal) -> Self {
        Self {
            underlying: ext,
            new_trie_node_count: std::cell::Cell::new(0),
            cached_trie_node_count: std::cell::Cell::new(0),
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

impl near_vm_logic::External for MockedExternalWithTrie {
    fn storage_set(&mut self, key: &[u8], value: &[u8]) -> Result<(), near_vm_logic::VMLogicError> {
        self.increment_new_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_set(key, value)
    }

    fn storage_get<'a>(
        &'a self,
        key: &[u8],
    ) -> Result<Option<Box<dyn near_vm_logic::ValuePtr + 'a>>, near_vm_logic::VMLogicError> {
        self.increment_new_trie_node_count(MAINNET_AVERAGE_TOUCHED_TRIE_PER_READ);
        self.increment_cached_trie_node_count(MAINNET_AVERAGE_READ_CACHED_TRIE_PER_READ);
        self.underlying.storage_get(key)
    }

    fn storage_remove(&mut self, key: &[u8]) -> Result<(), near_vm_logic::VMLogicError> {
        self.increment_new_trie_node_count(MAINNET_AVERAGE_TRIE_DEPTH);
        self.underlying.storage_remove(key)
    }

    fn storage_remove_subtree(&mut self, prefix: &[u8]) -> Result<(), near_vm_logic::VMLogicError> {
        self.underlying.storage_remove_subtree(prefix)
    }

    fn storage_has_key(&mut self, key: &[u8]) -> Result<bool, near_vm_logic::VMLogicError> {
        self.underlying.storage_has_key(key)
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

    fn generate_data_id(&mut self) -> near_primitives::hash::CryptoHash {
        self.underlying.generate_data_id()
    }

    fn get_trie_nodes_count(&self) -> near_primitives::types::TrieNodesCount {
        let db_reads = self.new_trie_node_count.get();
        let mem_reads = self.cached_trie_node_count.get();
        near_primitives::types::TrieNodesCount {
            db_reads,
            mem_reads,
        }
    }
}
