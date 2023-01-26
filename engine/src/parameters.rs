use crate::admin_controlled::PausedMask;
use crate::fungible_token::FungibleTokenMetadata;
use crate::json::{JsonError, JsonValue};
use crate::prelude::account_id::AccountId;
use crate::prelude::{
    format, Address, Balance, BorshDeserialize, BorshSerialize, RawU256, String, Vec,
};
use crate::proof::Proof;
pub use aurora_engine_types::parameters::engine::{
    CallArgs, DeployErc20TokenArgs, FunctionCallArgsV1, FunctionCallArgsV2,
    GetErc20FromNep141CallArgs, GetStorageAtArgs, ResultLog, SubmitResult, TransactionStatus,
    ViewCallArgs,
};
use aurora_engine_types::types::{Fee, NEP141Wei, Yocto};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Borsh-encoded parameters for the `new` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct NewCallArgs {
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

/// Borsh-encoded (genesis) account balance used by the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountBalance {
    pub address: Address,
    pub balance: RawU256,
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

/// Borsh-encoded parameters for the `ft_transfer_call` function
/// for regular NEP-141 tokens.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct NEP141FtOnTransferArgs {
    pub sender_id: AccountId,
    /// Balance can be for Eth on Near and for Eth to Aurora
    /// `ft_on_transfer` can be called with arbitrary NEP-141 tokens attached, therefore we do not specify a particular type Wei.
    pub amount: Balance,
    pub msg: String,
}

impl TryFrom<JsonValue> for NEP141FtOnTransferArgs {
    type Error = JsonError;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_id: AccountId::try_from(value.string("sender_id")?)
                .map_err(|_| JsonError::InvalidString)?,
            amount: Balance::new(value.u128("amount")?),
            msg: value.string("msg")?,
        })
    }
}

impl From<NEP141FtOnTransferArgs> for String {
    fn from(value: NEP141FtOnTransferArgs) -> Self {
        format!(
            r#"{{"sender_id": "{}", "amount": "{}", "msg": "{}"}}"#,
            value.sender_id,
            value.amount,
            // Escape message to avoid json injection attacks
            value.msg.replace('\\', "\\\\").replace('"', "\\\"")
        )
    }
}

/// Eth-connector deposit arguments
#[derive(BorshSerialize, BorshDeserialize)]
pub struct DepositCallArgs {
    /// Proof data
    pub proof: Proof,
    /// Optional relayer address
    pub relayer_eth_account: Option<Address>,
}

/// Eth-connector isUsedProof arguments
#[derive(BorshSerialize, BorshDeserialize)]
pub struct IsUsedProofCallArgs {
    /// Proof data
    pub proof: Proof,
}

/// withdraw result for eth-connector
#[derive(BorshSerialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(BorshDeserialize))]
pub struct WithdrawResult {
    pub amount: NEP141Wei,
    pub recipient_id: Address,
    pub eth_custodian_address: Address,
}

/// Fungible token storage balance
#[derive(Default)]
pub struct StorageBalance {
    pub total: Yocto,
    pub available: Yocto,
}

impl StorageBalance {
    pub fn to_json_bytes(&self) -> Vec<u8> {
        format!(
            "{{\"total\": \"{}\", \"available\": \"{}\"}}",
            self.total, self.available
        )
        .into_bytes()
    }
}

/// ft_resolve_transfer eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct ResolveTransferCallArgs {
    pub sender_id: AccountId,
    pub amount: NEP141Wei,
    pub receiver_id: AccountId,
}

impl TryFrom<JsonValue> for ResolveTransferCallArgs {
    type Error = error::ParseTypeFromJsonError;

    fn try_from(v: JsonValue) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_id: AccountId::try_from(v.string("sender_id")?)?,
            receiver_id: AccountId::try_from(v.string("receiver_id")?)?,
            amount: NEP141Wei::new(v.u128("amount")?),
        })
    }
}

/// Finish deposit NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct FinishDepositCallArgs {
    pub new_owner_id: AccountId,
    pub amount: NEP141Wei,
    pub proof_key: String,
    pub relayer_id: AccountId,
    pub fee: Fee,
    pub msg: Option<Vec<u8>>,
}

/// Deposit ETH args
#[derive(Default, BorshDeserialize, BorshSerialize, Clone)]
pub struct DepositEthCallArgs {
    pub proof: Proof,
    pub relayer_eth_account: Address,
}

/// Finish deposit NEAR eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinishDepositEthCallArgs {
    pub new_owner_id: Address,
    pub amount: NEP141Wei,
    pub fee: Balance,
    pub relayer_eth_account: AccountId,
    pub proof: Proof,
}

/// Eth-connector initial args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct InitCallArgs {
    pub prover_account: AccountId,
    pub eth_custodian_address: String,
    pub metadata: FungibleTokenMetadata,
}

/// Eth-connector Set contract data call args
pub type SetContractDataCallArgs = InitCallArgs;

/// transfer eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct TransferCallCallArgs {
    pub receiver_id: AccountId,
    pub amount: NEP141Wei,
    pub memo: Option<String>,
    pub msg: String,
}

impl TryFrom<JsonValue> for TransferCallCallArgs {
    type Error = error::ParseTypeFromJsonError;

    fn try_from(v: JsonValue) -> Result<Self, Self::Error> {
        let receiver_id = AccountId::try_from(v.string("receiver_id")?)?;
        let amount = NEP141Wei::new(v.u128("amount")?);
        let memo = v.string("memo").ok();
        let msg = v.string("msg")?;
        Ok(Self {
            receiver_id,
            amount,
            memo,
            msg,
        })
    }
}

/// storage_balance_of eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StorageBalanceOfCallArgs {
    pub account_id: AccountId,
}

impl TryFrom<JsonValue> for StorageBalanceOfCallArgs {
    type Error = error::ParseTypeFromJsonError;

    fn try_from(v: JsonValue) -> Result<Self, Self::Error> {
        let account_id = AccountId::try_from(v.string("account_id")?)?;
        Ok(Self { account_id })
    }
}

/// storage_deposit eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct StorageDepositCallArgs {
    pub account_id: Option<AccountId>,
    pub registration_only: Option<bool>,
}

impl From<JsonValue> for StorageDepositCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            account_id: v
                .string("account_id")
                .map_or(None, |acc| AccountId::try_from(acc).ok()),
            registration_only: v.bool("registration_only").ok(),
        }
    }
}

/// storage_withdraw eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct StorageWithdrawCallArgs {
    pub amount: Option<Yocto>,
}

impl From<JsonValue> for StorageWithdrawCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            amount: v.u128("amount").map(Yocto::new).ok(),
        }
    }
}

/// transfer args for json invocation
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: NEP141Wei,
    pub memo: Option<String>,
}

impl TryFrom<JsonValue> for TransferCallArgs {
    type Error = error::ParseTypeFromJsonError;

    fn try_from(v: JsonValue) -> Result<Self, Self::Error> {
        Ok(Self {
            receiver_id: AccountId::try_from(v.string("receiver_id")?)?,
            amount: NEP141Wei::new(v.u128("amount")?),
            memo: v.string("memo").ok(),
        })
    }
}

/// balance_of args for json invocation
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BalanceOfCallArgs {
    pub account_id: AccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BalanceOfEthCallArgs {
    pub address: Address,
}

impl TryFrom<JsonValue> for BalanceOfCallArgs {
    type Error = error::ParseTypeFromJsonError;

    fn try_from(v: JsonValue) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: AccountId::try_from(v.string("account_id")?)?,
        })
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RegisterRelayerCallArgs {
    pub address: Address,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PauseEthConnectorCallArgs {
    pub paused_mask: PausedMask,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PausePrecompilesCallArgs {
    pub paused_mask: u32,
}

pub mod error {
    use crate::json::JsonError;
    use aurora_engine_types::account_id::ParseAccountError;

    pub enum ParseTypeFromJsonError {
        Json(JsonError),
        InvalidAccount(ParseAccountError),
    }

    impl From<JsonError> for ParseTypeFromJsonError {
        fn from(e: JsonError) -> Self {
            Self::Json(e)
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
                Self::Json(e) => e.as_ref(),
                Self::InvalidAccount(e) => e.as_ref(),
            }
        }
    }
}
