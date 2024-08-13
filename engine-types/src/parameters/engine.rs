use crate::{
    account_id::AccountId,
    public_key::PublicKey,
    types::{Address, RawH256, RawU256, WeiU256, Yocto},
    Vec,
};
use borsh::{io, BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// Parameters for the `new` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum NewCallArgs {
    V1(LegacyNewCallArgs),
    V2(NewCallArgsV2),
    V3(NewCallArgsV3),
    V4(NewCallArgsV4),
}

impl NewCallArgs {
    /// Creates a `NewCallArs` from the provided bytes which could be represented
    /// in JSON or Borsh format. Supporting arguments in JSON format starting from V4.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, io::Error> {
        Self::try_from_json(bytes).or_else(|_| {
            Self::try_from_slice(bytes).map_or_else(
                |_| LegacyNewCallArgs::try_from_slice(bytes).map(Self::V1),
                Ok,
            )
        })
    }

    /// Returns a genesis hash of the Hashchain if present.
    #[must_use]
    pub const fn initial_hashchain(&self) -> Option<RawH256> {
        match self {
            Self::V4(args) => args.initial_hashchain,
            Self::V1(_) | Self::V2(_) | Self::V3(_) => None,
        }
    }

    fn try_from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice::<NewCallJsonArgs>(bytes).map(Into::into)
    }
}

impl From<NewCallJsonArgs> for NewCallArgs {
    fn from(value: NewCallJsonArgs) -> Self {
        match value {
            NewCallJsonArgs::V1(args) => Self::V4(args),
        }
    }
}

/// JSON encoded new parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NewCallJsonArgs {
    V1(NewCallArgsV4),
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

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct NewCallArgsV3 {
    /// Chain id, according to the EIP-115 / ethereum-lists spec.
    pub chain_id: RawU256,
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
    /// Relayer keys manager.
    pub key_manager: AccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct NewCallArgsV4 {
    /// Chain id, according to the EIP-115 / ethereum-lists spec.
    #[serde(with = "chain_id_deserialize")]
    pub chain_id: RawU256,
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
    /// Relayer keys manager.
    pub key_manager: AccountId,
    /// Initial value of the hashchain.
    /// If none is provided then the hashchain will start disabled.
    pub initial_hashchain: Option<RawH256>,
}

/// Borsh-encoded parameters for the `set_owner` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(Serialize, Deserialize))]
pub struct SetOwnerArgs {
    pub new_owner: AccountId,
}

/// Borsh-encoded parameters for the `set_upgrade_delay_blocks` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(Serialize, Deserialize))]
pub struct SetUpgradeDelayBlocksArgs {
    pub upgrade_delay_blocks: u64,
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

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(Serialize, Deserialize))]
pub struct StartHashchainArgs {
    pub block_height: u64,
    pub block_hashchain: RawH256,
}

/// Fungible token storage balance
#[derive(Default, Debug, Serialize, Deserialize)]
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

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PausePrecompilesCallArgs {
    pub paused_mask: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(Serialize, Deserialize))]
pub struct ResultLog {
    pub address: Address,
    pub topics: Vec<RawU256>,
    pub data: Vec<u8>,
}

/// The status of a transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(feature = "impl-serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "impl-serde", derive(Serialize, Deserialize))]
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
    pub const fn new(status: TransactionStatus, gas_used: u64, logs: Vec<ResultLog>) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct StorageUnregisterArgs {
    pub force: bool,
}

pub fn parse_json_args<'de, T: Deserialize<'de>>(
    bytes: &'de [u8],
) -> Result<T, errors::ParseArgsError> {
    serde_json::from_slice(bytes).map_err(Into::into)
}

/// Parameters for setting relayer keys manager.
#[derive(Debug, Clone, Eq, PartialEq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct RelayerKeyManagerArgs {
    pub key_manager: Option<AccountId>,
}

/// Parameters for adding or removing relayer function all keys.
#[derive(Debug, Clone, Eq, PartialEq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct RelayerKeyArgs {
    pub public_key: PublicKey,
}

pub type FullAccessKeyArgs = RelayerKeyArgs;

/// Parameters for upgrading the contract.
#[derive(Debug, Clone, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct UpgradeParams {
    /// Code for upgrading.
    pub code: Vec<u8>,
    /// Amount of gas for the state migration.
    pub state_migration_gas: Option<u64>,
}

mod chain_id_deserialize {
    use crate::types::{u256_to_arr, RawU256};
    use primitive_types::U256;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<RawU256, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(|v| u256_to_arr(&(v.into())))
    }

    pub fn serialize<S>(value: &RawU256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let chain_id = U256::from_big_endian(value.as_slice()).low_u64();
        serializer.serialize_u64(chain_id)
    }
}

pub mod errors {
    use crate::{account_id::ParseAccountError, String, ToString};

    pub const ERR_REVERT: &[u8; 10] = b"ERR_REVERT";
    pub const ERR_NOT_ALLOWED: &[u8; 15] = b"ERR_NOT_ALLOWED";
    pub const ERR_OUT_OF_FUNDS: &[u8; 16] = b"ERR_OUT_OF_FUNDS";
    pub const ERR_CALL_TOO_DEEP: &[u8; 17] = b"ERR_CALL_TOO_DEEP";
    pub const ERR_OUT_OF_OFFSET: &[u8; 17] = b"ERR_OUT_OF_OFFSET";
    pub const ERR_OUT_OF_GAS: &[u8; 14] = b"ERR_OUT_OF_GAS";

    #[derive(Debug)]
    pub enum ParseArgsError {
        Json(String),
        InvalidAccount(ParseAccountError),
    }

    impl From<serde_json::Error> for ParseArgsError {
        fn from(e: serde_json::Error) -> Self {
            Self::Json(e.to_string())
        }
    }

    impl From<ParseAccountError> for ParseArgsError {
        fn from(e: ParseAccountError) -> Self {
            Self::InvalidAccount(e)
        }
    }

    impl AsRef<[u8]> for ParseArgsError {
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
        let bytes = borsh::to_vec(&x).unwrap();
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
        let args = CallArgs::V2(new_input.clone());
        let input_bytes = borsh::to_vec(&args).unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, Some(args));

        // Using new input format (wrapped into call args enum) and old data structure with legacy arguments,
        // this is allowed for compatibility reason.
        let args = CallArgs::V1(legacy_input.clone());
        let input_bytes = borsh::to_vec(&args).unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, Some(args));

        // Parsing bytes in an old input format - raw data structure (not wrapped into call args enum) with legacy arguments,
        // made for backward compatibility.

        // Using old input format (not wrapped into call args enum) - raw data structure with legacy arguments.
        let input_bytes = borsh::to_vec(&legacy_input).unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, Some(CallArgs::V1(legacy_input)));

        // Using old input format (not wrapped into call args enum) - raw data structure with new argument (`value` field).
        // Data structures with new arguments allowed only in new input format for future extensibility reason.
        // Raw data structure (old input format) allowed only with legacy arguments for backward compatibility reason.
        // Unrecognized input should be handled and result as an exception in a call site.
        let input_bytes = borsh::to_vec(&new_input).unwrap();
        let parsed_data = CallArgs::deserialize(&input_bytes);
        assert_eq!(parsed_data, None);
    }

    #[test]
    fn test_deserialize_relayer_key_args() {
        let json = r#"{"public_key": "ed25519:DcA2MzgpJbrUATQLLceocVckhhAqrkingax4oJ9kZ847"}"#;
        let public_key: PublicKey = "ed25519:DcA2MzgpJbrUATQLLceocVckhhAqrkingax4oJ9kZ847"
            .parse()
            .unwrap();
        let args = serde_json::from_str::<RelayerKeyArgs>(json).unwrap();

        assert_eq!(args.public_key, public_key);
    }

    #[test]
    fn test_deserialize_new_call_args_json() {
        let chain_id = 1_313_161_559;
        let json = serde_json::json!({
            "chain_id": chain_id,
            "owner_id": "aurora",
            "upgrade_delay_blocks": 10,
            "key_manager": "manager.near",
            "initial_hashchain": null
        });
        let arguments = NewCallArgs::deserialize(&serde_json::to_vec(&json).unwrap());
        let Ok(NewCallArgs::V4(arguments)) = arguments else {
            panic!("Wrong type of arguments");
        };
        let value = serde_json::to_value(arguments).unwrap();
        assert_eq!(value.get("chain_id").unwrap().as_u64(), Some(chain_id));

        let outdated = serde_json::json!({
            "chain_id": chain_id,
            "owner_id": "aurora",
            "upgrade_delay_blocks": 19
        });
        let arguments = NewCallArgs::deserialize(&serde_json::to_vec(&outdated).unwrap());
        assert!(arguments.is_err());
    }
}
