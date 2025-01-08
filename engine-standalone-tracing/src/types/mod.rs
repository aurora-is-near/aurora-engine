use aurora_engine_types::types::EthGas;
use aurora_engine_types::BTreeMap;
use evm_core::Opcode;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::ops::Index;

pub mod call_tracer;

/// Depth of a log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Depth(u32);

impl Depth {
    /// Performs the conversion into a u32.
    #[must_use]
    pub const fn into_u32(self) -> u32 {
        self.0
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }

    pub fn decrement(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }

    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

/// A trace log memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogMemory(Vec<[u8; 32]>);

impl LogMemory {
    /// Returns the number of elements in the memory buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no elements in the memory buffer.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Performs the conversion into a raw buffer.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_raw(self) -> Vec<[u8; 32]> {
        self.0
    }
}

impl From<&[u8]> for LogMemory {
    fn from(bytes: &[u8]) -> Self {
        let mut result = Vec::with_capacity(bytes.len() / 32);
        let mut buf = [0u8; 32];
        for (i, b) in bytes.iter().enumerate() {
            let j = i % 32;
            buf[j] = *b;
            if j == 31 {
                result.push(buf);
            }
        }
        Self(result)
    }
}

/// The stack of the log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogStack(Vec<[u8; 32]>);

impl LogStack {
    /// Returns the number of elements in the stack buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no elements in the stack buffer.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Performs the conversion into a vector.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_raw(self) -> Vec<[u8; 32]> {
        self.0
    }
}

impl FromIterator<[u8; 32]> for LogStack {
    fn from_iter<T: IntoIterator<Item = [u8; 32]>>(iter: T) -> Self {
        let vec = iter.into_iter().collect();
        Self(vec)
    }
}

/// A trace log program counter.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProgramCounter(pub u32);

impl ProgramCounter {
    /// Performs the conversion into u32.
    #[must_use]
    pub const fn into_u32(self) -> u32 {
        self.0
    }
}

/// A storage key for the `LogStorage`.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogStorageKey(pub [u8; 32]);

impl LogStorageKey {
    /// Performs the conversion into a 32 byte word.
    #[must_use]
    pub const fn into_raw(self) -> [u8; 32] {
        self.0
    }
}

/// A storage value for the `LogStorage`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogStorageValue(pub [u8; 32]);

impl LogStorageValue {
    /// Performs the conversion into a 32 byte word.
    #[must_use]
    pub const fn into_raw(self) -> [u8; 32] {
        self.0
    }
}

/// A map for `LogStorageKeys` to `LogStorageValue`s.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogStorage(BTreeMap<LogStorageKey, LogStorageValue>);

impl LogStorage {
    pub fn insert(&mut self, key: LogStorageKey, value: LogStorageValue) {
        self.0.insert(key, value);
    }
}

impl IntoIterator for LogStorage {
    type Item = (LogStorageKey, LogStorageValue);
    type IntoIter = std::collections::btree_map::IntoIter<LogStorageKey, LogStorageValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// The trace log of an execution on the EVM.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TraceLog {
    /// The depth of the log.
    pub depth: Depth,
    /// Any errors that may have occurred during execution.
    pub error: Option<String>,
    /// Remaining (unused) gas.
    pub gas: EthGas,
    /// Gas cost for the opcode at this step.
    pub gas_cost: EthGas,
    /// The bounded memory.
    pub memory: LogMemory,
    /// The opcode as a byte.
    #[cfg_attr(feature = "serde", serde(with = "opcode_serde"))]
    pub opcode: Opcode,
    /// The current program counter of the transaction.
    pub program_counter: ProgramCounter,
    /// The local stack.
    pub stack: LogStack,
    /// The storage of the execution.
    pub storage: LogStorage,
}

impl Default for TraceLog {
    fn default() -> Self {
        Self {
            depth: Depth::default(),
            error: Option::default(),
            gas: EthGas::default(),
            gas_cost: EthGas::default(),
            memory: LogMemory::default(),
            opcode: Opcode::STOP,
            program_counter: ProgramCounter::default(),
            stack: LogStack::default(),
            storage: LogStorage::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Logs(pub Vec<TraceLog>);

impl Logs {
    /// Returns the number of logs.
    #[allow(dead_code)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no logs.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Index<usize> for Logs {
    type Output = TraceLog;

    fn index(&self, index: usize) -> &Self::Output {
        self.0.get(index).expect("index out of bounds")
    }
}

impl IntoIterator for Logs {
    type Item = TraceLog;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(dead_code)]
pub struct TransactionTrace {
    /// The total gas cost of the transaction.
    gas: EthGas,
    /// Flag indicating if the operation exited with an error.
    failed: bool,
    /// Bytes returned from the execution
    return_value: Vec<u8>,
    /// The collection of traces.
    struct_logs: Logs,
}

impl TransactionTrace {
    /// Constructs a new `TransactionTrace` with a given gas, return, and logs.
    #[must_use]
    pub const fn new(gas: EthGas, failed: bool, return_value: Vec<u8>, struct_logs: Logs) -> Self {
        Self {
            gas,
            failed,
            return_value,
            struct_logs,
        }
    }

    /// Returns the `EthGas` associated with this transaction as a reference.
    #[must_use]
    pub const fn gas(&self) -> EthGas {
        self.gas
    }

    /// Returns the output bytes of the transaction as a slice.
    #[must_use]
    pub fn result(&self) -> &[u8] {
        self.return_value.as_slice()
    }

    /// Returns a reference to the logs.
    #[must_use]
    pub const fn logs(&self) -> &Logs {
        &self.struct_logs
    }
}

/// Consumes a `TransactionTrace` and provides the ability to step through each
/// execution of the transaction.
#[derive(Debug, Default)]
pub struct StepTransactionTrace {
    /// The under-laying transaction trace.
    inner: TransactionTrace,
    /// The current step.
    step: usize,
}

impl StepTransactionTrace {
    /// Constructs a new `TraceStepper` with a given `TransactionTrace`.
    #[allow(dead_code)]
    #[must_use]
    pub const fn new(transaction_trace: TransactionTrace) -> Self {
        Self {
            inner: transaction_trace,
            step: 0,
        }
    }

    /// Steps through the logs, one at a time until it reaches the end of the
    /// execution.
    ///
    /// Returns a reference to a `TraceLog` if there is log, else it will return
    /// `None`.
    #[allow(dead_code)]
    pub fn step(&mut self) -> Option<&TraceLog> {
        // We subtract 2 from the length to avoid "index out of bounds" error,
        // given the else block increments the step by 1.
        if self.step > self.inner.struct_logs.len() - 2 {
            None
        } else {
            self.step += 1;
            Some(&self.inner.struct_logs[self.step])
        }
    }
}

// Custom serde serialization for opcode, given it is not provided upstream
// See here for custom serde serialization: https://serde.rs/custom-serialization.html
#[cfg(feature = "serde")]
mod opcode_serde {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(opcode: &evm_core::Opcode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(opcode.0)
    }

    struct U8Visitor;

    impl<'de> serde::de::Visitor<'de> for U8Visitor {
        type Value = u8;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an integer between 0 and 2^8 - 1")
        }

        fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<evm_core::Opcode, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(evm_core::Opcode(deserializer.deserialize_u8(U8Visitor)?))
    }
}
