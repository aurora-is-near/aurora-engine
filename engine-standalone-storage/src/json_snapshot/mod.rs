use crate::Storage;

pub mod types;

/// Write engine state directly into the Storage from a
/// JSON snapshot (which can be extracted from a NEAR RPC node).
pub fn initialize_engine_state(
    storage: &mut Storage,
    snapshot: types::JsonSnapshot,
) -> Result<(), error::Error> {
    // When we access engine storage, we are accessing the pre-state. The snapshot is giving
    // us a post-state, which of course is the pre-state of the following block.
    let block_height = snapshot.result.block_height + 1;

    let mut batch = rocksdb::WriteBatch::default();
    for entry in snapshot.result.values {
        let key = base64::decode(entry.key)?;
        let value = base64::decode(entry.value)?;
        let storage_key = crate::construct_engine_key(&key, block_height, 0);
        let storage_value = crate::diff::DiffValue::Modified(value);
        batch.put(storage_key, storage_value.try_to_bytes()?);
    }
    storage.db.write(batch)?;

    Ok(())
}

pub mod error {
    #[derive(Debug)]
    pub enum Error {
        Base64(base64::DecodeError),
        Rocksdb(rocksdb::Error),
        Borsh(std::io::Error),
    }

    impl From<base64::DecodeError> for Error {
        fn from(e: base64::DecodeError) -> Self {
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
    /// Requires a JSON snapshot to work. This can be obtained from https://github.com/aurora-is-near/contract-state
    #[test]
    #[ignore]
    fn test_consume_snapshot() {
        let snapshot = crate::json_snapshot::types::JsonSnapshot::load_from_file(
            "contract.aurora.block51077328.json",
        )
        .unwrap();
        let mut storage = crate::Storage::open("rocks_tmp/").unwrap();
        super::initialize_engine_state(&mut storage, snapshot).unwrap();
    }
}
