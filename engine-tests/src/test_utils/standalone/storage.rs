use aurora_engine_types::H256;
use engine_standalone_storage::{
    self,
    sync::types::{TransactionKind, TransactionMessage},
    Storage,
};
use tempfile::TempDir;

pub fn commit(
    storage: &mut Storage,
    diff: engine_standalone_storage::Diff,
    block_hash: H256,
    transaction_position: u16,
    transaction_hash: H256,
) {
    let tx_msg = TransactionMessage {
        block_hash,
        near_receipt_id: H256::zero(),
        position: transaction_position,
        succeeded: true,
        signer: "placeholder.near".parse().unwrap(),
        caller: "placeholder.near".parse().unwrap(),
        attached_near: 0,
        transaction: TransactionKind::Unknown,
    };
    storage
        .set_transaction_included(transaction_hash, &tx_msg, &diff)
        .unwrap();
}

pub fn create_db() -> (TempDir, Storage) {
    let dir = TempDir::new().unwrap();
    let storage = Storage::open(dir.path()).unwrap();
    (dir, storage)
}
