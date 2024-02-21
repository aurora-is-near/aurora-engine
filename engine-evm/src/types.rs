use alloc::borrow::Cow;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::engine::SubmitResult;
use aurora_engine_types::types::Wei;
use aurora_engine_types::Vec;
use aurora_engine_types::{H160, H256, U256};

#[derive(Default, Debug, Clone)]
pub struct TransactionInfo {
    pub origin: H160,
    pub value: Wei,
    pub input: Vec<u8>,
    pub address: Option<H160>,
    pub gas_limit: u64,
    pub access_list: Vec<(H160, Vec<H256>)>,
}

#[derive(Default, Debug, Clone)]
pub struct BlockInfo {
    pub gas_price: U256,
    pub current_account_id: AccountId,
    pub chain_id: [u8; 32],
}

pub struct TransactResult {
    pub submit_result: SubmitResult,
    pub logs: Vec<Log>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log {
    pub address: H160,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
}

/// EVM Runtime configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Gas paid for extcode.
    pub gas_ext_code: u64,
    /// Gas paid for extcodehash.
    pub gas_ext_code_hash: u64,
    /// Gas paid for sstore set.
    pub gas_sstore_set: u64,
    /// Gas paid for sstore reset.
    pub gas_sstore_reset: u64,
    /// Gas paid for sstore refund.
    pub refund_sstore_clears: i64,
    /// EIP-3529
    pub max_refund_quotient: u64,
    /// Gas paid for BALANCE opcode.
    pub gas_balance: u64,
    /// Gas paid for SLOAD opcode.
    pub gas_sload: u64,
    /// Gas paid for cold SLOAD opcode.
    pub gas_sload_cold: u64,
    /// Gas paid for SUICIDE opcode.
    pub gas_suicide: u64,
    /// Gas paid for SUICIDE opcode when it hits a new account.
    pub gas_suicide_new_account: u64,
    /// Gas paid for CALL opcode.
    pub gas_call: u64,
    /// Gas paid for EXP opcode for every byte.
    pub gas_expbyte: u64,
    /// Gas paid for a contract creation transaction.
    pub gas_transaction_create: u64,
    /// Gas paid for a message call transaction.
    pub gas_transaction_call: u64,
    /// Gas paid for zero data in a transaction.
    pub gas_transaction_zero_data: u64,
    /// Gas paid for non-zero data in a transaction.
    pub gas_transaction_non_zero_data: u64,
    /// Gas paid per address in transaction access list (see EIP-2930).
    pub gas_access_list_address: u64,
    /// Gas paid per storage key in transaction access list (see EIP-2930).
    pub gas_access_list_storage_key: u64,
    /// Gas paid for accessing cold account.
    pub gas_account_access_cold: u64,
    /// Gas paid for accessing ready storage.
    pub gas_storage_read_warm: u64,
    /// EIP-1283.
    pub sstore_gas_metering: bool,
    /// EIP-1706.
    pub sstore_revert_under_stipend: bool,
    /// EIP-2929
    pub increase_state_access_gas: bool,
    /// EIP-3529
    pub decrease_clears_refund: bool,
    /// EIP-3541
    pub disallow_executable_format: bool,
    /// EIP-3651
    pub warm_coinbase_address: bool,
    /// Whether to throw out of gas error when
    /// CALL/CALLCODE/DELEGATECALL requires more than maximum amount
    /// of gas.
    pub err_on_call_with_more_gas: bool,
    /// Take l64 for callcreate after gas.
    pub call_l64_after_gas: bool,
    /// Whether empty account is considered exists.
    pub empty_considered_exists: bool,
    /// Whether create transactions and create opcode increases nonce by one.
    pub create_increase_nonce: bool,
    /// Stack limit.
    pub stack_limit: usize,
    /// Memory limit.
    pub memory_limit: usize,
    /// Call limit.
    pub call_stack_limit: usize,
    /// Create contract limit.
    pub create_contract_limit: Option<usize>,
    /// EIP-3860, maximum size limit of init_code.
    pub max_initcode_size: Option<usize>,
    /// Call stipend.
    pub call_stipend: u64,
    /// Has delegate call.
    pub has_delegate_call: bool,
    /// Has create2.
    pub has_create2: bool,
    /// Has revert.
    pub has_revert: bool,
    /// Has return data.
    pub has_return_data: bool,
    /// Has bitwise shifting.
    pub has_bitwise_shifting: bool,
    /// Has chain ID.
    pub has_chain_id: bool,
    /// Has self balance.
    pub has_self_balance: bool,
    /// Has ext code hash.
    pub has_ext_code_hash: bool,
    /// Has ext block fee. See [EIP-3198](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-3198.md)
    pub has_base_fee: bool,
    /// Has PUSH0 opcode. See [EIP-3855](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-3855.md)
    pub has_push0: bool,
    /// Whether the gasometer is running in estimate mode.
    pub estimate: bool,
}

/// Transact execution result
pub type TransactExecutionResult<T> = Result<T, TransactErrorKind>;

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize))]
pub enum ExitError {
    StackUnderflow,
    StackOverflow,
    InvalidJump,
    InvalidRange,
    DesignatedInvalid,
    CallTooDeep,
    CreateCollision,
    CreateContractLimit,
    OutOfOffset,
    OutOfGas,
    OutOfFund,
    #[allow(clippy::upper_case_acronyms)]
    PCUnderflow,
    CreateEmpty,
    Other(Cow<'static, str>),
    MaxNonce,
    InvalidCode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize))]
pub enum ExitFatal {
    NotSupported,
    UnhandledInterrupt,
    CallErrorAsFatal(ExitError),
    Other(Cow<'static, str>),
}

/// Errors with the EVM transact.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TransactErrorKind {
    /// Normal EVM errors.
    EvmError(ExitError),
    /// Fatal EVM errors.
    EvmFatal(ExitFatal),
}

impl From<ExitError> for TransactErrorKind {
    fn from(e: ExitError) -> Self {
        Self::EvmError(e)
    }
}

impl From<ExitFatal> for TransactErrorKind {
    fn from(e: ExitFatal) -> Self {
        Self::EvmFatal(e)
    }
}
