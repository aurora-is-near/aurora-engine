use aurora_engine_hashchain::{
    bloom::{self, Bloom},
    error::BlockchainHashchainError,
    hashchain::Hashchain,
    wrapped_io::{CachedIO, IOCache},
};
use aurora_engine_sdk::{
    env::Env,
    io::{IO, StorageIntermediate},
};
use aurora_engine_types::{
    parameters::engine::SubmitResult,
    storage::{self, KeyPrefix},
};
use core::cell::RefCell;

use crate::contract_methods::ContractError;

pub const HASHCHAIN_STATE: &[u8] = b"HC_STATE";

pub fn with_hashchain<I, E, T, F>(
    mut io: I,
    env: &E,
    function_name: &str,
    f: F,
) -> Result<T, ContractError>
where
    I: IO + Copy,
    E: Env,
    F: for<'a> FnOnce(CachedIO<'a, I>) -> Result<T, ContractError>,
{
    let block_height = env.block_height();
    let maybe_hashchain = load_hashchain(&io, block_height)?;

    let cache = RefCell::new(IOCache::default());
    let hashchain_io = CachedIO::new(io, &cache);
    let result = f(hashchain_io)?;

    if let Some(mut hashchain) = maybe_hashchain {
        let cache_ref = cache.borrow();
        hashchain.add_block_tx(
            block_height,
            function_name,
            &cache_ref.input,
            &cache_ref.output,
            &Bloom::default(),
        )?;
        save_hashchain(&mut io, &hashchain)?;
    }

    Ok(result)
}

pub fn with_logs_hashchain<I, E, F>(
    mut io: I,
    env: &E,
    function_name: &str,
    f: F,
) -> Result<SubmitResult, ContractError>
where
    I: IO + Copy,
    E: Env,
    F: for<'a> FnOnce(CachedIO<'a, I>) -> Result<SubmitResult, ContractError>,
{
    let block_height = env.block_height();
    let maybe_hashchain = load_hashchain(&io, block_height)?;

    let cache = RefCell::new(IOCache::default());
    let hashchain_io = CachedIO::new(io, &cache);
    let result = f(hashchain_io)?;

    if let Some(mut hashchain) = maybe_hashchain {
        let log_bloom = bloom::get_logs_bloom(&result.logs);
        let cache_ref = cache.borrow();
        hashchain.add_block_tx(
            block_height,
            function_name,
            &cache_ref.input,
            &cache_ref.output,
            &log_bloom,
        )?;
        save_hashchain(&mut io, &hashchain)?;
    }

    Ok(result)
}

fn load_hashchain<I: IO>(io: &I, block_height: u64) -> Result<Option<Hashchain>, ContractError> {
    let mut maybe_hashchain = read_current_hashchain(io)?;
    if let Some(hashchain) = maybe_hashchain.as_mut() {
        if block_height > hashchain.get_current_block_height() {
            hashchain.move_to_block(block_height)?;
        }
    }
    Ok(maybe_hashchain)
}

pub fn read_current_hashchain<I: IO>(io: &I) -> Result<Option<Hashchain>, ContractError> {
    let key = storage::bytes_to_key(KeyPrefix::Hashchain, HASHCHAIN_STATE);
    let maybe_hashchain = io.read_storage(&key).map_or(Ok(None), |value| {
        let bytes = value.to_vec();
        Hashchain::try_deserialize(&bytes)
            .map(Some)
            .map_err(|_| BlockchainHashchainError::DeserializationFailed)
    })?;
    Ok(maybe_hashchain)
}

pub fn save_hashchain<I: IO>(io: &mut I, hashchain: &Hashchain) -> Result<(), ContractError> {
    let key = storage::bytes_to_key(KeyPrefix::Hashchain, HASHCHAIN_STATE);
    let bytes = hashchain
        .try_serialize()
        .map_err(|_| BlockchainHashchainError::SerializationFailed)?;
    io.write_storage(&key, &bytes);
    Ok(())
}
