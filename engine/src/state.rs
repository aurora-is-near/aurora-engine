use crate::parameters::NewCallArgs;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use aurora_engine_types::Vec;
use borsh::maybestd::io::Error as BorshError;
use borsh::{BorshDeserialize, BorshSerialize};

pub use error::EngineStateError;

/// Key for storing the state of the engine.
const STATE_KEY: &[u8; 5] = b"STATE";

/// Engine internal state, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct EngineState {
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

impl EngineState {
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let borshable: BorshableEngineState = self.into();
        borshable.try_to_vec().unwrap()
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, EngineStateError> {
        let borshable = match BorshableEngineState::try_from_slice(bytes) {
            Ok(b) => b,
            // To avoid DB migration, allow fallback on deserializing V1 EngineState
            Err(_) => {
                BorshableEngineStateV1::try_from_slice(bytes).map(BorshableEngineState::V1)?
            }
        };
        Self::try_from(borshable)
    }
}

impl From<BorshError> for EngineStateError {
    fn from(_err: BorshError) -> Self {
        Self::SerializationFailed
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
enum BorshableEngineState {
    V1(BorshableEngineStateV1),
    V2(BorshableEngineStateV2),
}

/// Engine internal state V1, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BorshableEngineStateV1 {
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

/// Engine internal state V2, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct BorshableEngineStateV2 {
    /// Chain id, according to the EIP-155 / ethereum-lists spec.
    pub chain_id: [u8; 32],
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

impl TryFrom<BorshableEngineState> for EngineState {
    type Error = EngineStateError;

    fn try_from(state: BorshableEngineState) -> Result<Self, Self::Error> {
        match state {
            BorshableEngineState::V1(state) => Ok(Self {
                chain_id: state.chain_id,
                owner_id: state.owner_id,
                bridge_prover_id: state.bridge_prover_id,
                upgrade_delay_blocks: state.upgrade_delay_blocks,
            }),
            BorshableEngineState::V2(state) => Ok(Self {
                chain_id: state.chain_id,
                owner_id: state.owner_id,
                bridge_prover_id: AccountId::default(),
                upgrade_delay_blocks: state.upgrade_delay_blocks,
            }),
        }
    }
}

impl From<&EngineState> for BorshableEngineState {
    fn from(state: &EngineState) -> Self {
        Self::V2(BorshableEngineStateV2 {
            chain_id: state.chain_id,
            owner_id: state.owner_id.clone(),
            upgrade_delay_blocks: state.upgrade_delay_blocks,
        })
    }
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        Self {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            bridge_prover_id: args.bridge_prover_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
        }
    }
}

/// Gets the state from storage, if it exists otherwise it will error.
pub fn get_state<I: IO>(io: &I) -> Result<EngineState, EngineStateError> {
    io.read_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY))
        .map_or_else(
            || Err(EngineStateError::NotFound),
            |bytes| {
                EngineState::try_from_slice(&bytes.to_vec())
                    .map_err(|_| EngineStateError::DeserializationFailed)
            },
        )
}

/// Saves state into the storage. Does not return the previous state.
pub fn set_state<I: IO>(io: &mut I, state: &EngineState) -> Result<(), EngineStateError> {
    io.write_storage(
        &bytes_to_key(KeyPrefix::Config, STATE_KEY),
        &state.to_bytes(),
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
