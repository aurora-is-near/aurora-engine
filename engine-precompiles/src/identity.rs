use crate::utils::calc_linear_cost_u32;
use crate::PrecompileError;
use evm::ExitError;

/// The base cost of the operation.
const IDENTITY_BASE: u64 = 15;

/// The cost per word.
const IDENTITY_PER_WORD: u64 = 3;

pub fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
    let input_len = u64::try_from(input.len()).map_err(utils::err_usize_conv)?;
    Ok(calc_linear_cost_u32(
        input_len,
        IDENTITY_BASE,
        IDENTITY_PER_WORD,
    ))
}

/// Takes the input bytes, copies them, and returns it as the output.
///
/// See: `https://ethereum.github.io/yellowpaper/paper.pdf`
/// See: `https://etherscan.io/address/0000000000000000000000000000000000000004`
pub fn run(input: &[u8], gas_limit: u64) -> crate::PrecompileResult {
    let gas_used = required_gas(input)?;
    if gas_used > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }
    Ok((gas_used, input.to_vec()))
}
