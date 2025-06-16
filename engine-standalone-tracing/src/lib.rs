pub mod sputnik;
pub mod types;

pub use types::{
    Depth, LogMemory, LogStack, LogStorage, LogStorageKey, LogStorageValue, Logs, ProgramCounter,
    StepTransactionTrace, TraceLog, TransactionTrace,
};

/// In order to use a tracer as a listener to listen events in the contract dynamic library,
/// the corresponding native function must be implemented.
/// See `aurora_engine_native::_native_traced_call_with_call_tracer`,
pub trait TracingNativeFn {
    const TRACING_NATIVE_FN: &'static str;
}
