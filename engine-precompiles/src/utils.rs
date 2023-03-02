use crate::prelude::Borrowed;
use core::num::TryFromIntError;
#[cfg(test)]
use evm::Context;
use evm::ExitError;

#[cfg(test)]
pub fn new_context() -> Context {
    Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: Default::default(),
    }
}

pub fn err_usize_conv(_e: TryFromIntError) -> ExitError {
    ExitError::Other(Borrowed("ERR_USIZE_CONVERSION"))
}

pub fn validate_no_value_attached_to_precompile(value: u128) -> Result<(), ExitError> {
    if value > 0 {
        // don't attach native token value to that precompile
        return Err(ExitError::Other(Borrowed("ATTACHED_VALUE_ERROR")));
    }
    Ok(())
}
