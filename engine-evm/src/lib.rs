#![cfg_attr(not(feature = "std"), no_std)]
#![deny(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::as_conversions
)]

extern crate alloc;
extern crate core;

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
#[allow(clippy::needless_pass_by_value)]
pub fn init_evm<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    _precompiles: Precompiles<'env, I, E, H::ReadOnly>,
    remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
) -> EngineEVM<revm::REVMHandler<'env, I, E>> {
    let handler = revm::REVMHandler::new(io, env, transaction, block, remove_eth_fn);
    EngineEVM::new(handler)
}

#[cfg(feature = "evm-sputnikvm")]
/// Init `SputnikVM`
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
#[must_use]
pub fn config() -> Config {
    revm::config()
}

#[cfg(feature = "evm-sputnikvm")]
#[must_use]
pub fn config() -> Config {
    sputnikvm::CONFIG.clone().into()
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
    pub const fn new(handler: H) -> Self {
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

#[cfg(feature = "integration-test")]
pub mod test_util {
    use aurora_engine_sdk::io::IO;
    use aurora_engine_types::storage::{address_to_key, KeyPrefix};
    use aurora_engine_types::types::{u256_to_arr, Address, Wei};
    use aurora_engine_types::{Vec, H160, U256};

    #[derive(Clone, Debug)]
    pub struct MintEthData {
        pub address: H160,
        pub balance: U256,
        pub nonce: U256,
        pub code: Option<Vec<u8>>,
    }

    pub fn mint_eth<I: IO + Copy>(mut io: I, accounts: Vec<MintEthData>) {
        for account in accounts {
            let address = Address::new(account.address);
            let old_nonce = get_nonce(&io, &address);
            let old_balance = get_balance(&io, &address).raw();

            if old_nonce != account.nonce {
                set_nonce(&mut io, &address, &account.nonce);
            }
            if old_balance != account.balance {
                set_balance(&mut io, &address, &Wei::new(account.balance));
            }
            if let Some(code) = account.code {
                set_code(&mut io, &address, &code);
            }
        }
    }

    fn get_balance<I: IO>(io: &I, address: &Address) -> Wei {
        let raw = io
            .read_u256(&address_to_key(KeyPrefix::Balance, address))
            .unwrap_or_else(|_| U256::zero());
        Wei::new(raw)
    }

    fn get_nonce<I: IO>(io: &I, address: &Address) -> U256 {
        io.read_u256(&address_to_key(KeyPrefix::Nonce, address))
            .unwrap_or_else(|_| U256::zero())
    }

    fn set_balance<I: IO>(io: &mut I, address: &Address, balance: &Wei) {
        io.write_storage(
            &address_to_key(KeyPrefix::Balance, address),
            &balance.to_bytes(),
        );
    }

    fn set_nonce<I: IO>(io: &mut I, address: &Address, nonce: &U256) {
        io.write_storage(
            &address_to_key(KeyPrefix::Nonce, address),
            &u256_to_arr(nonce),
        );
    }

    fn set_code<I: IO>(io: &mut I, address: &Address, code: &[u8]) {
        io.write_storage(&address_to_key(KeyPrefix::Code, address), code);
    }
}
