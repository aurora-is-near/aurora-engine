use crate::prelude::*;
use evm::ExitError;

/// Pad the input with a given length, if necessary.
pub(super) fn pad_input(input: &[u8], len: usize) -> Vec<u8> {
    let mut input = input.to_vec();
    input.resize(len, 0);

    input
}

/// Checks the target gas with the cost of the operation.
pub(super) fn check_gas(target_gas: Option<u64>, cost: u64) -> Result<(), ExitError> {
    if let Some(target_gas) = target_gas {
        if cost > target_gas {
            return Err(ExitError::OutOfGas);
        }
    } else {
        return Err(ExitError::OutOfGas);
    }

    Ok(())
}
