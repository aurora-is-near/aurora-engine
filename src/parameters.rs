use borsh::{BorshDeserialize, BorshSerialize};

use crate::prelude::{String, Vec};
use crate::types::{AccountId, Balance, RawAddress, RawH256, RawU256};
use crate::{
    admin_controlled::PausedMask,
    json,
    prelude::{is_valid_account_id, ToString, TryFrom},
    sdk,
    types::{EthAddress, Proof, SdkUnwrap},
};
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

/// Borsh-encoded parameters for the `call`, `call_with_args`, `deploy_code`,
/// and `deploy_with_input` methods.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct SubmitResult {
    pub status: bool,
    pub gas_used: u64,
    pub result: Vec<u8>,
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

#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq)]
pub struct DeployErc20TokenArgs {
    pub nep141: AccountId,
}

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

impl TryFrom<json::JsonValue> for NEP141FtOnTransferArgs {
    type Error = json::JsonError;

    fn try_from(value: json::JsonValue) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_id: value.string("sender_id")?,
            amount: value.u128("amount")?,
            msg: value.string("msg")?,
        })
    }
}

impl TryFrom<NEP141FtOnTransferArgs> for String {
    type Error = json::ParseError;

    fn try_from(value: NEP141FtOnTransferArgs) -> Result<Self, Self::Error> {
        if !is_valid_account_id(value.sender_id.as_bytes()) {
            return Err(json::ParseError::InvalidAccountId);
        }

        Ok(crate::prelude::format!(
            r#"{{"sender_id": "{}", "amount": "{}", "msg": "{}"}}"#,
            value.sender_id,
            value.amount,
            // Escape message to avoid json injection attacks
            value.msg.replace("\\", "\\\\").replace("\"", "\\\"")
        ))
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PromiseCreateArgs {
    pub target_account_id: AccountId,
    pub method: String,
    pub args: Vec<u8>,
    pub attached_balance: u128,
    pub attached_gas: u64,
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
        crate::prelude::format!(
            "{{\"total\": \"{}\", \"available\": \"{}\",}}",
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

impl From<json::JsonValue> for TransferCallCallArgs {
    fn from(v: json::JsonValue) -> Self {
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

impl From<json::JsonValue> for StorageBalanceOfCallArgs {
    fn from(v: json::JsonValue) -> Self {
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

impl From<json::JsonValue> for StorageDepositCallArgs {
    fn from(v: json::JsonValue) -> Self {
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

impl From<json::JsonValue> for StorageWithdrawCallArgs {
    fn from(v: json::JsonValue) -> Self {
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

impl From<json::JsonValue> for TransferCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            receiver_id: v.string("receiver_id").sdk_unwrap(),
            amount: v.u128("amount").sdk_unwrap(),
            memo: v.string("memo").ok(),
        }
    }
}

/// withdraw NEAR eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct WithdrawCallArgs {
    pub recipient_address: EthAddress,
    pub amount: Balance,
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

impl From<json::JsonValue> for BalanceOfCallArgs {
    fn from(v: json::JsonValue) -> Self {
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

pub trait ExpectUtf8<T> {
    fn expect_utf8(self, message: &[u8]) -> T;
}

impl<T> ExpectUtf8<T> for Option<T> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Some(t) => t,
            None => sdk::panic_utf8(message),
        }
    }
}

impl<T, E> ExpectUtf8<T> for core::result::Result<T, E> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Ok(t) => t,
            Err(_) => sdk::panic_utf8(message),
        }
    }
}

impl From<json::JsonValue> for ResolveTransferCallArgs {
    fn from(v: json::JsonValue) -> Self {
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
