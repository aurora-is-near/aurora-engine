use evm::{ExitError, ExitFatal};

impl From<ExitError> for crate::ExitError {
    fn from(e: ExitError) -> Self {
        match e {
            ExitError::StackUnderflow => Self::StackUnderflow,
            ExitError::StackOverflow => Self::StackOverflow,
            ExitError::InvalidJump => Self::InvalidJump,
            ExitError::InvalidRange => Self::InvalidRange,
            ExitError::DesignatedInvalid => Self::DesignatedInvalid,
            ExitError::CallTooDeep => Self::CallTooDeep,
            ExitError::CreateCollision => Self::CreateCollision,
            ExitError::CreateContractLimit => Self::CreateContractLimit,
            ExitError::OutOfOffset => Self::OutOfOffset,
            ExitError::OutOfGas => Self::OutOfGas,
            ExitError::OutOfFund => Self::OutOfFund,
            ExitError::PCUnderflow => Self::PCUnderflow,
            ExitError::CreateEmpty => Self::CreateEmpty,
            ExitError::Other(val) => Self::Other(val),
            ExitError::MaxNonce => Self::MaxNonce,
            ExitError::InvalidCode(_) => Self::InvalidCode,
        }
    }
}

impl From<ExitFatal> for crate::ExitFatal {
    fn from(e: ExitFatal) -> Self {
        match e {
            ExitFatal::NotSupported => Self::NotSupported,
            ExitFatal::UnhandledInterrupt => Self::UnhandledInterrupt,
            ExitFatal::CallErrorAsFatal(err) => Self::CallErrorAsFatal(err.into()),
            ExitFatal::Other(val) => Self::Other(val),
        }
    }
}
