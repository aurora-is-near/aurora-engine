use crate::{wasmer_runner::WasmRuntimeError, TransactionIncluded};
use aurora_engine_types::{account_id::AccountId, H256};

#[derive(Debug, Clone)]
pub enum Error {
    BlockNotFound(H256),
    Borsh(String),
    NoBlockAtHeight(u64),
    TransactionNotFound(TransactionIncluded),
    TransactionHashNotFound(H256),
    Rocksdb(rocksdb::Error),
    EngineAccountIdNotSet,
    EngineAccountIdCorrupted,
    Wasmer(WasmRuntimeError),
    AccountIdMismatch {
        expected: AccountId,
        found: AccountId,
    },
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
