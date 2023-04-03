use crate::parameters::NewCallArgs;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use borsh::{BorshDeserialize, BorshSerialize};

pub use error::EngineStateError;

/// Key for storing the state of the engine.
const STATE_KEY: &[u8; 5] = b"STATE";

/// Engine internal state, mostly configuration.
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum EngineState {
    V2(EngineStateV2),
    V1(EngineStateV1),
}

impl EngineState {
    #[must_use]
    pub const fn chain_id(&self) -> [u8; 32] {
        match self {
            Self::V2(state) => state.chain_id,
            Self::V1(state) => state.chain_id,
        }
    }

    pub fn set_chain_id(&mut self, chain_id: [u8; 32]) {
        match self {
            Self::V2(state) => state.chain_id = chain_id,
            Self::V1(state) => state.chain_id = chain_id,
        }
    }

    pub fn set_owner_id(&mut self, owner_id: AccountId) {
        match self {
            Self::V2(state) => state.owner_id = owner_id,
            Self::V1(state) => state.owner_id = owner_id,
        }
    }

    #[must_use]
    pub fn owner_id(&self) -> &AccountId {
        match self {
            Self::V2(state) => &state.owner_id,
            Self::V1(state) => &state.owner_id,
        }
    }

    #[must_use]
    pub const fn upgrade_delay_blocks(&self) -> u64 {
        match self {
            Self::V2(state) => state.upgrade_delay_blocks,
            Self::V1(state) => state.upgrade_delay_blocks,
        }
    }

    #[must_use]
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        Self::try_from_slice(bytes)
            .or_else(|_| EngineStateV1::try_from_slice(bytes).map(Self::V1))
            .ok()
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::V2(EngineStateV2::default())
    }
}

/// Engine internal state V2, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize, Default, Clone, PartialEq, Eq, Debug)]
pub struct EngineStateV2 {
    /// Chain id, according to the EIP-155 / ethereum-lists spec.
    pub chain_id: [u8; 32],
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

/// Engine internal state V1, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize, Default, Clone, PartialEq, Eq, Debug)]
pub struct EngineStateV1 {
    /// Chain id, according to the EIP-155 / ethereum-lists spec.
    pub chain_id: [u8; 32],
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// Account of the bridge prover.
    /// Use empty to not use base token as bridged asset.
    pub bridge_prover_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        Self::V2(EngineStateV2 {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
        })
    }
}

/// Gets the state from storage, if it exists otherwise it will error.
pub fn get_state<I: IO>(io: &I) -> Result<EngineState, EngineStateError> {
    io.read_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY))
        .map_or(Err(EngineStateError::NotFound), |bytes| {
            EngineState::deserialize(&bytes.to_vec()).ok_or(EngineStateError::DeserializationFailed)
        })
}

/// Saves state into the storage. Does not return the previous state.
pub fn set_state<I: IO>(io: &mut I, state: &EngineState) -> Result<(), EngineStateError> {
    io.write_storage(
        &bytes_to_key(KeyPrefix::Config, STATE_KEY),
        &state
            .try_to_vec()
            .map_err(|_| error::EngineStateError::SerializationFailed)?,
    );

    Ok(())
}

/// Engine state error module.
pub mod error {
    pub const ERR_STATE_NOT_FOUND: &[u8; 19] = b"ERR_STATE_NOT_FOUND";
    pub const ERR_STATE_SERIALIZATION_FAILED: &[u8; 26] = b"ERR_STATE_SERIALIZE_FAILED";
    pub const ERR_STATE_CORRUPTED: &[u8; 19] = b"ERR_STATE_CORRUPTED";

    #[derive(Debug)]
    /// Engine state error kinds.
    pub enum EngineStateError {
        /// The engine state is missing from storage, need to initialize with contract `new` method.
        NotFound,
        /// The engine state serialized had failed.
        SerializationFailed,
        /// The state of the engine is corrupted, possibly due to failed state migration.
        DeserializationFailed,
    }

    impl AsRef<[u8]> for EngineStateError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::NotFound => ERR_STATE_NOT_FOUND,
                Self::SerializationFailed => ERR_STATE_SERIALIZATION_FAILED,
                Self::DeserializationFailed => ERR_STATE_CORRUPTED,
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use std::cell::RefCell;

    #[test]
    fn test_missing_engine_state_is_not_found() {
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);

        let actual_error = get_state(&io).unwrap_err();
        let actual_error = std::str::from_utf8(actual_error.as_ref()).unwrap();
        let expected_error = std::str::from_utf8(error::ERR_STATE_NOT_FOUND).unwrap();
        assert_eq!(expected_error, actual_error);
    }

    #[test]
    fn test_empty_engine_state_is_corrupted() {
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);

        io.write_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY), &[]);
        let actual_error = get_state(&io).unwrap_err();
        let actual_error = std::str::from_utf8(actual_error.as_ref()).unwrap();
        let expected_error = std::str::from_utf8(error::ERR_STATE_CORRUPTED).unwrap();
        assert_eq!(expected_error, actual_error);
    }
}
