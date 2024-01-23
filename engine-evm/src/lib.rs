use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H160, H256, U256};

#[cfg(feature = "revm")]
mod revm;

pub struct EnginEVM<'env, I: IO, E: Env> {
    io: I,
    env: &'env E,
    gas_price: U256,
    origin: Address,
    value: Wei,
    input: Vec<u8>,
    address: Option<Address>,
    gas_limit: u64,
    access_list: Vec<(H160, Vec<H256>)>,
}
