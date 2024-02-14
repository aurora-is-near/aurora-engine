#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code, unused_variables)]

extern crate alloc;

use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
use aurora_engine_types::types::Wei;
use aurora_engine_types::Box;
use aurora_engine_types::Vec;
use aurora_engine_types::{H160, H256, U256};

#[cfg(feature = "evm-revm")]
mod revm;
#[cfg(feature = "evm-sputnikvm")]
mod sputnikvm;

pub use crate::sputnikvm::errors::{TransactErrorKind, TransactExecutionResult};

#[cfg(feature = "evm-revm")]
/// Init REVM
pub fn init_evm<'tx, 'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    _precompiles: Precompiles<'env, I, E, H::ReadOnly>,
) -> EngineEVM<'env, I, E, revm::REVMHandler<'env, I, E>> {
    let handler = revm::REVMHandler::new(io, env, transaction, block);
    EngineEVM::new(io, env, transaction, block, handler)
}

#[cfg(feature = "evm-sputnikvm")]
/// Init SputnikVM
pub fn init_evm<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    precompiles: Precompiles<'env, I, E, H::ReadOnly>,
    remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
) -> EngineEVM<sputnikvm::SputnikVMHandler<'env, I, E, H>> {
    let handler =
        sputnikvm::SputnikVMHandler::new(io, env, transaction, block, precompiles, remove_eth_fn);
    EngineEVM::new(handler)
}

#[cfg(feature = "evm-sputnikvm")]
pub fn config() -> Config {
    sputnikvm::CONFIG.clone().into()
}

#[cfg(feature = "integration-test")]
pub use sputnikvm::ApplyModify;

#[cfg(feature = "integration-test")]
pub fn apply<I: IO + Copy, E: Env>(io: I, env: &E, state_change: ApplyModify) {
    use evm::backend::ApplyBackend;
    let tx = TransactionInfo::default();
    let block = BlockInfo::default();
    let mut contract_state = sputnikvm::ContractState::new(io, env, &tx, &block, None);
    let state_change = evm::backend::Apply::Modify {
        address: state_change.address,
        basic: evm::backend::Basic {
            balance: state_change.basic_balance,
            nonce: state_change.basic_nonce,
        },
        code: state_change.code,
        storage: core::iter::empty(),
        reset_storage: false,
    };
    contract_state.apply(core::iter::once(state_change), core::iter::empty(), false);
}

/// Runtime configuration.
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

pub struct TransactResult {
    pub submit_result: SubmitResult,
    pub logs: Vec<Log>,
}

pub trait EVMHandler {
    fn transact_create(&mut self) -> TransactExecutionResult<TransactResult>;
    fn transact_call(&mut self) -> TransactExecutionResult<TransactResult>;
    fn view(&mut self) -> TransactExecutionResult<TransactionStatus>;
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log {
    pub address: H160,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
}

pub struct EngineEVM<H: EVMHandler> {
    handler: H,
}

impl<H: EVMHandler> EngineEVM<H> {
    /// Initialize Engine EVM.
    /// Where `handler` initialized from the feature flag.
    pub fn new(handler: H) -> Self {
        Self { handler }
    }
}

impl<H: EVMHandler> EVMHandler for EngineEVM<H> {
    /// Invoke EVM transact-create
    fn transact_create(&mut self) -> TransactExecutionResult<TransactResult> {
        self.handler.transact_create()
    }

    /// Invoke EVM transact-call
    fn transact_call(&mut self) -> TransactExecutionResult<TransactResult> {
        self.handler.transact_call()
    }

    /// View call
    fn view(&mut self) -> TransactExecutionResult<TransactionStatus> {
        self.handler.view()
    }
}
