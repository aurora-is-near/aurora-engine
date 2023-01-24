use crate::gas_token::GasToken;
use crate::parameters::NewCallArgs;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use borsh::{BorshDeserialize, BorshSerialize};

/// Key for storing the state of the engine.
const STATE_KEY: &[u8; 5] = b"STATE";

/// Engine internal state V2, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize, Default, Clone, PartialEq, Eq, Debug)]
pub struct EngineState {
    /// Chain id, according to the EIP-155 / ethereum-lists spec.
    pub chain_id: [u8; 32],
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
    /// The default gas token.
    pub default_gas_token: GasToken,
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        EngineState {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
            default_gas_token: GasToken::from_array(args.default_gas_token),
        }
    }
}

/// Gets the state from storage, if it exists otherwise it will error.
pub fn get_state<I: IO>(io: &I) -> Result<EngineState, error::EngineStateError> {
    match io.read_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY)) {
        None => Err(error::EngineStateError::NotFound),
        Some(bytes) => EngineState::try_from_slice(&bytes.to_vec())
            .map_err(|_| error::EngineStateError::DeserializationFailed),
    }
}

/// Saves state into the storage. Does not return the previous state.
pub fn set_state<I: IO>(io: &mut I, state: EngineState) -> Result<(), error::EngineStateError> {
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

pub mod legacy {
    use super::*;

    /// Migrates the state from the old format to the latest format.
    ///
    /// While this function works for a single v1 -> v2 migration, it would need to be
    /// altered to handle all earlier migrations if needed.
    pub fn migrate_state<I: IO>(io: &mut I) -> Result<(), error::EngineStateError> {
        // Get the engine legacy state.
        let engine_state_legacy = match io.read_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY))
        {
            None => Err(error::EngineStateError::NotFound),
            Some(bytes) => EngineStateLegacy::try_from_slice(&bytes.to_vec())
                .map_err(|_| error::EngineStateError::DeserializationFailed),
        }?;

        // Migrate to new engine state.
        let engine_state_new = EngineState {
            chain_id: engine_state_legacy.chain_id,
            owner_id: engine_state_legacy.owner_id,
            upgrade_delay_blocks: 0,
            default_gas_token: Default::default(),
        };

        // Set the new engine state.
        io.write_storage(
            &bytes_to_key(KeyPrefix::Config, STATE_KEY),
            &engine_state_new
                .try_to_vec()
                .map_err(|_| error::EngineStateError::SerializationFailed)?,
        );

        Ok(())
    }

    /// Engine internal state V1, mostly configuration.
    /// Should not contain anything large or enumerable.
    #[derive(BorshSerialize, BorshDeserialize, Default, Clone, PartialEq, Eq, Debug)]
    pub struct EngineStateLegacy {
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
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use std::sync::RwLock;

    #[test]
    fn test_missing_engine_state_is_not_found() {
        let storage = Storage::default();
        let storage = RwLock::new(storage);
        let io = StoragePointer(&storage);

        let actual_error = get_state(&io).unwrap_err();
        let actual_error = std::str::from_utf8(actual_error.as_ref()).unwrap();
        let expected_error = std::str::from_utf8(error::ERR_STATE_NOT_FOUND).unwrap();

        assert_eq!(expected_error, actual_error);
    }

    #[test]
    fn test_empty_engine_state_is_corrupted() {
        let storage = Storage::default();
        let storage = RwLock::new(storage);
        let mut io = StoragePointer(&storage);

        io.write_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY), &[]);
        let actual_error = get_state(&io).unwrap_err();
        let actual_error = std::str::from_utf8(actual_error.as_ref()).unwrap();
        let expected_error = std::str::from_utf8(error::ERR_STATE_CORRUPTED).unwrap();

        assert_eq!(expected_error, actual_error);
    }
}
