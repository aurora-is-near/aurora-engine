use crate::{bloom::Bloom, error::BlockchainHashchainError, merkle::StreamCompactMerkleTree};
use aurora_engine_sdk::keccak;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::{self, io, BorshDeserialize, BorshSerialize},
    format,
    types::RawH256,
    Cow, Vec,
};

/// Blockchain Hashchain.
/// Continually keeps track of the previous block hashchain through the blocks heights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashchain {
    chain_id: [u8; 32],
    contract_account_id: AccountId,
    current_block_height: u64,
    previous_block_hashchain: RawH256,
    block_hashchain_computer: BlockHashchainComputer,
}

impl Hashchain {
    #[must_use]
    pub fn new(
        chain_id: [u8; 32],
        contract_account_id: AccountId,
        current_block_height: u64,
        previous_block_hashchain: RawH256,
    ) -> Self {
        Self {
            chain_id,
            contract_account_id,
            current_block_height,
            previous_block_hashchain,
            block_hashchain_computer: BlockHashchainComputer::new(),
        }
    }

    /// Adds a transaction if the indicated block height is equal to the current block height.
    /// Returns an error in other case.
    pub fn add_block_tx(
        &mut self,
        block_height: u64,
        method_name: &str,
        input: &[u8],
        output: &[u8],
        log_bloom: &Bloom,
    ) -> Result<(), BlockchainHashchainError> {
        if block_height != self.current_block_height {
            return Err(BlockchainHashchainError::BlockHeightIncorrect);
        }

        self.block_hashchain_computer
            .add_tx(method_name, input, output, log_bloom);

        Ok(())
    }

    /// Moves to the indicated block height if it is bigger than the current block height:
    /// -Updates the previous block hashchain computing the hash.
    /// -Updates the current block height.
    /// -Clears the transactions.
    /// -Clears the transactions.
    /// Returns an error in other case.
    pub fn move_to_block(
        &mut self,
        next_block_height: u64,
    ) -> Result<(), BlockchainHashchainError> {
        if next_block_height <= self.current_block_height {
            return Err(BlockchainHashchainError::BlockHeightIncorrect);
        }

        while self.current_block_height < next_block_height {
            self.previous_block_hashchain = self.block_hashchain_computer.compute_block_hashchain(
                &self.chain_id,
                self.contract_account_id.as_bytes(),
                self.current_block_height,
                self.previous_block_hashchain,
            );

            self.block_hashchain_computer.clear_txs();
            self.current_block_height += 1;
        }

        Ok(())
    }

    /// Gets the current block height of the structure.
    #[must_use]
    pub const fn get_current_block_height(&self) -> u64 {
        self.current_block_height
    }

    /// Gets the previous block hashchain of the structure.
    #[must_use]
    pub const fn get_previous_block_hashchain(&self) -> RawH256 {
        self.previous_block_hashchain
    }

    pub fn get_logs_bloom(&self) -> &Bloom {
        &self.block_hashchain_computer.txs_logs_bloom
    }

    pub fn is_empty(&self) -> bool {
        self.block_hashchain_computer.is_empty()
    }

    pub fn try_serialize(&self) -> Result<Vec<u8>, io::Error> {
        let serializable: BorshableHashchain = self.into();
        borsh::to_vec(&serializable)
    }

    pub fn try_deserialize(bytes: &[u8]) -> Result<Self, io::Error> {
        let serializable = BorshableHashchain::try_from_slice(bytes)?;
        Self::try_from(serializable)
    }
}

#[derive(Debug, Default)]
pub struct HashchainBuilder {
    chain_id: [u8; 32],
    contract_account_id: AccountId,
    current_block_height: u64,
    previous_block_hashchain: RawH256,
}

impl HashchainBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_chain_id(mut self, chain_id: [u8; 32]) -> Self {
        self.chain_id = chain_id;
        self
    }

    pub fn with_u64_chain_id(self, chain_id: u64) -> Self {
        self.with_chain_id(aurora_engine_types::types::u256_to_arr(&chain_id.into()))
    }

    pub fn with_account_id(mut self, id: AccountId) -> Self {
        self.contract_account_id = id;
        self
    }

    pub fn with_current_block_height(mut self, height: u64) -> Self {
        self.current_block_height = height;
        self
    }

    pub fn with_previous_hashchain(mut self, hashchain: RawH256) -> Self {
        self.previous_block_hashchain = hashchain;
        self
    }

    pub fn build(self) -> Hashchain {
        Hashchain::new(
            self.chain_id,
            self.contract_account_id,
            self.current_block_height,
            self.previous_block_hashchain,
        )
    }
}

/// Representation of the hashchain that can be serialized/deserialized to/from bytes.
/// This struct is intentionally separate from `Hashchain` because then the business logic
/// is not bogged down with details of serialization (for example this data type is an enum
/// to allow for easy changes to the serialized form in the future).
#[derive(Debug, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
enum BorshableHashchain<'a> {
    V1 {
        chain_id: Cow<'a, [u8; 32]>,
        contract_account_id: Cow<'a, str>,
        current_block_height: u64,
        previous_block_hashchain: Cow<'a, RawH256>,
        block_hashchain_computer: Cow<'a, BlockHashchainComputer>,
    },
}

impl<'a> From<&'a Hashchain> for BorshableHashchain<'a> {
    fn from(value: &'a Hashchain) -> Self {
        Self::V1 {
            chain_id: Cow::Borrowed(&value.chain_id),
            contract_account_id: Cow::Borrowed(value.contract_account_id.as_ref()),
            current_block_height: value.current_block_height,
            previous_block_hashchain: Cow::Borrowed(&value.previous_block_hashchain),
            block_hashchain_computer: Cow::Borrowed(&value.block_hashchain_computer),
        }
    }
}

impl<'a> TryFrom<BorshableHashchain<'a>> for Hashchain {
    type Error = io::Error;

    fn try_from(value: BorshableHashchain<'a>) -> Result<Self, Self::Error> {
        match value {
            BorshableHashchain::V1 {
                chain_id,
                contract_account_id,
                current_block_height,
                previous_block_hashchain,
                block_hashchain_computer,
            } => Ok(Self {
                chain_id: chain_id.into_owned(),
                contract_account_id: AccountId::new(&contract_account_id)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?,
                current_block_height,
                previous_block_hashchain: previous_block_hashchain.into_owned(),
                block_hashchain_computer: block_hashchain_computer.into_owned(),
            }),
        }
    }
}

/// Block Hashchain Computer.
/// The order of operations should be:
/// 1. Create the `BlockHashchainComputer` one time.
/// 2. Add transactions of the current block.
/// 3. Compute the block hashchain for the current block once all the transactions were added.
/// 4. Clear the transactions of the current block.
/// 5. Go back to step 2 for the next block.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
#[borsh(crate = "aurora_engine_types::borsh")]
struct BlockHashchainComputer {
    pub txs_logs_bloom: Bloom,
    pub txs_merkle_tree: StreamCompactMerkleTree,
}

impl BlockHashchainComputer {
    pub fn new() -> Self {
        Self {
            txs_logs_bloom: Bloom::default(),
            txs_merkle_tree: StreamCompactMerkleTree::new(),
        }
    }

    /// Adds a transaction.
    pub fn add_tx(&mut self, method_name: &str, input: &[u8], output: &[u8], log_bloom: &Bloom) {
        let data = [
            &saturating_cast(method_name.len()).to_be_bytes(),
            method_name.as_bytes(),
            &saturating_cast(input.len()).to_be_bytes(),
            input,
            &saturating_cast(output.len()).to_be_bytes(),
            output,
        ]
        .concat();

        let tx_hash = keccak(&data).0;

        self.txs_logs_bloom.accrue_bloom(log_bloom);
        self.txs_merkle_tree.add(tx_hash);
    }

    /// Computes the block hashchain.
    pub fn compute_block_hashchain(
        &self,
        chain_id: &[u8; 32],
        contract_account_id: &[u8],
        current_block_height: u64,
        previous_block_hashchain: RawH256,
    ) -> RawH256 {
        let txs_hash = self.txs_merkle_tree.compute_hash();

        let data = [
            chain_id,
            contract_account_id,
            &current_block_height.to_be_bytes(),
            &previous_block_hashchain,
            &txs_hash,
            self.txs_logs_bloom.as_bytes(),
        ]
        .concat();

        keccak(&data).0
    }

    /// Clears the transactions added.
    pub fn clear_txs(&mut self) {
        self.txs_logs_bloom = Bloom::default();
        self.txs_merkle_tree.clear();
    }

    /// Checks no transactions have been added.
    pub fn is_empty(&self) -> bool {
        self.txs_merkle_tree.is_empty()
    }
}

fn saturating_cast(x: usize) -> u32 {
    x.try_into().unwrap_or(u32::MAX)
}
