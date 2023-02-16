use aurora_engine_types::H256;
use aurora_engine_sdk::keccak;

/// Block Hashchain
/// The order of operations should be:
/// 1. Create the BlockHashchain one time.
/// 2. Add transactions of the current block.
/// 3. Compute the block hashchain for the current block once all the transactions were added.
/// 4. Clear the transactions of the current block.
/// 5. Go back to step 2 for the next block.
struct BlockHashchain {
    contract_name_hash: H256,
    txs_merkle_tree: StreamCompactMerkleTree
}

impl BlockHashchain {
    pub fn new(contract_name: &str) -> Self {
        Self {
            contract_name_hash: keccak(contract_name.as_bytes()),
            txs_merkle_tree: StreamCompactMerkleTree::new()
        }
    }

    /// Adds a transaction.
    pub fn add_tx(&mut self, method_name: &str, input: &[u8], output: &[u8]) {
        let method_name_hash = keccak(method_name.as_bytes());
        let input_hash = keccak(input);
        let output_hash = keccak(output);

        let tx_hash = keccak(&[method_name_hash.as_bytes(), input_hash.as_bytes(), output_hash.as_bytes()].concat());

        self.txs_merkle_tree.add(tx_hash);
    }

    /// Computes the block hashchain.
    /// Uses the added transactions and the parameters.
    pub fn compute_block_hashchain(&self, block_height: u64, previous_block_hashchain: H256) -> H256 {
        let block_height_hash = keccak(&block_height.to_be_bytes());
        let txs_hash = self.txs_merkle_tree.compute_hash();
        
        keccak(&[self.contract_name_hash.as_bytes(), block_height_hash.as_bytes(), previous_block_hashchain.as_bytes(), txs_hash.as_bytes()].concat())
    }

    /// Clears the transactions added.
    pub fn clear_txs(&mut self) {
        self.txs_merkle_tree.clear();
    }
}

/// Stream Compact Merkle Tree
/// It can be feed by a stream of hashes (leafs) adding them to the right of the tree.
/// Internally, compacts full binary subtrees mantaining only the growing branch.
/// Space used is O(log n) where n is the number of leaf hashes added.
struct StreamCompactMerkleTree {
    /// Complete binary merkle subtrees.
    /// Left subtrees are strictly higher (bigger).
    /// Subtrees are compacted right to left when two consecutives have same height.
    subtrees: Vec<CompactMerkleSubtree>,
}

impl StreamCompactMerkleTree {
    pub fn new() -> Self {
        Self {
            subtrees: Vec::new(),
        }
    }

    /// Adds a leaf hash to the right of the tree.
    /// For n leaf hashes added, a single call to this function is O(log n),
    /// but the amortized time for the n calls is O(1).
    pub fn add(&mut self, leaf_hash: H256) {
        // add new leaf to the right as is own subtree
        let leaf_subtree = CompactMerkleSubtree {
            height: 1,
            hash: leaf_hash,
        };
        self.subtrees.push(leaf_subtree);

        // compact subtrees from right to left
        let mut index = &self.subtrees.len() - 1;

        while index >= 1 {
            let right_subtree = &self.subtrees[index];
            let left_subtree = &self.subtrees[index - 1];

            // same height means they are sibilings so we can compact them
            if left_subtree.height == right_subtree.height {
                let father_subtree = CompactMerkleSubtree {
                    height: left_subtree.height + 1,
                    hash: keccak(&[left_subtree.hash.as_bytes(), right_subtree.hash.as_bytes()].concat())
                };

                self.subtrees.pop();
                self.subtrees.pop();
                self.subtrees.push(father_subtree);

                index = index - 1;
            }
            // all remaining subtrees have different heights so we can't compact anything else
            else {
                break;
            }
        }
    }

    /// Computes the hash of the Merkle Tree.
    /// For n leaf hashes added, this function is O(log n).
    pub fn compute_hash(&self) -> H256 {
        // emtpy tree
        if self.subtrees.len() == 0 {
            return H256::zero();
        }

        // compute hash compacting or duplicating subtrees hashes from right to left
        let mut index = &self.subtrees.len() - 1;
        let mut right_subtree = CompactMerkleSubtree {
            ..self.subtrees[index]
        };

        while index >= 1 {
            let left_subtree = &self.subtrees[index - 1];

            // same height means they are siblings so we can compact hashes
            if left_subtree.height == right_subtree.height {
                right_subtree.hash = keccak(&[left_subtree.hash.as_bytes(), right_subtree.hash.as_bytes()].concat());
                index = index - 1;
            }
            // left_subtree is higher so we need to duplicate right_subtree to grow up (standard mechanism for unbalanced merkle trees)
            else {
                right_subtree.hash = keccak(&[right_subtree.hash.as_bytes(), right_subtree.hash.as_bytes()].concat());
            }

            right_subtree.height = right_subtree.height + 1;
        }

        right_subtree.hash
    }

    /// Clears the structure leaving it empty.
    pub fn clear(&mut self) {
        self.subtrees.clear();
    }
}

/// Compact Merkle Subtree
/// For leafs, this represents only the leaf node with height 1 and the hash of the leaf.
/// For bigger subtrees, this represents the entire balanced subtree with its height and merkle hash.
struct CompactMerkleSubtree {
    /// Height of the subtree.
    height: u8,

    /// Merkle tree hash of the subtree.
    hash: H256,
}