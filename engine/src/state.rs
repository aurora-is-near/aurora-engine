use crate::parameters::{LegacyNewCallArgs, NewCallArgs, NewCallArgsV2};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use aurora_engine_types::{Cow, Vec};
use borsh::{BorshDeserialize, BorshSerialize};

pub use error::EngineStateError;

/// Key for storing the state of the engine.
pub const STATE_KEY: &[u8; 5] = b"STATE";

/// Engine internal state, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct EngineState {
    /// Chain id, according to the EIP-155 / ethereum-lists spec.
    pub chain_id: [u8; 32],
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

impl EngineState {
    pub fn borsh_serialize(&self) -> Result<Vec<u8>, error::EngineStateError> {
        let borshable: BorshableEngineState = self.into();
        borshable
            .try_to_vec()
            .map_err(|_| EngineStateError::SerializationFailed)
    }

    /// Deserialization with lazy state migration.
    pub fn try_from_slice<I: IO + Copy>(
        bytes: &[u8],
        io: &I,
    ) -> Result<Self, error::EngineStateError> {
        let Ok(borshable) = BorshableEngineState::try_from_slice(bytes) else {
            let legacy = BorshableEngineStateV1::try_from_slice(bytes)
                .map_err(|_| EngineStateError::DeserializationFailed)?;
            let result = legacy.into();

            let mut io = *io;
            set_state(&mut io, &result)?;

            return Ok(result);
        };
        Ok(borshable.into())
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum BorshableEngineState<'a> {
    V1(BorshableEngineStateV1<'a>),
    V2(BorshableEngineStateV2<'a>),
}

#[derive(BorshSerialize, BorshDeserialize, Default, Clone, PartialEq, Eq, Debug)]
pub struct BorshableEngineStateV1<'a> {
    pub chain_id: [u8; 32],
    pub owner_id: Cow<'a, AccountId>,
    pub bridge_prover_id: Cow<'a, AccountId>,
    pub upgrade_delay_blocks: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Default, Clone, PartialEq, Eq, Debug)]
pub struct BorshableEngineStateV2<'a> {
    pub chain_id: [u8; 32],
    pub owner_id: Cow<'a, AccountId>,
    pub upgrade_delay_blocks: u64,
}

impl<'a> From<&'a EngineState> for BorshableEngineState<'a> {
    fn from(state: &'a EngineState) -> Self {
        Self::V2(BorshableEngineStateV2 {
            chain_id: state.chain_id,
            owner_id: Cow::Borrowed(&state.owner_id),
            upgrade_delay_blocks: state.upgrade_delay_blocks,
        })
    }
}

impl<'a> From<BorshableEngineState<'a>> for EngineState {
    fn from(state: BorshableEngineState<'a>) -> Self {
        match state {
            BorshableEngineState::V1(state) => state.into(),
            BorshableEngineState::V2(state) => state.into(),
        }
    }
}

impl<'a> From<BorshableEngineStateV1<'a>> for EngineState {
    fn from(state: BorshableEngineStateV1<'a>) -> Self {
        Self {
            chain_id: state.chain_id,
            owner_id: state.owner_id.into_owned(),
            upgrade_delay_blocks: state.upgrade_delay_blocks,
        }
    }
}

impl<'a> From<BorshableEngineStateV2<'a>> for EngineState {
    fn from(state: BorshableEngineStateV2<'a>) -> Self {
        Self {
            chain_id: state.chain_id,
            owner_id: state.owner_id.into_owned(),
            upgrade_delay_blocks: state.upgrade_delay_blocks,
        }
    }
}

impl From<LegacyNewCallArgs> for EngineState {
    fn from(args: LegacyNewCallArgs) -> Self {
        Self {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
        }
    }
}

impl From<NewCallArgsV2> for EngineState {
    fn from(args: NewCallArgsV2) -> Self {
        Self {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
        }
    }
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        match args {
            NewCallArgs::V1(args) => args.into(),
            NewCallArgs::V2(args) => args.into(),
        }
    }
}

/// Gets the state from storage, if it exists otherwise it will error.
pub fn get_state<I: IO + Copy>(io: &I) -> Result<EngineState, error::EngineStateError> {
    io.read_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY))
        .map_or_else(
            || Err(EngineStateError::NotFound),
            |bytes| EngineState::try_from_slice(&bytes.to_vec(), io),
        )
}

/// Saves state into the storage. Does not return the previous state.
pub fn set_state<I: IO>(io: &mut I, state: &EngineState) -> Result<(), EngineStateError> {
    io.write_storage(
        &bytes_to_key(KeyPrefix::Config, STATE_KEY),
        &state.borsh_serialize()?,
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
