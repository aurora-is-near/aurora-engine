#![deny(clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions, clippy::missing_panics_doc)]
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
