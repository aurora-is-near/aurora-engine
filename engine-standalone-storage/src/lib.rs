use aurora_engine_sdk::env::Timestamp;
use aurora_engine_types::H256;
use rocksdb::DB;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;

const VERSION: u8 = 0;

pub mod diff;
pub mod engine_state;
pub mod error;
pub mod json_snapshot;
mod promise;
pub mod relayer_db;
/// Functions for receiving new blocks and transactions to keep the storage up to date.
pub mod sync;

pub use diff::Diff;
pub use error::Error;

/// Length (in bytes) of the suffix appended to Engine keys which specify the
/// block height and transaction position. 64 bits for the block height,
/// 16 bits for the transaction position.
const ENGINE_KEY_SUFFIX_LEN: usize = (64 / 8) + (16 / 8);

#[repr(u8)]
pub enum StoragePrefix {
    BlockHash = 0x00,
    BlockHeight = 0x01,
    TransactionPosition = 0x02,
    TransactionHash = 0x03,
    Diff = 0x04,
    Engine = 0x05,
    BlockMetadata = 0x06,
}

pub struct Storage {
    db: DB,
    engine_transaction: RefCell<Diff>,
    engine_output: Cell<Vec<u8>>,
}

impl Storage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, rocksdb::Error> {
        let db = DB::open_default(path)?;
        let engine_transaction = RefCell::new(Diff::default());
        let engine_output = Cell::new(Vec::new());
        Ok(Self {
            db,
            engine_transaction,
            engine_output,
        })
    }

    pub fn get_block_hash_by_height(&self, block_height: u64) -> Result<H256, error::Error> {
        let storage_key =
            construct_storage_key(StoragePrefix::BlockHash, &block_height.to_be_bytes());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| H256::from_slice(slice.as_ref()))
            .ok_or(error::Error::NoBlockAtHeight(block_height))
    }

    pub fn get_block_height_by_hash(&self, block_hash: H256) -> Result<u64, error::Error> {
        let storage_key = construct_storage_key(StoragePrefix::BlockHeight, block_hash.as_ref());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(slice.as_ref());
                u64::from_be_bytes(buf)
            })
            .ok_or(error::Error::BlockNotFound(block_hash))
    }

    pub fn get_block_metadata(&self, block_hash: H256) -> Result<BlockMetadata, error::Error> {
        let storage_key = construct_storage_key(StoragePrefix::BlockMetadata, block_hash.as_ref());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| {
                let mut buf = [0u8; 40];
                buf.copy_from_slice(slice.as_ref());
                BlockMetadata::from_bytes(buf)
            })
            .ok_or(error::Error::BlockNotFound(block_hash))
    }

    pub fn set_block_data(
        &mut self,
        block_hash: H256,
        block_height: u64,
        block_metadata: BlockMetadata,
    ) -> Result<(), rocksdb::Error> {
        let block_height_bytes = block_height.to_be_bytes();

        let mut batch = rocksdb::WriteBatch::default();

        let storage_key = construct_storage_key(StoragePrefix::BlockHash, &block_height_bytes);
        batch.put(storage_key, block_hash);

        let storage_key = construct_storage_key(StoragePrefix::BlockHeight, block_hash.as_ref());
        batch.put(storage_key, block_height_bytes);

        let storage_key = construct_storage_key(StoragePrefix::BlockMetadata, block_hash.as_ref());
        batch.put(storage_key, block_metadata.to_bytes());

        self.db.write(batch)
    }

    pub fn get_transaction_by_hash(
        &self,
        tx_hash: H256,
    ) -> Result<TransactionIncluded, error::Error> {
        let storage_key =
            construct_storage_key(StoragePrefix::TransactionPosition, tx_hash.as_ref());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| {
                let mut buf = [0u8; 34];
                buf.copy_from_slice(slice.as_ref());
                TransactionIncluded::from_bytes(buf)
            })
            .ok_or(error::Error::TransactionHashNotFound(tx_hash))
    }

    pub fn get_transaction_by_position(
        &self,
        tx_included: TransactionIncluded,
    ) -> Result<H256, error::Error> {
        let storage_key =
            construct_storage_key(StoragePrefix::TransactionHash, &tx_included.to_bytes());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| H256::from_slice(slice.as_ref()))
            .ok_or(error::Error::TransactionNotFound(tx_included))
    }

    pub fn get_transaction_diff(
        &self,
        tx_included: TransactionIncluded,
    ) -> Result<Diff, error::Error> {
        let storage_key = construct_storage_key(StoragePrefix::Diff, &tx_included.to_bytes());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| Diff::try_from_bytes(slice.as_ref()).unwrap())
            .ok_or(error::Error::TransactionNotFound(tx_included))
    }

    pub fn set_transaction_included(
        &mut self,
        tx_hash: H256,
        tx_included: &TransactionIncluded,
        diff: &Diff,
    ) -> Result<(), error::Error> {
        let tx_included_bytes = tx_included.to_bytes();
        let block_height = self.get_block_height_by_hash(tx_included.block_hash)?;

        let mut batch = rocksdb::WriteBatch::default();

        let storage_key = construct_storage_key(StoragePrefix::TransactionHash, &tx_included_bytes);
        batch.put(storage_key, tx_hash);

        let storage_key =
            construct_storage_key(StoragePrefix::TransactionPosition, tx_hash.as_ref());
        batch.put(storage_key, tx_included_bytes);

        let storage_key = construct_storage_key(StoragePrefix::Diff, &tx_included_bytes);
        batch.put(storage_key, diff.try_to_bytes().unwrap());

        for (key, value) in diff.iter() {
            let storage_key = construct_engine_key(key, block_height, tx_included.position);
            batch.put(storage_key, value.try_to_bytes().unwrap());
        }

        self.db.write(batch).map_err(Into::into)
    }

    /// Construct a snapshot of the Engine post-state at the given block height.
    /// I.e. get the state of the Engine after all transactions in that block have been applied.
    pub fn get_snapshot(&self, block_height: u64) -> Result<HashMap<Vec<u8>, Vec<u8>>, rocksdb::Error> {
        let engine_prefix = construct_storage_key(StoragePrefix::Engine, &[]);
        let mut iter: rocksdb::DBRawIterator = self.db.prefix_iterator(&engine_prefix).into();
        let mut result = HashMap::new();

        while iter.valid() {
            // unwrap is safe because the iterator is valid
            let db_key = iter.key().unwrap().to_vec();
            if &db_key[0..engine_prefix.len()] != &engine_prefix {
                break;
            }
            // raw engine key skips the 2-byte prefix and the block+position suffix
            let engine_key = &db_key[2..(db_key.len() - ENGINE_KEY_SUFFIX_LEN)];
            // the key we want is the last key for this block, or the key immediately before it
            let desired_db_key = construct_engine_key(engine_key, block_height, u16::MAX);
            iter.seek_for_prev(&desired_db_key);
            
            let value = if iter.valid() {
                let bytes = iter.value().unwrap();
                diff::DiffValue::try_from_bytes(bytes).unwrap_or_else(|e|{
                    panic!(
                        "Could not deserialize key={} value={} error={:?}",
                        base64::encode(&db_key),
                        base64::encode(bytes),
                        e,
                    )
                })
            } else {
                break;
            };
            // only put it values that are still present (i.e. ignore deleted keys)
            if let Some(bytes) = value.take_value() {
                result.insert(engine_key.to_vec(), bytes);
            }

            // move to the next db key, which is after all the blocks for this engine key
            let key = construct_engine_key(engine_key, u64::MAX, u16::MAX);
            iter.seek(&key);
        }
        
        iter.status()?;

        Ok(result)
    }

    /// Get an object which represents the state of the engine at the given block hash,
    /// after transactions up to (not including) the given transaction index.
    /// The `input` is the bytes that would be present in the NEAR runtime (normally
    /// not needed for standalone engine).
    pub fn access_engine_storage_at_position<'db, 'input: 'db>(
        &'db mut self,
        block_height: u64,
        transaction_position: u16,
        input: &'input [u8],
    ) -> engine_state::EngineStateAccess<'db, 'db, 'db> {
        self.engine_transaction.borrow_mut().clear();
        self.engine_output.set(Vec::new());

        engine_state::EngineStateAccess::new(
            input,
            block_height,
            transaction_position,
            &self.engine_transaction,
            &self.engine_output,
            &self.db,
        )
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TransactionIncluded {
    pub block_hash: H256,
    pub position: u16,
}

impl TransactionIncluded {
    pub fn to_bytes(self) -> [u8; 34] {
        let mut bytes = [0u8; 34];

        bytes[0..32].copy_from_slice(self.block_hash.as_ref());
        bytes[32..34].copy_from_slice(&self.position.to_be_bytes());

        bytes
    }

    pub fn from_bytes(bytes: [u8; 34]) -> Self {
        let block_hash = H256::from_slice(&bytes[0..32]);
        let mut position = [0u8; 2];
        position.copy_from_slice(&bytes[32..34]);

        Self {
            block_hash,
            position: u16::from_be_bytes(position),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMetadata {
    pub timestamp: Timestamp,
    /// Each NEAR block has a 32-byte entropy source generated by a VRF. We need this data
    /// to execute the Aurora randomness precompile correctly because it uses this NEAR
    /// entropy source.
    pub random_seed: H256,
}

impl BlockMetadata {
    pub fn to_bytes(&self) -> [u8; 40] {
        let mut buf = [0u8; 40];
        buf[0..8].copy_from_slice(&self.timestamp.nanos().to_be_bytes());
        buf[8..40].copy_from_slice(self.random_seed.as_ref());
        buf
    }

    pub fn from_bytes(bytes: [u8; 40]) -> Self {
        let nanos = {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[0..8]);
            u64::from_be_bytes(buf)
        };
        let random_seed = {
            let mut buf = [0u8; 32];
            buf.copy_from_slice(&bytes[8..40]);
            H256(buf)
        };

        Self {
            timestamp: Timestamp::new(nanos),
            random_seed,
        }
    }
}

fn construct_storage_key(prefix: StoragePrefix, key: &[u8]) -> Vec<u8> {
    [&[VERSION], &[prefix as u8], key].concat()
}

fn construct_engine_key(key: &[u8], block_height: u64, transaction_position: u16) -> Vec<u8> {
    construct_storage_key(
        StoragePrefix::Engine,
        [
            key,
            &block_height.to_be_bytes(),
            &transaction_position.to_be_bytes(),
        ]
        .concat()
        .as_slice(),
    )
}
