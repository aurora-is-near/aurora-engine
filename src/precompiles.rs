#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use evm::{Context, ExitError, ExitSucceed};
use primitive_types::H160;

#[allow(dead_code)]
pub fn no_precompiles(
    _address: H160,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<Result<(ExitSucceed, Vec<u8>, u64), ExitError>> {
    None
}

pub fn istanbul_precompiles(
    _address: H160,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<Result<(ExitSucceed, Vec<u8>, u64), ExitError>> {
    None // TODO: implement Istanbul precompiles
}
