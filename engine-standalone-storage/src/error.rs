use aurora_engine_types::H256;

use crate::TransactionIncluded;

#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    BlockNotFound(H256),
    Borsh(String),
    NoBlockAtHeight(u64),
    TransactionNotFound(TransactionIncluded),
    TransactionHashNotFound(H256),
    Rocksdb(rocksdb::Error),
    EngineAccountIdNotSet,
    EngineAccountIdCorrupted,
}

impl From<rocksdb::Error> for Error {
    fn from(e: rocksdb::Error) -> Self {
        Self::Rocksdb(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Borsh(e.to_string())
    }
}
