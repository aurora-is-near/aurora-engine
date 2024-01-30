use crate::{EVMHandler, EngineEVM, TransactionInfo};
use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
// use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::{executor, Config};

/// SputnikVM handler
pub struct SputnikVMHandler<'env, I: IO, E: Env, H> {
    env_state: &'env E,
    state: ContractState<'env, I, E>,
    precompiles: Precompiles<'env, I, E, H>,
}

/// Init REVM
pub fn init_evm<'tx, 'env, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler>(
    io: &I,
    env: &'env E,
    transaction: &'tx TransactionInfo,
    precompiles: Precompiles<'env, I, E, H>,
    config: &'env Config,
) -> EngineEVM<'tx, 'env, I, E, SputnikVMHandler<'env, I, E, H>> {
    let handler = SputnikVMHandler::new(io, env, &transaction, precompiles, config);
    EngineEVM::new(io, env, transaction, handler)
}

pub struct ContractState<'env, I: IO, E: Env> {
    io: I,
    env_state: &'env E,
}

impl<'env, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> SputnikVMHandler<'env, I, E, H> {
    pub fn new(
        io: &I,
        env_state: &'env E,
        transaction: &TransactionInfo,
        precompiles: Precompiles<'env, I, E, H>,
        config: &'env Config,
    ) -> Self {
        let _metadata = executor::stack::StackSubstateMetadata::new(transaction.gas_limit, config);
        todo!()
    }
}

impl<'env, I: IO + Copy, E: Env, H> EVMHandler for SputnikVMHandler<'env, I, E, H> {
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
