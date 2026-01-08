use crate::Storage;

pub mod types;

/// Write engine state directly into the Storage from a
/// JSON snapshot (which can be extracted from a NEAR RPC node).
pub fn initialize_engine_state(
    storage: &Storage,
    snapshot: types::JsonSnapshot,
) -> Result<(), error::Error> {
    // The snapshot is giving us a post-state, so we insert it right at the end of its block height.
    let block_height = snapshot.result.block_height;
    let transaction_position = u16::MAX;

    let mut batch = rocksdb::WriteBatch::default();
    for entry in snapshot.result.values {
        let key = aurora_engine_sdk::base64::decode(entry.key)?;
        let value = aurora_engine_sdk::base64::decode(entry.value)?;
        let storage_key = crate::construct_engine_key(&key, block_height, transaction_position);
        let storage_value = crate::diff::DiffValue::Modified(value);
        batch.put(storage_key, storage_value.try_to_bytes()?);
    }
    storage.db.write(batch)?;

    Ok(())
}

pub mod error {
    use aurora_engine_sdk::base64::DecodeError;

    #[derive(Debug)]
    pub enum Error {
        Base64(DecodeError),
        Rocksdb(rocksdb::Error),
        Borsh(std::io::Error),
    }

    impl From<DecodeError> for Error {
        fn from(e: DecodeError) -> Self {
            Self::Base64(e)
        }
    }

    impl From<rocksdb::Error> for Error {
        fn from(e: rocksdb::Error) -> Self {
            Self::Rocksdb(e)
        }
    }

    impl From<std::io::Error> for Error {
        fn from(e: std::io::Error) -> Self {
            Self::Borsh(e)
        }
    }
}

#[cfg(test)]
mod test {
    /// Requires a JSON snapshot to work. This can be obtained from `https://github.com/aurora-is-near/contract-state`
    #[test]
    #[ignore]
    fn test_consume_snapshot() {
        let snapshot = crate::json_snapshot::types::JsonSnapshot::load_from_file(
            "contract.aurora.block51077328.json",
        )
        .unwrap();
        let storage =
            crate::Storage::open_ensure_account_id("rocks_tmp/", &"aurora".parse().unwrap())
                .unwrap();
        super::initialize_engine_state(&storage, snapshot).unwrap();
    }
}
