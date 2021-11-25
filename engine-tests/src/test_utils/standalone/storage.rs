use aurora_engine_types::H256;
use engine_standalone_storage::{self, Storage};
use tempfile::TempDir;

pub fn commit(
    storage: &mut Storage,
    diff: engine_standalone_storage::Diff,
    block_hash: H256,
    transaction_position: u16,
    transaction_hash: H256,
) {
    let tx_included = engine_standalone_storage::TransactionIncluded {
        block_hash,
        position: transaction_position,
    };
    storage
        .set_transaction_included(transaction_hash, &tx_included, &diff)
        .unwrap();
}

pub fn create_db() -> (TempDir, Storage) {
    let dir = TempDir::new().unwrap();
    let storage = Storage::open(dir.path()).unwrap();
    (dir, storage)
}
