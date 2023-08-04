use crate::{sync::types::TransactionKindTag, TransactionIncluded};
use aurora_engine_types::H256;
use std::fmt;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseTransactionKindError {
    UnknownMethodName {
        name: String,
    },
    FailedDeserialization {
        transaction_kind_tag: TransactionKindTag,
        error_message: String,
    },
}

impl ParseTransactionKindError {
    pub fn failed_deserialization<E: fmt::Debug>(
        tag: TransactionKindTag,
        error: Option<E>,
    ) -> Self {
        Self::FailedDeserialization {
            transaction_kind_tag: tag,
            error_message: error.map(|e| format!("{e:?}")).unwrap_or_default(),
        }
    }
}

impl fmt::Display for ParseTransactionKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMethodName { name } => {
                write!(
                    f,
                    "Error parsing transaction kind: Unknown method name {name}"
                )
            }
            Self::FailedDeserialization {
                transaction_kind_tag,
                error_message,
            } => {
                write!(f, "Error deserializing args for transaction of kind {transaction_kind_tag:?}. Error message: {error_message:?}")
            }
        }
    }
}

impl std::error::Error for ParseTransactionKindError {}
