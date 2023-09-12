//! This module contains implementations for all top-level functions in the Aurora Engine
//! smart contract. All functions return `Result<(), ContractError>` because any output
//! is returned via the `IO` object and none of these functions are intended to panic.
//! Conditions which would cause the smart contract to panic are captured in the `ContractError`.
//! The actual panic happens via the `sdk_unwrap()` call where these functions are used in `lib.rs`.
//! The reason to isolate these implementations is so that they can be shared between both
//! the smart contract and the standalone.

use crate::{errors, state};
use aurora_engine_types::{account_id::AccountId, fmt, types::Address, Box};

pub mod admin;
pub mod connector;
pub mod evm_transactions;
pub mod xcc;

pub struct ContractError {
    pub message: Box<dyn AsRef<[u8]> + Send + Sync>,
}

impl ContractError {
    #[must_use]
    pub fn msg(self) -> ErrorMessage {
        ErrorMessage {
            message: self.message,
        }
    }
}

impl fmt::Debug for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = aurora_engine_types::str::from_utf8(self.message.as_ref().as_ref())
            .unwrap_or("NON_PRINTABLE_ERROR");
        f.debug_struct("ContractError")
            .field("message", &message)
            .finish()
    }
}

impl<T: AsRef<[u8]> + Send + Sync + 'static> From<T> for ContractError {
    fn from(value: T) -> Self {
        Self {
            message: Box::new(value),
        }
    }
}

/// This type is structurally the same as `ContractError`, but
/// importantly `ContractError` implements `From<T: AsRef<[u8]>>`
/// for easy usage in this module's function implementations, while
/// `ErrorMessage` implements `AsRef<[u8]>` for compatibility with
/// `sdk_unwrap`.
pub struct ErrorMessage {
    pub message: Box<dyn AsRef<[u8]>>,
}

impl AsRef<[u8]> for ErrorMessage {
    fn as_ref(&self) -> &[u8] {
        self.message.as_ref().as_ref()
    }
}

fn require_running(state: &state::EngineState) -> Result<(), ContractError> {
    if state.is_paused {
        return Err(errors::ERR_PAUSED.into());
    }
    Ok(())
}

fn require_paused(state: &state::EngineState) -> Result<(), ContractError> {
    if !state.is_paused {
        return Err(errors::ERR_RUNNING.into());
    }
    Ok(())
}

fn require_owner_only(
    state: &state::EngineState,
    predecessor_account_id: &AccountId,
) -> Result<(), ContractError> {
    if &state.owner_id != predecessor_account_id {
        return Err(errors::ERR_NOT_ALLOWED.into());
    }
    Ok(())
}

fn require_key_manager_only(
    state: &state::EngineState,
    predecessor_account_id: &AccountId,
) -> Result<(), ContractError> {
    let key_manager = state
        .key_manager
        .as_ref()
        .ok_or(errors::ERR_KEY_MANAGER_IS_NOT_SET)?;
    if key_manager != predecessor_account_id {
        return Err(errors::ERR_NOT_ALLOWED.into());
    }
    Ok(())
}

fn predecessor_address(predecessor_account_id: &AccountId) -> Address {
    aurora_engine_sdk::types::near_account_to_evm_address(predecessor_account_id.as_bytes())
}
