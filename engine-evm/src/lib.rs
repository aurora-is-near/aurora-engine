#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code, unused_variables)]

extern crate alloc;

use crate::revm::REVMHandler;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{Box, Vec};
use aurora_engine_types::{H160, H256, U256};

#[cfg(feature = "revm")]
mod revm;

pub trait EVMHandler {
    fn transact_create(&mut self);
    fn transact_create_fixed(&mut self);
    fn transact_call(&mut self);
}

pub type SetBalanceHandler = dyn Fn(Box<Address>, Box<Wei>);
pub type EnvTimeStamp = dyn Fn() -> u64;
pub type EnvCoinbase = dyn Fn() -> [u8; 20];
pub type EnvBlockHeight = dyn Fn() -> u64;

// #[derive(Clone, Debug)]
pub struct TransactionInfo {
    pub gas_price: U256,
    pub origin: Address,
    pub value: Wei,
    pub input: Vec<u8>,
    pub address: Option<Address>,
    pub gas_limit: u64,
    pub access_list: Vec<(H160, Vec<H256>)>,
    // pub set_balance_handler: Box<SetBalanceHandler>,
    // pub time_stamp: Box<EnvTimeStamp>,
    // pub coinbase: Box<EnvCoinbase>,
    // pub block_height: Rc<EnvBlockHeight>,
}

pub struct EngineEVM<'tx> {
    handler: Box<dyn EVMHandler>,
    transaction: &'tx TransactionInfo,
}

pub struct EnvInfo {}

impl<'tx> EngineEVM<'tx> {
    /// Initialize Engine EVM.
    /// Where `handler` initialized from the feature flag.
    pub fn new(transaction: &'tx TransactionInfo) -> Self {
        #[cfg(feature = "revm")]
        let handler = Box::new(REVMHandler::new(transaction));
        Self {
            handler,
            transaction,
        }
    }
}

impl<'tx> EVMHandler for EngineEVM<'tx> {
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
