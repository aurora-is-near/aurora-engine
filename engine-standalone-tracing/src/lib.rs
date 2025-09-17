#![no_std]

extern crate alloc;

pub mod sputnik;
pub mod types;

pub use types::{
    Depth, LogMemory, LogStack, LogStorage, LogStorageKey, LogStorageValue, Logs, ProgramCounter,
    StepTransactionTrace, TraceLog, TransactionTrace,
};

#[derive(Clone, Copy, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum TraceKind {
    Transaction,
    CallFrame,
}
