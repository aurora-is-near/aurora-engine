use crate::prelude::Borrowed;
use core::num::TryFromIntError;
use aurora_engine_types::U256;
#[cfg(test)]
use evm::Context;
use evm::ExitError;

#[cfg(test)]
pub fn new_context() -> Context {
    use aurora_engine_types::{H160, U256};

    Context {
        address: H160::default(),
        caller: H160::default(),
        apparent_value: U256::default(),
    }
}

pub const fn err_usize_conv(_e: TryFromIntError) -> ExitError {
    ExitError::Other(Borrowed("ERR_USIZE_CONVERSION"))
}

pub fn validate_no_value_attached_to_precompile(value: U256) -> Result<(), ExitError> {
    if value > U256::zero() {
        // don't attach native token value to that precompile
        return Err(ExitError::Other(Borrowed("ATTACHED_VALUE_ERROR")));
    }
    Ok(())
}
