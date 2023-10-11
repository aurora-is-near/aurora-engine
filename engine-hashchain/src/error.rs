pub const ERR_STATE_NOT_FOUND: &[u8; 19] = b"ERR_STATE_NOT_FOUND";
pub const ERR_STATE_SERIALIZATION_FAILED: &[u8; 26] = b"ERR_STATE_SERIALIZE_FAILED";
pub const ERR_STATE_CORRUPTED: &[u8; 19] = b"ERR_STATE_CORRUPTED";
pub const ERR_BLOCK_HEIGHT_INCORRECT: &[u8; 26] = b"ERR_BLOCK_HEIGHT_INCORRECT";
pub const ERR_REQUIRES_FEATURE_INTEGRATION_TEST: &[u8; 37] =
    b"ERR_REQUIRES_FEATURE_INTEGRATION_TEST";

#[derive(Debug)]
/// Blockchain Hashchain Error
pub enum BlockchainHashchainError {
    /// The state is missing from storage, need to initialize with contract `new` method.
    NotFound,
    /// The state serialized had failed.
    SerializationFailed,
    /// The state is corrupted, possibly due to failed state migration.
    DeserializationFailed,
    /// The block height is incorrect regarding the current block height.
    BlockHeightIncorrect,
    /// Some functionality requires integration-test feature.
    RequiresFeatureIntegrationTest,
}

impl AsRef<[u8]> for BlockchainHashchainError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::NotFound => ERR_STATE_NOT_FOUND,
            Self::SerializationFailed => ERR_STATE_SERIALIZATION_FAILED,
            Self::DeserializationFailed => ERR_STATE_CORRUPTED,
            Self::BlockHeightIncorrect => ERR_BLOCK_HEIGHT_INCORRECT,
            Self::RequiresFeatureIntegrationTest => ERR_REQUIRES_FEATURE_INTEGRATION_TEST,
        }
    }
}
