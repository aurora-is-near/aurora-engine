use crate::prelude::{H160, H256, U256, Vec};
use evm::{Context, ExitError, ExitSucceed};

type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

#[allow(dead_code)]
pub fn no_precompiles(
    _address: H160,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // not precompiles supported
}

#[allow(dead_code)]
pub fn istanbul_precompiles(
    _address: H160,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // TODO: implement Istanbul precompiles
}

#[cfg(test)]
mod tests {}
