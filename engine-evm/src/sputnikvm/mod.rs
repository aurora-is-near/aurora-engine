use crate::{EVMHandler, TransactionInfo};
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;

/// SputnikVM handler
pub struct SputnikVMHandler<'env, I: IO, E: aurora_engine_sdk::env::Env> {
    env_state: &'env E,
    state: ContractState<'env, I, E>,
}

pub struct ContractState<'env, I: IO, E: aurora_engine_sdk::env::Env> {
    io: I,
    env_state: &'env E,
}

impl<'env, I: IO + Copy, E: Env> SputnikVMHandler<'env, I, E> {
    pub fn new(io: &I, env_state: &'env E, transaction: &TransactionInfo) -> Self {
        todo!()
    }
}

impl<'env, I: IO + Copy, E: Env> EVMHandler for SputnikVMHandler<'env, I, E> {
    fn transact_create(&mut self) {
        todo!()
    }

    fn transact_create_fixed(&mut self) {
        todo!()
    }

    fn transact_call(&mut self) {
        todo!()
    }
}
