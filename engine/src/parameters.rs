use crate::admin_controlled::PausedMask;
use crate::fungible_token::FungibleTokenMetadata;
use crate::json::{JsonError, JsonValue, ParseError};
use crate::prelude::{
    format, is_valid_account_id, AccountId, Balance, BorshDeserialize, BorshSerialize, EthAddress,
    RawAddress, RawH256, RawU256, SdkUnwrap, String, ToString, TryFrom, Vec,
};
use crate::proof::Proof;
use evm::backend::Log;

/// Borsh-encoded parameters for the `new` function.
#[derive(BorshSerialize, BorshDeserialize)]
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

/// Borsh-encoded parameters for the `meta_call` function.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct MetaCallArgs {
    pub signature: [u8; 64],
    pub v: u8,
    pub nonce: RawU256,
    pub fee_amount: RawU256,
    pub fee_address: RawAddress,
    pub contract_address: RawAddress,
    pub value: RawU256,
    pub method_def: String,
    pub args: Vec<u8>,
}

/// Borsh-encoded log for use in a `SubmitResult`.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ResultLog {
    pub topics: Vec<RawU256>,
    pub data: Vec<u8>,
}

impl From<Log> for ResultLog {
    fn from(log: Log) -> Self {
        let topics = log
            .topics
            .into_iter()
            .map(|topic| topic.0)
            .collect::<Vec<_>>();
        ResultLog {
            topics,
            data: log.data,
        }
    }
}

/// The status of a transaction.
#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
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
            Self::Revert(_) => b"ERR_REVERT",
            Self::OutOfFund => b"ERR_OUT_OF_FUNDS",
            Self::OutOfGas => b"ERR_OUT_OF_GAS",
            Self::OutOfOffset => b"ERR_OUT_OF_OFFSET",
            Self::CallTooDeep => b"ERR_CALL_TOO_DEEP",
        }
    }
}

/// Borsh-encoded parameters for the `call`, `call_with_args`, `deploy_code`,
/// and `deploy_with_input` methods.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct SubmitResult {
    pub status: TransactionStatus,
    pub gas_used: u64,
    pub logs: Vec<ResultLog>,
}

/// Borsh-encoded parameters for the `call` function.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FunctionCallArgs {
    pub contract: RawAddress,
    pub input: Vec<u8>,
}

/// Borsh-encoded parameters for the `view` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq)]
pub struct ViewCallArgs {
    pub sender: RawAddress,
    pub address: RawAddress,
    pub amount: RawU256,
    pub input: Vec<u8>,
}

/// Borsh-encoded parameters for `deploy_erc20_token` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq)]
pub struct DeployErc20TokenArgs {
    pub nep141: AccountId,
}

/// Borsh-encoded parameters for `get_erc20_from_nep141` function.
pub type GetErc20FromNep141CallArgs = DeployErc20TokenArgs;

/// Borsh-encoded parameters for the `get_storage_at` function.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct GetStorageAtArgs {
    pub address: RawAddress,
    pub key: RawH256,
}

/// Borsh-encoded (genesis) account balance used by the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountBalance {
    pub address: RawAddress,
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
    pub coinbase: RawAddress,
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
pub struct NEP141FtOnTransferArgs {
    pub sender_id: AccountId,
    pub amount: Balance,
    pub msg: String,
}

impl TryFrom<JsonValue> for NEP141FtOnTransferArgs {
    type Error = JsonError;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_id: value.string("sender_id")?,
            amount: value.u128("amount")?,
            msg: value.string("msg")?,
        })
    }
}

impl TryFrom<NEP141FtOnTransferArgs> for String {
    type Error = ParseError;

    fn try_from(value: NEP141FtOnTransferArgs) -> Result<Self, Self::Error> {
        if !is_valid_account_id(value.sender_id.as_bytes()) {
            return Err(ParseError::InvalidAccountId);
        }

        Ok(format!(
            r#"{{"sender_id": "{}", "amount": "{}", "msg": "{}"}}"#,
            value.sender_id,
            value.amount,
            // Escape message to avoid json injection attacks
            value.msg.replace("\\", "\\\\").replace("\"", "\\\"")
        ))
    }
}

/// Eth-connector deposit arguments
#[derive(BorshSerialize, BorshDeserialize)]
pub struct DepositCallArgs {
    /// Proof data
    pub proof: Proof,
    /// Optional relayer address
    pub relayer_eth_account: Option<EthAddress>,
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
    pub amount: Balance,
    pub recipient_id: RawAddress,
    pub eth_custodian_address: RawAddress,
}

/// ft_resolve_transfer eth-connector call args
#[derive(BorshSerialize)]
pub struct FtResolveTransfer {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub current_account_id: AccountId,
}

/// Fungible token storage balance
#[derive(Default)]
pub struct StorageBalance {
    pub total: Balance,
    pub available: Balance,
}

impl StorageBalance {
    pub fn to_json_bytes(&self) -> Vec<u8> {
        format!(
            "{{\"total\": \"{}\", \"available\": \"{}\"}}",
            self.total.to_string(),
            self.available.to_string()
        )
        .as_bytes()
        .to_vec()
    }
}

/// resolve_transfer eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct ResolveTransferCallArgs {
    pub sender_id: AccountId,
    pub amount: Balance,
    pub receiver_id: AccountId,
}

/// Finish deposit NEAR eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinishDepositCallArgs {
    pub new_owner_id: AccountId,
    pub amount: Balance,
    pub proof_key: String,
    pub relayer_id: AccountId,
    pub fee: Balance,
    pub msg: Option<Vec<u8>>,
}

/// Deposit ETH args
#[derive(Default, BorshDeserialize, BorshSerialize, Clone)]
pub struct DepositEthCallArgs {
    pub proof: Proof,
    pub relayer_eth_account: EthAddress,
}

/// Finish deposit NEAR eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinishDepositEthCallArgs {
    pub new_owner_id: EthAddress,
    pub amount: Balance,
    pub fee: Balance,
    pub relayer_eth_account: AccountId,
    pub proof: Proof,
}

/// Eth-connector initial args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitCallArgs {
    pub prover_account: AccountId,
    pub eth_custodian_address: AccountId,
    pub metadata: FungibleTokenMetadata,
}

/// Eth-connector Set contract data call args
pub type SetContractDataCallArgs = InitCallArgs;

/// transfer eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransferCallCallArgs {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub memo: Option<String>,
    pub msg: String,
}

impl From<JsonValue> for TransferCallCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            receiver_id: v.string("receiver_id").sdk_unwrap(),
            amount: v.u128("amount").sdk_unwrap(),
            memo: v.string("memo").ok(),
            msg: v.string("msg").sdk_unwrap(),
        }
    }
}

/// storage_balance_of eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StorageBalanceOfCallArgs {
    pub account_id: AccountId,
}

impl From<JsonValue> for StorageBalanceOfCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            account_id: v.string("account_id").sdk_unwrap(),
        }
    }
}

/// storage_deposit eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StorageDepositCallArgs {
    pub account_id: Option<AccountId>,
    pub registration_only: Option<bool>,
}

impl From<JsonValue> for StorageDepositCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            account_id: v.string("account_id").ok(),
            registration_only: v.bool("registration_only").ok(),
        }
    }
}

/// storage_withdraw eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StorageWithdrawCallArgs {
    pub amount: Option<u128>,
}

impl From<JsonValue> for StorageWithdrawCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            amount: v.u128("amount").ok(),
        }
    }
}

/// transfer args for json invocation
#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub memo: Option<String>,
}

impl From<JsonValue> for TransferCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            receiver_id: v.string("receiver_id").sdk_unwrap(),
            amount: v.u128("amount").sdk_unwrap(),
            memo: v.string("memo").ok(),
        }
    }
}

/// balance_of args for json invocation
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BalanceOfCallArgs {
    pub account_id: AccountId,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BalanceOfEthCallArgs {
    pub address: EthAddress,
}

impl From<JsonValue> for BalanceOfCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            account_id: v.string("account_id").sdk_unwrap(),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RegisterRelayerCallArgs {
    pub address: EthAddress,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PauseEthConnectorCallArgs {
    pub paused_mask: PausedMask,
}

impl From<JsonValue> for ResolveTransferCallArgs {
    fn from(v: JsonValue) -> Self {
        Self {
            sender_id: v.string("sender_id").sdk_unwrap(),
            receiver_id: v.string("receiver_id").sdk_unwrap(),
            amount: v.u128("amount").sdk_unwrap(),
        }
    }
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
            sender: [1; 20],
            address: [2; 20],
            amount: [3; 32],
            input: vec![1, 2, 3],
        };
        let bytes = x.try_to_vec().unwrap();
        let res = ViewCallArgs::try_from_slice(&bytes).unwrap();
        assert_eq!(x, res);
    }
}
