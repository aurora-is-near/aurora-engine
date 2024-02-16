#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code, unused_variables)]

extern crate alloc;

use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::parameters::engine::TransactionStatus;
use aurora_engine_types::{types::Wei, Box};

#[cfg(feature = "evm-revm")]
mod revm;
#[cfg(feature = "evm-sputnikvm")]
mod sputnikvm;
mod types;

pub use types::{
    BlockInfo, Config, ExitError, ExitFatal, Log, TransactErrorKind, TransactExecutionResult,
    TransactResult, TransactionInfo,
};

#[cfg(feature = "evm-revm")]
/// Init REVM
pub fn init_evm<'tx, 'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    _precompiles: Precompiles<'env, I, E, H::ReadOnly>,
    _remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
) -> EngineEVM<revm::REVMHandler<'env, I, E>> {
    let handler = revm::REVMHandler::new(io, env, transaction, block);
    EngineEVM::new(handler)
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

#[cfg(feature = "evm-revm")]
pub fn config() -> Config {
    todo!()
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

pub trait EVMHandler {
    fn transact_create(&mut self) -> TransactExecutionResult<TransactResult>;
    fn transact_call(&mut self) -> TransactExecutionResult<TransactResult>;
    fn view(&mut self) -> TransactExecutionResult<TransactionStatus>;
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
