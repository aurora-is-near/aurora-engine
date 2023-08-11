use aurora_engine_sdk::keccak;
use aurora_engine_types::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    types::RawH256,
    Vec,
};

/// Stream Compact Merkle Tree.
/// It can be feed by a stream of hashes (leaves) adding them to the right of the tree.
/// Internally, compacts full binary subtrees maintaining only the growing branch.
/// Space used is O(log n) where n is the number of leaf hashes added. It is usually less.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct StreamCompactMerkleTree {
    /// Complete binary merkle subtrees.
    /// Left subtrees are strictly higher (bigger).
    /// Subtrees are compacted right to left when two consecutives have same height.
    subtrees: Vec<CompactMerkleSubtree>,
}

impl StreamCompactMerkleTree {
    pub const fn new() -> Self {
        Self {
            subtrees: Vec::new(),
        }
    }

    /// Returns `true` if no data has been added to the tree.
    pub fn is_empty(&self) -> bool {
        self.subtrees.is_empty()
    }

    /// Adds a leaf hash to the right of the tree.
    /// For n leaves hashes added, a single call to this function is O(log n),
    /// but the amortized time per each of the n calls is O(1).
    pub fn add(&mut self, leaf_hash: RawH256) {
        let leaf_subtree = CompactMerkleSubtree {
            height: 1,
            hash: leaf_hash,
        };
        self.subtrees.push(leaf_subtree);

        // compact subtrees from right to left
        let mut index = self.subtrees.len() - 1;

        while index >= 1 {
            debug_assert_eq!(index, self.subtrees.len() - 1);

            let right_subtree = &self.subtrees[index];
            let left_subtree = &self.subtrees[index - 1];

            // same height means they are siblings so we can compact them
            if left_subtree.height == right_subtree.height {
                let father_subtree = CompactMerkleSubtree {
                    height: left_subtree.height + 1,
                    hash: keccak(&[left_subtree.hash, right_subtree.hash].concat()).0,
                };

                self.subtrees.pop();
                // Unwrap is stafe since `index >= 1`
                *(self.subtrees.last_mut().unwrap()) = father_subtree;

                index -= 1;
            }
            // all remaining subtrees have different heights so we can't compact anything else
            else {
                debug_assert!(self
                    .subtrees
                    .iter()
                    .zip(self.subtrees.iter().skip(1))
                    .all(|(left, right)| left.height > right.height));
                break;
            }
        }
    }

    /// Computes the hash of the Merkle Tree.
    /// For n leaves hashes added, this function is O(log n).
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

impl Default for StreamCompactMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact Merkle Subtree.
/// For leaves, this represents only the leaf node with height 1 and the hash of the leaf.
/// For bigger subtrees, this represents the entire balanced subtree with its height and merkle hash.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
struct CompactMerkleSubtree {
    /// Height of the subtree.
    pub height: u8,

    /// Merkle tree hash of the subtree.
    pub hash: RawH256,
}

#[cfg(test)]
mod tests {
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

        assert_eq!(
            merkle_tree.subtrees.len(),
            1,
            "1 and 2 should be compacted."
        );
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
        let expected_merkle_tree_hash = hash_concatenation(
            expected_left_subtree_hash,
            expected_right_subtree_hash_computation,
        );

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(
            merkle_tree.subtrees.len(),
            2,
            "1 and 2 should be compacted; 3 should be alone."
        );
        assert_eq!(merkle_tree.subtrees[0].hash, expected_left_subtree_hash);
        assert_eq!(
            merkle_tree.subtrees[1].hash, three_hash,
            "3 should be alone."
        );
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
        let expected_merkle_tree_hash =
            hash_concatenation(expected_left_subtree_hash, expected_right_subtree_hash);

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);
        merkle_tree.add(four_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(merkle_tree.subtrees.len(), 1, "1 and 2 should be compacted, 3 and 4 also, and then both resulting should be compacted too.");
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
        let expected_left_subtree_hash = hash_concatenation(
            expected_left_left_subtree_hash,
            expected_left_right_subtree_hash,
        );

        let expected_right_left_subtree_hash_computation = hash_concatenation(five_hash, five_hash);
        let expected_right_subtree_hash_computation = hash_concatenation(
            expected_right_left_subtree_hash_computation,
            expected_right_left_subtree_hash_computation,
        );

        let expected_merkle_tree_hash = hash_concatenation(
            expected_left_subtree_hash,
            expected_right_subtree_hash_computation,
        );

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);
        merkle_tree.add(four_hash);
        merkle_tree.add(five_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(
            merkle_tree.subtrees.len(),
            2,
            "1, 2, 3 and 4 should be compacted; 5 is alone."
        );
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
        let expected_left_subtree_hash = hash_concatenation(
            expected_left_left_subtree_hash,
            expected_left_right_subtree_hash,
        );

        let expected_right_left_subtree_hash = hash_concatenation(five_hash, six_hash);
        let expected_right_right_subtree_hash_computation =
            hash_concatenation(seven_hash, seven_hash);
        let expected_right_subtree_hash_computation = hash_concatenation(
            expected_right_left_subtree_hash,
            expected_right_right_subtree_hash_computation,
        );

        let expected_merkle_tree_hash = hash_concatenation(
            expected_left_subtree_hash,
            expected_right_subtree_hash_computation,
        );

        let mut merkle_tree = StreamCompactMerkleTree::new();
        merkle_tree.add(one_hash);
        merkle_tree.add(two_hash);
        merkle_tree.add(three_hash);
        merkle_tree.add(four_hash);
        merkle_tree.add(five_hash);
        merkle_tree.add(six_hash);
        merkle_tree.add(seven_hash);

        let merkle_tree_hash = merkle_tree.compute_hash();

        assert_eq!(
            merkle_tree.subtrees.len(),
            3,
            "1, 2, 3 and 4 should be compacted; 5 and 6 too; 7 is alone."
        );
        assert_eq!(merkle_tree.subtrees[0].hash, expected_left_subtree_hash);
        assert_eq!(
            merkle_tree.subtrees[1].hash,
            expected_right_left_subtree_hash
        );
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
