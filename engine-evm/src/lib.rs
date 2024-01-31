#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code, unused_variables)]

extern crate alloc;

use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::Vec;
use aurora_engine_types::{H160, H256, U256};

#[cfg(feature = "evm-revm")]
mod revm;
#[cfg(feature = "evm-sputnikvm")]
mod sputnikvm;

#[cfg(feature = "evm-revm")]
pub use crate::revm::init_evm;
#[cfg(feature = "evm-sputnikvm")]
pub use crate::sputnikvm::init_evm;

pub trait EVMHandler {
    fn transact_create(&mut self);
    fn transact_create_fixed(&mut self);
    fn transact_call(&mut self);
}

pub struct TransactionInfo {
    pub origin: Address,
    pub value: Wei,
    pub input: Vec<u8>,
    pub address: Option<Address>,
    pub gas_limit: u64,
    pub access_list: Vec<(H160, Vec<H256>)>,
}

pub struct BlockInfo {
    pub gas_price: U256,
    pub current_account_id: AccountId,
    pub chain_id: [u8; 32],
}

pub struct EngineEVM<'env, I: IO, E: Env, H: EVMHandler> {
    io: I,
    env: &'env E,
    handler: H,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
}

impl<'env, I: IO + Copy, E: Env, H: EVMHandler> EngineEVM<'env, I, E, H> {
    /// Initialize Engine EVM.
    /// Where `handler` initialized from the feature flag.
    pub fn new(
        io: I,
        env: &'env E,
        transaction: &'env TransactionInfo,
        block: &'env BlockInfo,
        handler: H,
    ) -> Self {
        Self {
            io,
            env,
            handler,
            transaction,
            block,
        }
    }
}

impl<'env, I: IO + Copy, E: Env, H: EVMHandler> EVMHandler for EngineEVM<'env, I, E, H> {
    /// Invoke EVM transact-create
    fn transact_create(&mut self) {
        self.handler.transact_create();
    }

    /// Invoke EVM transact-create-fixed
    fn transact_create_fixed(&mut self) {
        self.handler.transact_create_fixed();
    }

    /// Invoke EVM transact-call
    fn transact_call(&mut self) {
        self.handler.transact_call();
    }
}
