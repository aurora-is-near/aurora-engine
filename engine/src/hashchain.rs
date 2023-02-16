use aurora_engine_sdk::keccak;
use aurora_engine_types::types::RawH256;

/// Block Hashchain
/// The order of operations should be:
/// 1. Create the BlockHashchain one time.
/// 2. Add transactions of the current block.
/// 3. Compute the block hashchain for the current block once all the transactions were added.
/// 4. Clear the transactions of the current block.
/// 5. Go back to step 2 for the next block.
struct BlockHashchain {
    contract_name_hash: RawH256,
    txs_merkle_tree: StreamCompactMerkleTree
}

impl BlockHashchain {
    pub fn new(contract_name: &str) -> Self {
        Self {
            contract_name_hash: keccak(contract_name.as_bytes()).0,
            txs_merkle_tree: StreamCompactMerkleTree::new()
        }
    }

    /// Adds a transaction.
    pub fn add_tx(&mut self, method_name: &str, input: &[u8], output: &[u8]) {
        let method_name_hash = keccak(method_name.as_bytes());
        let input_hash = keccak(input);
        let output_hash = keccak(output);

        let tx_hash = keccak(&[method_name_hash.as_bytes(), input_hash.as_bytes(), output_hash.as_bytes()].concat()).0;

        self.txs_merkle_tree.add(tx_hash);
    }

    /// Computes the block hashchain.
    /// Uses the added transactions and the parameters.
    pub fn compute_block_hashchain(&self, block_height: u64, previous_block_hashchain: RawH256) -> RawH256 {
        let block_height_hash = keccak(&block_height.to_be_bytes());
        let txs_hash = self.txs_merkle_tree.compute_hash();
        
        keccak(&[&self.contract_name_hash, block_height_hash.as_bytes(), &previous_block_hashchain, &txs_hash].concat()).0
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
    pub fn add(&mut self, leaf_hash: RawH256) {
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
                    hash: keccak(&[left_subtree.hash, right_subtree.hash].concat()).0
                };

                self.subtrees.pop();
                self.subtrees.pop();
                self.subtrees.push(father_subtree);

                index -= 1;
            }
            // all remaining subtrees have different heights so we can't compact anything else
            else {
                break;
            }
        }
    }

    /// Computes the hash of the Merkle Tree.
    /// For n leaf hashes added, this function is O(log n).
    pub fn compute_hash(&self) -> RawH256 {
        if self.subtrees.is_empty() {
            return [0; 32];
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
                right_subtree.hash = keccak(&[left_subtree.hash, right_subtree.hash].concat()).0;
                index -= 1;
            }
            // left_subtree is higher so we need to duplicate right_subtree to grow up (standard mechanism for unbalanced merkle trees)
            else {
                right_subtree.hash = keccak(&[right_subtree.hash, right_subtree.hash].concat()).0;
            }

            right_subtree.height += 1;
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
    hash: RawH256,
}

#[cfg(test)]
mod StreamCompactMerkleTree_tests {
    use super::*;

    #[test]
    fn empty_tree() {
        let merkle_tree = StreamCompactMerkleTree::new();

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 0);
        assert_eq!(merkle_tree_hash, [0; 32]);
    }

    #[test]
    fn one_leaf_tree() {
        let one_hash = hash(1);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 1);
        assert_eq!(merkle_tree.subtrees[0].hash, one_hash);
        assert_eq!(merkle_tree_hash, one_hash);
    }

    #[test]
    fn two_leaf_tree() {
        let one_hash = hash(1);
        let two_hash = hash(2);

        let expected_merkle_tree_hash = keccak(&[one_hash, two_hash].concat()).0;

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 1, "One and two should be compacted into a single subtree.");
        assert_eq!(merkle_tree.subtrees[0].hash, expected_merkle_tree_hash);
        assert_eq!(merkle_tree_hash, expected_merkle_tree_hash);
    }

    #[test]
    fn three_leaf_tree() {
        let one_hash = hash(1);
        let two_hash = hash(2);
        let three_hash = hash(3);

        let expected_left_subtree_hash = hash_concatenation(one_hash, two_hash);
        let expected_right_subtree_hash_computation = hash_concatenation(three_hash, three_hash);
        let expected_merkle_tree_hash = hash_concatenation(expected_left_subtree_hash, expected_right_subtree_hash_computation);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 2, "One and two should be compacted into a single subtree, and three should be alone.");
        assert_eq!(merkle_tree.subtrees[0].hash, expected_left_subtree_hash);
        assert_eq!(merkle_tree.subtrees[1].hash, three_hash, "Three is alone so its hash should not change.");
        assert_eq!(merkle_tree_hash, expected_merkle_tree_hash);
    }

    #[test]
    fn four_leaf_tree() {
        let one_hash = hash(1);
        let two_hash = hash(2);
        let three_hash = hash(3);
        let four_hash = hash(4);

        let expected_left_subtree_hash = hash_concatenation(one_hash, two_hash);
        let expected_right_subtree_hash = hash_concatenation(three_hash, four_hash);
        let expected_merkle_tree_hash = hash_concatenation(expected_left_subtree_hash, expected_right_subtree_hash);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);
        merkle_tree.add(four_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 1, "One and two should be compacted, three and four also, and then both resulting should be compacted too.");
        assert_eq!(merkle_tree.subtrees[0].hash, expected_merkle_tree_hash);
        assert_eq!(merkle_tree_hash, expected_merkle_tree_hash);
    }

    #[test]
    fn five_leaf_tree() {
        let one_hash = hash(1);
        let two_hash = hash(2);
        let three_hash = hash(3);
        let four_hash = hash(4);
        let five_hash = hash(5);

        let expected_left_left_subtree_hash = hash_concatenation(one_hash, two_hash);
        let expected_left_right_subtree_hash = hash_concatenation(three_hash, four_hash);
        let expected_left_subtree_hash = hash_concatenation(expected_left_left_subtree_hash, expected_left_right_subtree_hash);

        let expected_right_left_subtree_hash_computation = hash_concatenation(five_hash, five_hash);
        let expected_right_subtree_hash_computation = hash_concatenation(expected_right_left_subtree_hash_computation, expected_right_left_subtree_hash_computation);

        let expected_merkle_tree_hash = hash_concatenation(expected_left_subtree_hash, expected_right_subtree_hash_computation);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);
        merkle_tree.add(four_hash);
        merkle_tree.add(five_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 2, "One, two, three and four should be compacted, five is alone.");
        assert_eq!(merkle_tree.subtrees[0].hash, expected_left_subtree_hash);
        assert_eq!(merkle_tree.subtrees[1].hash, five_hash);
        assert_eq!(merkle_tree_hash, expected_merkle_tree_hash);
    }

    #[test]
    fn seven_leaf_tree() {
        let one_hash = hash(1);
        let two_hash = hash(2);
        let three_hash = hash(3);
        let four_hash = hash(4);
        let five_hash = hash(5);
        let six_hash = hash(6);
        let seven_hash = hash(7);

        let expected_left_left_subtree_hash = hash_concatenation(one_hash, two_hash);
        let expected_left_right_subtree_hash = hash_concatenation(three_hash, four_hash);
        let expected_left_subtree_hash = hash_concatenation(expected_left_left_subtree_hash, expected_left_right_subtree_hash);

        let expected_right_left_subtree_hash = hash_concatenation(five_hash, six_hash);
        let expected_right_right_subtree_hash_computation = hash_concatenation(seven_hash, seven_hash);
        let expected_right_subtree_hash_computation = hash_concatenation(expected_right_left_subtree_hash, expected_right_right_subtree_hash_computation);

        let expected_merkle_tree_hash = hash_concatenation(expected_left_subtree_hash, expected_right_subtree_hash_computation);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);
        merkle_tree.add(four_hash);
        merkle_tree.add(five_hash);
        merkle_tree.add(six_hash);
        merkle_tree.add(seven_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 3, "One, two, three and four should be compacted, five and six too, seven is alone.");
        assert_eq!(merkle_tree.subtrees[0].hash, expected_left_subtree_hash);
        assert_eq!(merkle_tree.subtrees[1].hash, expected_right_left_subtree_hash);
        assert_eq!(merkle_tree.subtrees[2].hash, seven_hash);
        assert_eq!(merkle_tree_hash, expected_merkle_tree_hash);
    }

    #[test]
    fn clear_tree() {
        let one_hash = hash(1);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        assert_eq!(merkle_tree.subtrees.len(), 0);
        assert_eq!(merkle_tree.compute_hash(), [0; 32]);
        assert_eq!(merkle_tree.subtrees.len(), 0);

        merkle_tree.add(one_hash);
        assert_eq!(merkle_tree.subtrees.len(), 1);
        assert_eq!(merkle_tree.compute_hash(), one_hash);
        assert_eq!(merkle_tree.subtrees.len(), 1);

        merkle_tree.clear();
        assert_eq!(merkle_tree.subtrees.len(), 0);
        assert_eq!(merkle_tree.compute_hash(), [0; 32]);
        assert_eq!(merkle_tree.subtrees.len(), 0);
    }

    fn hash(number: u16) -> RawH256 {
        keccak(&number.to_be_bytes()).0
    }

    fn hash_concatenation(hash_left: RawH256, hash_right: RawH256) -> RawH256 {
        keccak(&[hash_left, hash_right].concat()).0
    }
}