use crate::{
    account_id::AccountId,
    types::{Address, RawH256, RawU256, WeiU256, Yocto},
    Vec,
};
#[cfg(not(feature = "borsh-compat"))]
use borsh::{BorshDeserialize, BorshSerialize};
#[cfg(feature = "borsh-compat")]
use borsh_compat::{self as borsh, BorshDeserialize, BorshSerialize};

/// Parameters for the `new` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum NewCallArgs {
    V1(LegacyNewCallArgs),
    V2(NewCallArgsV2),
}

impl NewCallArgs {
    pub fn deserialize(bytes: &[u8]) -> Result<Self, borsh::maybestd::io::Error> {
        Self::try_from_slice(bytes).map_or_else(
            |_| LegacyNewCallArgs::try_from_slice(bytes).map(Self::V1),
            Ok,
        )
    }
}

/// Old Borsh-encoded parameters for the `new` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct LegacyNewCallArgs {
    /// Chain id, according to the EIP-115 / ethereum-lists spec.
    pub chain_id: RawU256,
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// Account of the bridge prover.
    /// Use empty to not use base token as bridged asset.
    pub bridge_prover_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct NewCallArgsV2 {
    /// Chain id, according to the EIP-115 / ethereum-lists spec.
    pub chain_id: RawU256,
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

/// Borsh-encoded parameters for the `set_owner` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetOwnerArgs {
    pub new_owner: AccountId,
}

/// Borsh-encoded parameters for the `set_upgrade_delay_blocks` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetUpgradeDelayBlocksArgs {
    pub upgrade_delay_blocks: u64,
}

/// Borsh-encoded (genesis) account balance used by the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountBalance {
    pub address: Address,
    pub balance: RawU256,
}

/// Borsh-encoded submit arguments used by the `submit_with_args` function.
#[derive(Default, Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct SubmitArgs {
    /// Bytes of the transaction.
    pub tx_data: Vec<u8>,
    /// Max gas price the user is ready to pay for the transaction.
    pub max_gas_price: Option<u128>,
    /// Address of the `ERC20` token the user prefers to pay in.
    pub gas_token_address: Option<Address>,
}

/// Borsh-encoded parameters for the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BeginChainArgs {
    pub chain_id: RawU256,
    pub genesis_alloc: Vec<AccountBalance>,
}

/// Borsh-encoded parameters for the `begin_block` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BeginBlockArgs {
    /// The current block's hash (for replayer use).
    pub hash: RawU256,
    /// The current block's beneficiary address.
    pub coinbase: Address,
    /// The current block's timestamp (in seconds since the Unix epoch).
    pub timestamp: RawU256,
    /// The current block's number (the genesis block is number zero).
    pub number: RawU256,
    /// The current block's difficulty.
    pub difficulty: RawU256,
    /// The current block's gas limit.
    pub gaslimit: RawU256,
}

/// Fungible token storage balance
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct StorageBalance {
    pub total: Yocto,
    pub available: Yocto,
}

impl StorageBalance {
    #[must_use]
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RegisterRelayerCallArgs {
    pub address: Address,
}

pub type PausedMask = u8;

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PauseEthConnectorCallArgs {
    pub paused_mask: PausedMask,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PausePrecompilesCallArgs {
    pub paused_mask: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResultLog {
    pub address: Address,
    pub topics: Vec<RawU256>,
    pub data: Vec<u8>,
}

/// The status of a transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TransactionStatus {
    Succeed(Vec<u8>),
    Revert(Vec<u8>),
    OutOfGas,
    OutOfFund,
    OutOfOffset,
    CallTooDeep,
}

impl TransactionStatus {
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(*self, Self::Succeed(_))
    }

    #[must_use]
    pub const fn is_revert(&self) -> bool {
        matches!(*self, Self::Revert(_))
    }

    #[must_use]
    pub fn is_fail(&self) -> bool {
        *self == Self::OutOfGas
            || *self == Self::OutOfFund
            || *self == Self::OutOfOffset
            || *self == Self::CallTooDeep
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
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubmitResult {
    version: u8,
    pub status: TransactionStatus,
    pub gas_used: u64,
    pub logs: Vec<ResultLog>,
}

impl SubmitResult {
    /// Must be incremented when making breaking changes to the `SubmitResult` ABI.
    /// The current value of 7 is chosen because previously a `TransactionStatus` object
    /// was first in the serialization, which is an enum with less than 7 variants.
    /// Therefore, no previous `SubmitResult` would have began with a leading 7 byte,
    /// and this can be used to distinguish the new ABI (with version byte) from the old.
    const VERSION: u8 = 7;

    #[must_use]
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
    #[must_use]
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        Self::try_from_slice(bytes).map_or_else(
            |_| {
                FunctionCallArgsV1::try_from_slice(bytes)
                    .map_or(None, |value| Some(Self::V1(value)))
            },
            Some,
        )
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
    use crate::{account_id::ParseAccountError, String, ToString};

    pub const ERR_REVERT: &[u8; 10] = b"ERR_REVERT";
    pub const ERR_OUT_OF_FUNDS: &[u8; 16] = b"ERR_OUT_OF_FUNDS";
    pub const ERR_CALL_TOO_DEEP: &[u8; 17] = b"ERR_CALL_TOO_DEEP";
    pub const ERR_OUT_OF_OFFSET: &[u8; 17] = b"ERR_OUT_OF_OFFSET";
    pub const ERR_OUT_OF_GAS: &[u8; 14] = b"ERR_OUT_OF_GAS";

    #[derive(Debug)]
    pub enum ParseTypeFromJsonError {
        Json(String),
        InvalidAccount(ParseAccountError),
    }

    impl From<serde_json::Error> for ParseTypeFromJsonError {
        fn from(e: serde_json::Error) -> Self {
            Self::Json(e.to_string())
        }
    }

    impl From<ParseAccountError> for ParseTypeFromJsonError {
        fn from(e: ParseAccountError) -> Self {
            Self::InvalidAccount(e)
        }
    }

    impl AsRef<[u8]> for ParseTypeFromJsonError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::Json(e) => e.as_bytes(),
                Self::InvalidAccount(e) => e.as_ref(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_call_fail() {
        let bytes = [0; 71];
        let _args = ViewCallArgs::try_from_slice(&bytes).unwrap_err();
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
