//! Tests to ensure we can use serde to serialize various types.

use aurora_engine::{
    engine::{EngineError, EngineErrorKind},
    parameters::{ResultLog, SubmitResult, TransactionStatus},
};
use aurora_engine_transactions::eip_2930::AccessTuple;
use aurora_engine_types::parameters::engine::EvmErrorKind;
use aurora_engine_types::{types::Address, H160};

#[test]
fn test_serde_submit_result() {
    let result = SubmitResult::new(
        TransactionStatus::Error(EvmErrorKind::OutOfFund),
        0,
        vec![ResultLog {
            address: Address::default(),
            topics: Vec::new(),
            data: Vec::new(),
        }],
    );
    let serialized = serde_json::to_value(result).unwrap();
    assert!(serialized.is_object());
}

#[test]
fn test_serde_engine_error() {
    let engine_error = EngineError {
        kind: EngineErrorKind::GasOverflow,
        gas_used: 0,
    };
    let serialized = serde_json::to_value(engine_error).unwrap();
    assert!(serialized.is_object());
}

#[test]
fn test_serde_access_tuple() {
    let tuple = AccessTuple {
        address: H160::default(),
        storage_keys: Vec::new(),
    };
    let serialized = serde_json::to_value(tuple).unwrap();
    assert!(serialized.is_object());
}
