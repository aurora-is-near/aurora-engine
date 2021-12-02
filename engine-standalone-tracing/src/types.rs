use aurora_engine_types::types::EthGas;
use aurora_engine_types::BTreeMap;
use evm_core::Opcode;
use std::ops::Index;

/// Depth of a log.
#[derive(Debug, Clone, Copy)]
pub struct Depth(u32);

impl Depth {
    /// Performs the conversion into a u32.
    pub fn into_u32(self) -> u32 {
        self.0
    }
}

/// A trace log memory.
#[derive(Debug, Clone)]
pub struct LogMemory(Vec<[u8; 32]>);

impl LogMemory {
    /// Returns the number of elements in the memory buffer.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no elements in the memory buffer.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Performs the conversion into a raw buffer.
    pub fn into_raw(self) -> Vec<[u8; 32]> {
        self.0
    }
}

/// The stack of the log.
#[derive(Debug, Clone)]
pub struct LogStack(Vec<[u8; 32]>);

impl LogStack {
    /// Returns the number of elements in the stack buffer.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no elements in the stack buffer.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Performs the conversion into a vector.
    pub fn into_raw(self) -> Vec<[u8; 32]> {
        self.0
    }
}

/// A trace log program counter.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProgramCounter(pub u32);

impl ProgramCounter {
    /// Performs the conversion into a u32.
    pub fn into_u32(self) -> u32 {
        self.0
    }
}

/// A storage key for the `LogStorage`.
#[derive(Debug, Clone)]
pub struct LogStorageKey([u8; 32]);

impl LogStorageKey {
    /// Performs the conversion into a 32 byte word.
    pub fn into_raw(self) -> [u8; 32] {
        self.0
    }
}

/// A storage value for the `LogStorage`.
#[derive(Debug, Clone)]
pub struct LogStorageValue([u8; 32]);

impl LogStorageValue {
    /// Performs the conversion into a 32 byte word.
    pub fn into_raw(self) -> [u8; 32] {
        self.0
    }
}

/// A map for `LogStorageKeys` to `LogStorageValue`s.
#[derive(Debug, Clone)]
pub struct LogStorage(BTreeMap<LogStorageKey, LogStorageValue>);

impl IntoIterator for LogStorage {
    type Item = (LogStorageKey, LogStorageValue);
    type IntoIter = std::collections::btree_map::IntoIter<LogStorageKey, LogStorageValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// The trace log of an execution on the EVM.
#[derive(Debug, Clone)]
pub struct TraceLog {
    /// The depth of the log.
    depth: Depth,
    /// Any errors that may have occurred during execution.
    error: Option<String>,
    /// Gas used to execute the transaction.
    gas: EthGas,
    /// Gas cost for the transaction.
    gas_cost: EthGas,
    /// The bounded memory.
    memory: LogMemory,
    /// The opcode as a byte.
    opcode: Opcode,
    /// The current program counter of the transaction.
    program_counter: ProgramCounter,
    /// The local stack.
    stack: LogStack,
    /// The storage of the execution.
    storage: LogStorage,
}

impl TraceLog {
    /// Returns the depth of the log.
    pub fn depth(&self) -> Depth {
        self.depth
    }

    /// Returns a potential error, if any in the execution.
    pub fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }

    /// Returns the gas consumed.
    pub fn gas(&self) -> EthGas {
        self.gas
    }

    /// Returns the gas cost of the execution.
    pub fn gas_cost(&self) -> EthGas {
        self.gas_cost
    }

    /// Returns the memory of the log.
    pub fn memory(&self) -> &LogMemory {
        &self.memory
    }

    /// Returns the opcode for the execution of the log.
    pub fn opcode(&self) -> Opcode {
        self.opcode
    }

    /// Returns the program counter for the log.
    pub fn program_counter(&self) -> ProgramCounter {
        self.program_counter
    }

    /// Returns the stack of the log.
    pub fn stack(&self) -> &LogStack {
        &self.stack
    }

    /// Returns the storage of the log.
    pub fn storage(&self) -> &LogStorage {
        &self.storage
    }
}

#[derive(Debug, Clone)]
pub struct Logs(Vec<TraceLog>);

impl Logs {
    /// Returns the number of logs.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no logs.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Index<usize> for Logs {
    type Output = TraceLog;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IntoIterator for Logs {
    type Item = TraceLog;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug)]
pub struct TransactionTrace {
    /// The total gas cost of the transaction.
    gas: EthGas,
    /// The result of the operation.
    result: String,
    /// The collection of traces.
    logs: Logs,
}

impl TransactionTrace {
    /// Constructs a new TransactionTrace with a given gas, return, and logs.
    #[allow(dead_code)]
    pub fn new(gas: EthGas, result: String, logs: Logs) -> TransactionTrace {
        Self { gas, result, logs }
    }

    /// Returns the EthGas associated with this transaction as a reference.
    pub fn gas(&self) -> EthGas {
        self.gas
    }

    /// Returns the return as a str reference.
    pub fn result(&self) -> &str {
        self.result.as_str()
    }

    /// Returns a reference to the logs.
    pub fn logs(&self) -> &Logs {
        &self.logs
    }
}

/// Consumes a `TransactionTrace` and provides the ability to step through each
/// execution of the transaction.
#[derive(Debug)]
pub struct StepTransactionTrace {
    /// The under-laying transaction trace.
    inner: TransactionTrace,
    /// The current step.
    step: usize,
}

impl StepTransactionTrace {
    /// Constructs a new `TraceStepper` with a given `TransactionTrace`.
    #[allow(dead_code)]
    pub fn new(transaction_trace: TransactionTrace) -> Self {
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
        if self.step > self.inner.logs.len() {
            None
        } else {
            self.step += 1;
            Some(&self.inner.logs[self.step])
        }
    }
}
