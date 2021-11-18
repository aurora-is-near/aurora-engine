use aurora_engine_types::H256;

use crate::TransactionIncluded;

#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    BlockNotFound(H256),
    NoBlockAtHeight(u64),
    TransactionNotFound(TransactionIncluded),
    TransactionHashNotFound(H256),
    Rocksdb(rocksdb::Error),
}

impl From<rocksdb::Error> for Error {
    fn from(e: rocksdb::Error) -> Self {
        Self::Rocksdb(e)
    }
}
