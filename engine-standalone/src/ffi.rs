use engine_standalone_tracing::{TraceLog, TransactionTrace};
use libc::{c_uchar, c_uint, c_ushort, size_t, uintmax_t};
use std::ffi::CString;

/// Translates a struct into a C struct.
pub trait IntoC<T> {
    /// A method used to consume a struct and convert it into a C-compatible
    /// struct.
    fn into_c(self) -> T;
}

#[repr(C)]
/// The C trace log of an execution on the EVM.
pub struct CTraceLog {
    /// The depth of the log.
    depth: c_uint,
    /// Any errors that may have occurred during execution.
    ///
    /// Empty if none.
    error: CString,
    /// Gas used to execute the transaction.
    gas: uintmax_t,
    /// Gas cost for the transaction.
    gas_cost: uintmax_t,
    /// The bounded memory.
    memory_ptr: *const [c_uchar; 32],
    /// The length of the memory vector.
    memory_len: size_t,
    /// The opcode as a byte.
    opcode: c_uchar, // opcode as byte
    /// The current program counter of the transaction.
    program_counter: c_uint,
    /// The local stack.
    stack_ptr: *const [c_uchar; 32],
    /// The length of the stack vector.
    stack_len: size_t,
    /// The storage of the logs as a set of tuples.
    storage_ptr: *const ([c_uchar; 32], [c_uchar; 32]),
    /// The length of the storage.
    storage_len: size_t,
}

impl From<TraceLog> for CTraceLog {
    fn from(log: TraceLog) -> Self {
        let error = match &log.error {
            Some(err) => CString::new(err.to_string()),
            None => CString::new(""),
        }
        .expect("CString::new failed");
        let (memory_ptr, memory_len) = {
            let len = log.memory.len();
            let memory = log.memory.clone();

            (memory.into_raw().as_ptr(), len)
        };
        let (stack_ptr, stack_len) = {
            let len = log.stack.len();
            let stack = log.stack.clone();

            (stack.into_raw().as_ptr(), len)
        };
        let (storage_ptr, storage_len) = {
            let storage_map = log.storage.clone();
            let storage: Vec<([u8; 32], [u8; 32])> = storage_map
                .into_iter()
                .map(|(key, value)| (key.into_raw(), value.into_raw()))
                .collect();

            (storage.as_ptr(), storage.len())
        };

        Self {
            depth: log.depth.into_u32(),
            error,
            gas: log.gas.as_u64(),
            gas_cost: log.gas_cost.as_u64(),
            memory_ptr,
            memory_len,
            opcode: log.opcode.as_u8(),
            program_counter: log.program_counter.into_u32(),
            stack_ptr,
            stack_len,
            storage_ptr,
            storage_len,
        }
    }
}

#[repr(C)]
pub struct CTransactionTrace {
    /// The total gas cost of the transaction.
    gas: uintmax_t,
    /// The return of the operation.
    result: CString,
    /// The collection of traces.
    logs_ptr: *const CTraceLog,
    /// The length of the logs vector.
    logs_len: size_t,
}

impl From<TransactionTrace> for CTransactionTrace {
    fn from(trace: TransactionTrace) -> Self {
        let logs = trace.logs().clone();
        let c_logs: Vec<CTraceLog> = logs.into_iter().map(CTraceLog::from).collect();
        let (logs_ptr, logs_len) = {
            let len = c_logs.len();
            (c_logs.as_ptr(), len)
        };

        Self {
            gas: trace.gas().as_u64(),
            result: CString::new(trace.result()).expect("CString::new failed"),
            logs_ptr,
            logs_len,
        }
    }
}

// Debug methods

/// Takes in a transaction hash and returns a `TransactionTrace`.
#[no_mangle]
pub extern "C" fn trace_transaction(_tx_hash: *const [c_uchar; 32]) -> *const CTransactionTrace {
    todo!()
}

// Storage getters

/// Gets the nonce of an Ethereum address at a given block.
/// Returns 0 on success, 1 on failure (block hash not found); the nonce variable is overwritten
/// with the requested nonce iff 0 is returned.
#[no_mangle]
pub extern "C" fn get_nonce(
    _block_hash: *const [c_uchar; 32],
    _address: *const [c_uchar; 20],
    _nonce_out: *mut uintmax_t,
) -> c_uchar {
    todo!()
}

/// Gets the balance of an Ethereum address at a given block.
///
/// Returns 0 on success, 1 on failure (block hash not found); the balance variable is overwritten
/// with the requested balance (big endian encoded) iff 0 is returned.
#[no_mangle]
pub extern "C" fn get_balance(
    _block_hash: *const [c_uchar; 32],
    _address: *const [c_uchar; 20],
    _balance_out: *mut [c_uchar; 32],
) -> c_uchar {
    todo!()
}

/// Returns the size of the EVM bytecode (in bytes) for the specified account at a given block.
///
/// Returns 0 on success, 1 on failure (block hash not found); the size variable is overwritten
/// with the requested balance (big endian encoded) iff 0 is returned.
#[no_mangle]
pub extern "C" fn get_code_size(
    _block_hash: *const [c_uchar; 32],
    _address: *const [c_uchar; 20],
    _size_out: *const c_uint,
) -> c_uchar {
    todo!()
}

/// Returns the byte slice with the code for the specified account at a given block.
///
/// Returns 0 on success, 1 on failure (block hash not found); the code variable is overwritten
/// with the requested balance (big endian encoded) iff 0 is returned. The size of the output slice
/// needed should be determined from `get_code_size`.
#[no_mangle]
pub extern "C" fn get_code(
    _block_hash: *const [c_uchar; 32],
    _address: *const [c_uchar; 20],
    _code_out: *mut c_uchar,
    _code_out_len: *mut c_uint,
) -> c_uchar {
    todo!()
}

/// Gets the state value for the provided address and key values at a given block.
/// Returns 0 on success, 1 on failure (block hash not found); the value variable is overwritten
/// with the requested balance (big endian encoded) iff 0 is returned.
#[no_mangle]
pub extern "C" fn get_state(
    _block_hash: *const [c_uchar; 32],
    _address: *const [c_uchar; 20],
    _key: *const [c_uchar; 32],
    _value_out: *mut [c_uchar; 32],
) -> c_uchar {
    todo!()
}

// Storage setters

/// Submit a transaction which was included in the given block. The transaction is RPL encoded.
/// This will update the storage to include the transaction, the diff it generated, and other state metadata (see storage details).
/// The return value is 0 on success. Non-zero return values will correspond to different errors that may occur (exact errors TBD).
#[no_mangle]
pub extern "C" fn submit_transaction(
    _block_hash: *const [c_uchar; 32],
    _block_height: *const uintmax_t,
    _transaction: *const c_uchar,
    _transaction_len: *const c_uint,
    _tx_position: *const c_ushort,
) -> c_uchar {
    todo!()
}
