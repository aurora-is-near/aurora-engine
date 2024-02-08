use evm::{ExitError, ExitFatal};

/// Transact execution result
pub type TransactExecutionResult<T> = Result<T, TransactErrorKind>;

/// Errors with the EVM transact.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TransactErrorKind {
    /// Normal EVM errors.
    EvmError(ExitError),
    /// Fatal EVM errors.
    EvmFatal(ExitFatal),
}

impl From<ExitError> for TransactErrorKind {
    fn from(e: ExitError) -> Self {
        Self::EvmError(e)
    }
}

impl From<ExitFatal> for TransactErrorKind {
    fn from(e: ExitFatal) -> Self {
        Self::EvmFatal(e)
    }
}
