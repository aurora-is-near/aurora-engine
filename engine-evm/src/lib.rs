#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code, unused_variables)]

extern crate alloc;

use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::engine::SubmitResult;
use aurora_engine_types::types::Wei;
use aurora_engine_types::Vec;
use aurora_engine_types::{H160, H256, U256};
use evm::backend::Log;

#[cfg(feature = "evm-revm")]
mod revm;
#[cfg(feature = "evm-sputnikvm")]
mod sputnikvm;

#[cfg(feature = "evm-revm")]
use crate::revm::REVMHandler;

#[cfg(feature = "evm-revm")]
/// Init REVM
pub fn init_evm<'tx, 'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    _precompiles: Precompiles<'env, I, E, H::ReadOnly>,
) -> EngineEVM<'env, I, E, REVMHandler<'env, I, E>> {
    let handler = REVMHandler::new(io, env, transaction, block);
    EngineEVM::new(io, env, transaction, block, handler)
}

#[cfg(feature = "evm-sputnikvm")]
use crate::sputnikvm::SputnikVMHandler;

#[cfg(feature = "evm-sputnikvm")]
/// Init SputnikVM
pub fn init_evm<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    precompiles: Precompiles<'env, I, E, H::ReadOnly>,
) -> EngineEVM<SputnikVMHandler<'env, I, E, H>> {
    let handler = SputnikVMHandler::new(io, env, transaction, block, precompiles);
    EngineEVM::new(handler)
}

pub trait EVMHandler {
    fn transact_create(&mut self);
    fn transact_create_fixed(&mut self);
    fn transact_call(&mut self) -> (SubmitResult, Vec<Log>);
}

pub struct TransactionInfo {
    pub origin: H160,
    pub value: Wei,
    pub input: Vec<u8>,
    pub address: Option<H160>,
    pub gas_limit: u64,
    pub access_list: Vec<(H160, Vec<H256>)>,
}

pub struct BlockInfo {
    pub gas_price: U256,
    pub current_account_id: AccountId,
    pub chain_id: [u8; 32],
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
    fn transact_create(&mut self) {
        self.handler.transact_create();
    }

    /// Invoke EVM transact-create-fixed
    fn transact_create_fixed(&mut self) {
        self.handler.transact_create_fixed();
    }

    /// Invoke EVM transact-call
    fn transact_call(&mut self) -> (SubmitResult, Vec<Log>) {
        self.handler.transact_call()
    }
}
