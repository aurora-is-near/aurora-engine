use aurora_engine_transactions::legacy::LegacyEthSignedTransaction;

/// A vastly simplified block structure
pub struct Block {
    pub height: u64,
    pub transactions: Vec<LegacyEthSignedTransaction>,
}

/// A vastly simplified blockchain structure. It is assumed
/// the 0th block is genesis and the parent of ith block is
/// block i-1.
pub type Blockchain = Vec<Block>;
