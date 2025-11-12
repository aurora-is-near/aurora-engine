use aurora_engine_types::account_id::AccountId;
use engine_standalone_storage::{self, sync::TransactionIncludedOutcome, Storage};
use tempfile::TempDir;

pub fn commit(storage: &mut Storage, outcome: &TransactionIncludedOutcome) {
    storage
        .set_transaction_included(outcome.hash, &outcome.info, &outcome.diff)
        .unwrap();
}

pub fn create_db(account_id: &AccountId) -> (TempDir, Storage) {
    let dir = TempDir::new().unwrap();
    let storage = Storage::open_ensure_account_id(dir.path(), account_id).unwrap();
    (dir, storage)
}
