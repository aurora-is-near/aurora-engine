use crate::prelude::Borrowed;
use core::num::TryFromIntError;
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
