mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
mod secp256k1;

pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::prelude::{Address, Vec};
use evm::{Context, ExitError, ExitSucceed};

type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

#[allow(dead_code)]
pub fn no_precompiles(
    _address: Address,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // no precompiles supported
}

#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    match address.to_low_u64_be() {
        1 => Some(secp256k1::ecrecover_raw(input, target_gas)),
        2 => Some(hash::sha256(input, target_gas)),
        3 => Some(hash::ripemd160(input, target_gas)),
        4 => Some(identity::identity(input, target_gas)),
        5 => Some(modexp::modexp(input, target_gas)),
        6 => Some(bn128::alt_bn128_add(input, target_gas)),
        7 => Some(bn128::alt_bn128_mul(input, target_gas)),
        8 => Some(bn128::alt_bn128_pair(input, target_gas)),
        9 => Some(blake2::blake2f(input, target_gas)),
        // Not supported.
        _ => None,
    }
}

/// Checks the target gas with the cost of the operation.
fn check_gas(target_gas: Option<u64>, cost: u64) -> Result<(), ExitError> {
    if let Some(target_gas) = target_gas {
        if cost > target_gas {
            return Err(ExitError::OutOfGas);
        }
    } else {
        return Err(ExitError::OutOfGas);
    }

    Ok(())
}
