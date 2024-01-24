#![allow(dead_code, unused_variables)]
use crate::revm::REVMHandler;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H160, H256, U256};

#[cfg(feature = "revm")]
mod revm;

pub trait EVMHandler {
    fn transact_create(&mut self);
    fn transact_create_fixed(&mut self);
    fn transact_call(&mut self);
}

#[derive(Clone, Debug)]
pub struct TransactionInfo {
    pub gas_price: U256,
    pub origin: Address,
    pub value: Wei,
    pub input: Vec<u8>,
    pub address: Option<Address>,
    pub gas_limit: u64,
    pub access_list: Vec<(H160, Vec<H256>)>,
}

pub struct EngineEVM<'env, I: IO, E: Env> {
    io: I,
    env: &'env E,
    handler: Box<dyn EVMHandler>,
    transaction: TransactionInfo,
}

impl<'env, I: IO + Copy + 'static, E: Env> EngineEVM<'env, I, E> {
    /// Initialize Engine EVM.
    /// Where `handler` initialized from the feature flag.
    pub fn new(io: I, env: &'env E, transaction: TransactionInfo) -> Self {
        #[cfg(feature = "revm")]
        let handler = Box::new(REVMHandler::new(io, &transaction));
        Self {
            io,
            env,
            handler,
            transaction,
        }
    }

    /// Invoke EVM transact-create
    pub fn transact_create(&mut self) {
        self.handler.transact_create();
    }

    /// Invoke EVM transact-create-fixed
    pub fn transact_create_fixed(&mut self) {
        self.handler.transact_create_fixed();
    }

    /// Invoke EVM transact-call
    pub fn transact_call(&mut self) {
        self.handler.transact_call();
    }
}
