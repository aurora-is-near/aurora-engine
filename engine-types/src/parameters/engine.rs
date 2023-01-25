use crate::{
    account_id::AccountId,
    types::{Address, RawH256, RawU256, WeiU256},
    Vec,
};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResultLog {
    pub address: Address,
    pub topics: Vec<RawU256>,
    pub data: Vec<u8>,
}

/// The status of a transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TransactionStatus {
    Succeed(Vec<u8>),
    Revert(Vec<u8>),
    OutOfGas,
    OutOfFund,
    OutOfOffset,
    CallTooDeep,
}

impl TransactionStatus {
    pub fn is_ok(&self) -> bool {
        matches!(*self, TransactionStatus::Succeed(_))
    }

    pub fn is_revert(&self) -> bool {
        matches!(*self, TransactionStatus::Revert(_))
    }

    pub fn is_fail(&self) -> bool {
        *self == TransactionStatus::OutOfGas
            || *self == TransactionStatus::OutOfFund
            || *self == TransactionStatus::OutOfOffset
            || *self == TransactionStatus::CallTooDeep
    }
}

impl AsRef<[u8]> for TransactionStatus {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Succeed(_) => b"SUCCESS",
            Self::Revert(_) => errors::ERR_REVERT,
            Self::OutOfFund => errors::ERR_OUT_OF_FUNDS,
            Self::OutOfGas => errors::ERR_OUT_OF_GAS,
            Self::OutOfOffset => errors::ERR_OUT_OF_OFFSET,
            Self::CallTooDeep => errors::ERR_CALL_TOO_DEEP,
        }
    }
}

/// Borsh-encoded parameters for the `call`, `call_with_args`, `deploy_code`,
/// and `deploy_with_input` methods.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubmitResult {
    version: u8,
    pub status: TransactionStatus,
    pub gas_used: u64,
    pub logs: Vec<ResultLog>,
}

impl SubmitResult {
    /// Must be incremented when making breaking changes to the SubmitResult ABI.
    /// The current value of 7 is chosen because previously a `TransactionStatus` object
    /// was first in the serialization, which is an enum with less than 7 variants.
    /// Therefore, no previous `SubmitResult` would have began with a leading 7 byte,
    /// and this can be used to distinguish the new ABI (with version byte) from the old.
    const VERSION: u8 = 7;

    pub fn new(status: TransactionStatus, gas_used: u64, logs: Vec<ResultLog>) -> Self {
        Self {
            version: Self::VERSION,
            status,
            gas_used,
            logs,
        }
    }
}

/// Borsh-encoded parameters for the engine `call` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub struct FunctionCallArgsV2 {
    pub contract: Address,
    /// Wei compatible Borsh-encoded value field to attach an ETH balance to the transaction
    pub value: WeiU256,
    pub input: Vec<u8>,
}

/// Legacy Borsh-encoded parameters for the engine `call` function, to provide backward type compatibility
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub struct FunctionCallArgsV1 {
    pub contract: Address,
    pub input: Vec<u8>,
}

/// Deserialized values from bytes to current or legacy Borsh-encoded parameters
/// for passing to the engine `call` function, and to provide backward type compatibility
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub enum CallArgs {
    V2(FunctionCallArgsV2),
    V1(FunctionCallArgsV1),
}

impl CallArgs {
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        // For handling new input format (wrapped into call args enum) - for data structures with new arguments,
        // made for flexibility and extensibility.
        if let Ok(value) = Self::try_from_slice(bytes) {
            Some(value)
            // Fallback, for handling old input format,
            // i.e. input, formed as a raw (not wrapped into call args enum) data structure with legacy arguments,
            // made for backward compatibility.
        } else if let Ok(value) = FunctionCallArgsV1::try_from_slice(bytes) {
            Some(Self::V1(value))
            // Dealing with unrecognized input should be handled and result as an exception in a call site.
        } else {
            None
        }
    }
}

/// Borsh-encoded parameters for the `view` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq)]
pub struct ViewCallArgs {
    pub sender: Address,
    pub address: Address,
    pub amount: RawU256,
    pub input: Vec<u8>,
}

/// Borsh-encoded parameters for `deploy_erc20_token` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq, Clone)]
pub struct DeployErc20TokenArgs {
    pub nep141: AccountId,
}

/// Borsh-encoded parameters for `get_erc20_from_nep141` function.
pub type GetErc20FromNep141CallArgs = DeployErc20TokenArgs;

/// Borsh-encoded parameters for the `get_storage_at` function.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct GetStorageAtArgs {
    pub address: Address,
    pub key: RawH256,
}

pub mod errors {
    pub const ERR_REVERT: &[u8; 10] = b"ERR_REVERT";
    pub const ERR_OUT_OF_FUNDS: &[u8; 16] = b"ERR_OUT_OF_FUNDS";
    pub const ERR_CALL_TOO_DEEP: &[u8; 17] = b"ERR_CALL_TOO_DEEP";
    pub const ERR_OUT_OF_OFFSET: &[u8; 17] = b"ERR_OUT_OF_OFFSET";
    pub const ERR_OUT_OF_GAS: &[u8; 14] = b"ERR_OUT_OF_GAS";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_call_fail() {
        let bytes = [0; 71];
        let _ = ViewCallArgs::try_from_slice(&bytes).unwrap_err();
    }

    #[test]
    fn test_roundtrip_view_call() {
        let x = ViewCallArgs {
            sender: Address::from_array([1; 20]),
            address: Address::from_array([2; 20]),
            amount: [3; 32],
            input: vec![1, 2, 3],
        };
        let bytes = x.try_to_vec().unwrap();
        let res = ViewCallArgs::try_from_slice(&bytes).unwrap();
        assert_eq!(x, res);
    }

    #[test]
    fn test_call_args_deserialize() {
        let new_input = FunctionCallArgsV2 {
            contract: Address::from_array([0u8; 20]),
            value: WeiU256::default(),
            input: Vec::new(),
        };
        let legacy_input = FunctionCallArgsV1 {
            contract: Address::from_array([0u8; 20]),
            input: Vec::new(),
        };

        // Parsing bytes in a new input format - data structures (wrapped into call args enum) with new arguments,
        // made for flexibility and extensibility.

        // Using new input format (wrapped into call args enum) and data structure with new argument (`value` field).
        let input_bytes = CallArgs::V2(new_input.clone()).try_to_vec().unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, Some(CallArgs::V2(new_input.clone())));

        // Using new input format (wrapped into call args enum) and old data structure with legacy arguments,
        // this is allowed for compatibility reason.
        let input_bytes = CallArgs::V1(legacy_input.clone()).try_to_vec().unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, Some(CallArgs::V1(legacy_input.clone())));

        // Parsing bytes in an old input format - raw data structure (not wrapped into call args enum) with legacy arguments,
        // made for backward compatibility.

        // Using old input format (not wrapped into call args enum) - raw data structure with legacy arguments.
        let input_bytes = legacy_input.try_to_vec().unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, Some(CallArgs::V1(legacy_input)));

        // Using old input format (not wrapped into call args enum) - raw data structure with new argument (`value` field).
        // Data structures with new arguments allowed only in new input format for future extensibility reason.
        // Raw data structure (old input format) allowed only with legacy arguments for backward compatibility reason.
        // Unrecognized input should be handled and result as an exception in a call site.
        let input_bytes = new_input.try_to_vec().unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, None);
    }
}
