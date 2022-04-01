use engine_standalone_storage::{self, sync::TransactionIncludedOutcome, Storage};
use tempfile::TempDir;

pub fn commit(storage: &mut Storage, outcome: &TransactionIncludedOutcome) {
    storage
        .set_transaction_included(outcome.hash, &outcome.info, &outcome.diff)
        .unwrap();
}

pub fn create_db() -> (TempDir, Storage) {
    let dir = TempDir::new().unwrap();
    let storage = Storage::open(dir.path()).unwrap();
    (dir, storage)
}
