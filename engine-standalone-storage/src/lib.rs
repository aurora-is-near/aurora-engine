use aurora_engine_sdk::env::Timestamp;
use aurora_engine_types::{account_id::AccountId, H256};
use rocksdb::DB;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use sync::types::TransactionMessage;

const VERSION: u8 = 0;

pub mod diff;
pub mod engine_state;
pub mod error;
pub mod json_snapshot;
pub mod promise;
pub mod relayer_db;
/// Functions for receiving new blocks and transactions to keep the storage up to date.
pub mod sync;

pub use diff::{Diff, DiffValue};
pub use error::Error;

mod state;

/// Length (in bytes) of the suffix appended to Engine keys which specify the
/// block height and transaction position. 64 bits for the block height,
/// 16 bits for the transaction position.
const ENGINE_KEY_SUFFIX_LEN: usize = (64 / 8) + (16 / 8);

#[repr(u8)]
pub enum StoragePrefix {
    BlockHash = 0x00,
    BlockHeight = 0x01,
    TransactionData = 0x02,
    TransactionHash = 0x03,
    Diff = 0x04,
    Engine = 0x05,
    BlockMetadata = 0x06,
    EngineAccountId = 0x07,
    /// Prefix used for storing arbitrary data from the outside of the crate.
    CustomData = 0x8,
}

impl From<StoragePrefix> for u8 {
    fn from(value: StoragePrefix) -> Self {
        match value {
            StoragePrefix::BlockHash => 0x00,
            StoragePrefix::BlockHeight => 0x01,
            StoragePrefix::TransactionData => 0x02,
            StoragePrefix::TransactionHash => 0x03,
            StoragePrefix::Diff => 0x04,
            StoragePrefix::Engine => 0x05,
            StoragePrefix::BlockMetadata => 0x06,
            StoragePrefix::EngineAccountId => 0x07,
            StoragePrefix::CustomData => 0x08,
        }
    }
}

const ACCOUNT_ID_KEY: &[u8] = b"engine_account_id";

pub struct Storage {
    db: DB,
}

impl Storage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, rocksdb::Error> {
        let db = DB::open_default(path)?;
        Ok(Self { db })
    }

    pub fn set_engine_account_id(&mut self, id: &AccountId) -> Result<(), rocksdb::Error> {
        let key = construct_storage_key(StoragePrefix::EngineAccountId, ACCOUNT_ID_KEY);
        self.db.put(key, id.as_bytes())
    }

    pub fn get_engine_account_id(&self) -> Result<AccountId, Error> {
        let key = construct_storage_key(StoragePrefix::EngineAccountId, ACCOUNT_ID_KEY);
        let slice = self
            .db
            .get_pinned(key)?
            .ok_or(Error::EngineAccountIdNotSet)?;
        let account_id =
            AccountId::try_from(slice.as_ref()).map_err(|_| Error::EngineAccountIdCorrupted)?;
        Ok(account_id)
    }

    pub fn get_latest_block(&self) -> Result<(H256, u64), Error> {
        self.block_read(rocksdb::IteratorMode::End)
    }

    pub fn get_earliest_block(&self) -> Result<(H256, u64), Error> {
        self.block_read(rocksdb::IteratorMode::Start)
    }

    fn read_by_key(
        &self,
        key: &[u8],
        bound_block_height: u64,
        transaction_position: u16,
    ) -> Result<DiffValue, Error> {
        let upper_bound = construct_engine_key(key, bound_block_height, transaction_position);
        let lower_bound = construct_storage_key(StoragePrefix::Engine, key);
        let mut opt = rocksdb::ReadOptions::default();
        opt.set_iterate_upper_bound(upper_bound);
        opt.set_iterate_lower_bound(lower_bound);

        let mut iter = self.db.iterator_opt(rocksdb::IteratorMode::End, opt);
        // TODO: error kind
        let (_, value) = iter.next().ok_or(Error::NoBlockAtHeight(0))??;
        Ok(DiffValue::try_from_bytes(&value).expect("diff value is invalid"))
    }

    fn block_read(&self, mode: rocksdb::IteratorMode) -> Result<(H256, u64), Error> {
        let upper_bound = construct_storage_key(StoragePrefix::BlockHash, &u64::MAX.to_be_bytes());
        let lower_bound = construct_storage_key(StoragePrefix::BlockHash, &[]);
        let prefix_len = lower_bound.len();
        let mut opt = rocksdb::ReadOptions::default();
        opt.set_iterate_upper_bound(upper_bound);
        opt.set_iterate_lower_bound(lower_bound);

        let mut iter = self.db.iterator_opt(mode, opt);
        let (key, value) = iter.next().ok_or(Error::NoBlockAtHeight(0))??;
        let block_height = {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&key[prefix_len..]);
            u64::from_be_bytes(buf)
        };
        let block_hash = H256::from_slice(&value);
        Ok((block_hash, block_height))
    }

    pub fn get_block_hash_by_height(&self, block_height: u64) -> Result<H256, Error> {
        let storage_key =
            construct_storage_key(StoragePrefix::BlockHash, &block_height.to_be_bytes());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| H256::from_slice(slice.as_ref()))
            .ok_or(Error::NoBlockAtHeight(block_height))
    }

    pub fn get_block_height_by_hash(&self, block_hash: H256) -> Result<u64, Error> {
        let storage_key = construct_storage_key(StoragePrefix::BlockHeight, block_hash.as_ref());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(slice.as_ref());
                u64::from_be_bytes(buf)
            })
            .ok_or(Error::BlockNotFound(block_hash))
    }

    pub fn get_block_metadata(&self, block_hash: H256) -> Result<BlockMetadata, Error> {
        let storage_key = construct_storage_key(StoragePrefix::BlockMetadata, block_hash.as_ref());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| {
                let mut buf = [0u8; 40];
                buf.copy_from_slice(slice.as_ref());
                BlockMetadata::from_bytes(buf)
            })
            .ok_or(Error::BlockNotFound(block_hash))
    }

    pub fn set_block_data(
        &mut self,
        block_hash: H256,
        block_height: u64,
        block_metadata: &BlockMetadata,
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

    pub fn get_transaction_data(&self, tx_hash: H256) -> Result<TransactionMessage, Error> {
        let storage_key = construct_storage_key(StoragePrefix::TransactionData, tx_hash.as_ref());
        let bytes = self
            .db
            .get_pinned(storage_key)?
            .ok_or(Error::TransactionHashNotFound(tx_hash))?;
        let message = TransactionMessage::try_from_slice(bytes.as_ref())?;
        Ok(message)
    }

    pub fn get_transaction_by_position(
        &self,
        tx_included: TransactionIncluded,
    ) -> Result<H256, Error> {
        let storage_key =
            construct_storage_key(StoragePrefix::TransactionHash, &tx_included.to_bytes());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| H256::from_slice(slice.as_ref()))
            .ok_or(Error::TransactionNotFound(tx_included))
    }

    pub fn get_transaction_diff(&self, tx_included: TransactionIncluded) -> Result<Diff, Error> {
        let storage_key = construct_storage_key(StoragePrefix::Diff, &tx_included.to_bytes());
        self.db
            .get_pinned(storage_key)?
            .map(|slice| Diff::try_from_bytes(slice.as_ref()).expect("transaction_diff is invalid"))
            .ok_or(Error::TransactionNotFound(tx_included))
    }

    pub fn set_transaction_included(
        &mut self,
        tx_hash: H256,
        tx_included: &TransactionMessage,
        diff: &Diff,
    ) -> Result<(), Error> {
        let batch = rocksdb::WriteBatch::default();
        self.process_transaction(tx_hash, tx_included, diff, batch, |batch, key, value| {
            batch.put(key, value);
        })
    }

    pub fn revert_transaction_included(
        &mut self,
        tx_hash: H256,
        tx_included: &TransactionMessage,
        diff: &Diff,
    ) -> Result<(), Error> {
        let batch = rocksdb::WriteBatch::default();
        self.process_transaction(tx_hash, tx_included, diff, batch, |batch, key, _value| {
            batch.delete(key);
        })
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    fn process_transaction<F: Fn(&mut rocksdb::WriteBatch, &[u8], &[u8])>(
        &mut self,
        tx_hash: H256,
        tx_msg: &TransactionMessage,
        diff: &Diff,
        mut batch: rocksdb::WriteBatch,
        action: F,
    ) -> Result<(), Error> {
        let tx_included = TransactionIncluded {
            block_hash: tx_msg.block_hash,
            position: tx_msg.position,
        };
        let tx_included_bytes = tx_included.to_bytes();
        let block_height = self.get_block_height_by_hash(tx_included.block_hash)?;

        let storage_key = construct_storage_key(StoragePrefix::TransactionHash, &tx_included_bytes);
        action(&mut batch, &storage_key, tx_hash.as_ref());

        let storage_key = construct_storage_key(StoragePrefix::TransactionData, tx_hash.as_ref());
        let msg_bytes = tx_msg.to_bytes();
        action(&mut batch, &storage_key, &msg_bytes);

        let storage_key = construct_storage_key(StoragePrefix::Diff, &tx_included_bytes);
        let diff_bytes = diff.try_to_bytes().expect("diff should is invalid");
        action(&mut batch, &storage_key, &diff_bytes);

        for (key, value) in diff {
            let storage_key = construct_engine_key(key, block_height, tx_included.position);
            let value_bytes = value.try_to_bytes().expect("value is invalid");
            action(&mut batch, &storage_key, &value_bytes);
        }

        self.db.write(batch).map_err(Into::into)
    }

    /// Returns a list of transactions that modified the key, and the values _after_ each transaction.
    pub fn track_engine_key(
        &self,
        engine_key: &[u8],
    ) -> Result<Vec<(u64, H256, DiffValue)>, Error> {
        let db_key_prefix = construct_storage_key(StoragePrefix::Engine, engine_key);
        let n = db_key_prefix.len();
        let iter = self.db.prefix_iterator(&db_key_prefix);
        let mut result = Vec::with_capacity(100);
        for maybe_elem in iter {
            let (k, v) = maybe_elem?;
            if k.len() < n || k[0..n] != db_key_prefix {
                break;
            }
            let value = DiffValue::try_from_bytes(v.as_ref()).expect("diff should is invalid");
            let block_height = {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&k[n..(n + 8)]);
                u64::from_be_bytes(buf)
            };
            let transaction_position = {
                let mut buf = [0u8; 2];
                buf.copy_from_slice(&k[(n + 8)..(n + 10)]);
                u16::from_be_bytes(buf)
            };
            let block_hash = self
                .get_block_hash_by_height(block_height)
                .unwrap_or_default();
            let tx_included = TransactionIncluded {
                block_hash,
                position: transaction_position,
            };
            let tx_hash = self
                .get_transaction_by_position(tx_included)
                .unwrap_or_default();
            result.push((block_height, tx_hash, value));
        }
        Ok(result)
    }

    /// Construct a snapshot of the Engine post-state at the given block height.
    /// I.e. get the state of the Engine after all transactions in that block have been applied.
    pub fn get_snapshot(
        &self,
        block_height: u64,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, rocksdb::Error> {
        let engine_prefix = construct_storage_key(StoragePrefix::Engine, &[]);
        let engine_prefix_len = engine_prefix.len();
        let mut iter: rocksdb::DBRawIterator = self.db.prefix_iterator(&engine_prefix).into();
        let mut result = HashMap::new();

        while iter.valid() {
            // unwrap is safe because the iterator is valid
            let db_key = iter.key().expect("iterator should is invalid").to_vec();
            if db_key.get(0..engine_prefix_len) != Some(&engine_prefix) {
                break;
            }
            // raw engine key skips the 2-byte prefix and the block+position suffix
            let engine_key = &db_key
                .get(engine_prefix_len..(db_key.len() - ENGINE_KEY_SUFFIX_LEN))
                .expect("index out of bounds");
            let key_block_height = {
                let n = engine_prefix_len + engine_key.len();
                let mut buf = [0u8; 8];
                buf.copy_from_slice(db_key.get(n..(n + 8)).expect("index out of bounds"));
                u64::from_be_bytes(buf)
            };
            // If the key was created after the block height we want then we can skip it
            if key_block_height <= block_height {
                // the key we want is the last key for this block, or the key immediately before it
                let desired_db_key = construct_engine_key(engine_key, block_height, u16::MAX);
                iter.seek_for_prev(&desired_db_key);

                let value = if iter.valid() {
                    let bytes = iter.value().expect("iterator is invalid");
                    DiffValue::try_from_bytes(bytes).unwrap_or_else(|e| {
                        panic!(
                            "Could not deserialize key={} value={} error={:?}",
                            aurora_engine_sdk::base64::encode(&db_key),
                            aurora_engine_sdk::base64::encode(bytes),
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
            }

            // move to the next key by skipping all other DB keys corresponding to the same engine key
            while iter.valid()
                && iter.key().is_some_and(|db_key| {
                    db_key[0..engine_prefix_len] == engine_prefix
                        && &db_key[engine_prefix_len..(db_key.len() - ENGINE_KEY_SUFFIX_LEN)]
                            == *engine_key
                })
            {
                iter.next();
            }
        }

        iter.status()?;

        Ok(result)
    }

    /// Same as `access_engine_storage_at_position`, but does not modify `self`, hence the immutable
    /// borrow instead of the mutable one. The use case for this function is to execute a transaction
    /// with the engine, but not to make any immediate changes to storage; only return the diff and outcome.
    /// Note the closure is allowed to mutate the `EngineStateAccess` object, but this does not impact the `Storage`
    /// because all changes are held in the diff in memory.
    pub fn with_engine_access<'db, 'input, R, F>(
        &'db self,
        block_height: u64,
        transaction_position: u16,
        input: &'input [u8],
        f: F,
    ) -> EngineAccessResult<R>
    where
        F: for<'output> FnOnce(engine_state::EngineStateAccess<'db, 'input, 'output>) -> R,
    {
        let diff = RefCell::new(Diff::default());
        let engine_output = Cell::new(Vec::new());

        let engine_state = engine_state::EngineStateAccess::new(
            input,
            block_height,
            transaction_position,
            &diff,
            &engine_output,
            &self.db,
        );

        let result = f(engine_state);
        let diff = engine_state.get_transaction_diff();
        let engine_output = engine_output.into_inner();

        EngineAccessResult {
            result,
            engine_output,
            diff,
        }
    }

    /// Retrieve data for a key with `CustomData` prefix. A helper method which allows getting
    /// arbitrary data from outside the crate.
    pub fn get_custom_data(&self, key: &[u8]) -> Result<Option<Vec<u8>>, rocksdb::Error> {
        let key = construct_storage_key(StoragePrefix::CustomData, key);
        self.db.get(key)
    }

    /// Save data for a key with `CustomData` prefix. A helper method which allows saving
    /// arbitrary data from outside the crate.
    pub fn set_custom_data(&self, key: &[u8], value: &[u8]) -> Result<(), rocksdb::Error> {
        let key = construct_storage_key(StoragePrefix::CustomData, key);
        self.db.put(key, value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EngineAccessResult<R> {
    pub result: R,
    pub engine_output: Vec<u8>,
    pub diff: Diff,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TransactionIncluded {
    pub block_hash: H256,
    pub position: u16,
}

impl TransactionIncluded {
    #[must_use]
    pub fn to_bytes(self) -> [u8; 34] {
        let mut bytes = [0u8; 34];

        bytes[0..32].copy_from_slice(self.block_hash.as_ref());
        bytes[32..34].copy_from_slice(&self.position.to_be_bytes());

        bytes
    }

    #[must_use]
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
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 40] {
        let mut buf = [0u8; 40];
        buf[0..8].copy_from_slice(&self.timestamp.nanos().to_be_bytes());
        buf[8..40].copy_from_slice(self.random_seed.as_ref());
        buf
    }

    #[must_use]
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
    [&[VERSION], &[u8::from(prefix)], key].concat()
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
