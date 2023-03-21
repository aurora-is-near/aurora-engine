use crate::admin_controlled::PausedMask;
use crate::fungible_token::FungibleTokenMetadata;
use crate::prelude::account_id::AccountId;
use crate::prelude::{Address, Balance, BorshDeserialize, BorshSerialize, RawU256, String, Vec};
use crate::proof::Proof;
pub use aurora_engine_types::parameters::engine::{
    CallArgs, DeployErc20TokenArgs, FunctionCallArgsV1, FunctionCallArgsV2,
    GetErc20FromNep141CallArgs, GetStorageAtArgs, ResultLog, SubmitResult, TransactionStatus,
    ViewCallArgs,
};
use aurora_engine_types::types::{Fee, NEP141Wei, Yocto};
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

/// Borsh-encoded parameters for the `set_owner` function.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct SetOwnerArgs {
    pub new_owner: AccountId,
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

/// Borsh-encoded parameters for the `ft_transfer_call` function
/// for regular NEP-141 tokens.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct NEP141FtOnTransferArgs {
    pub sender_id: AccountId,
    /// Balance can be for Eth on Near and for Eth to Aurora
    /// `ft_on_transfer` can be called with arbitrary NEP-141 tokens attached, therefore we do not specify a particular type Wei.
    pub amount: Balance,
    pub msg: String,
}

/// Eth-connector deposit arguments
#[derive(BorshSerialize, BorshDeserialize)]
pub struct DepositCallArgs {
    /// Proof data
    pub proof: Proof,
    /// Optional relayer address
    pub relayer_eth_account: Option<Address>,
}

/// Eth-connector `isUsedProof` arguments
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
#[derive(Default, Deserialize, Serialize)]
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

/// `ft_resolve_transfer` eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct ResolveTransferCallArgs {
    pub sender_id: AccountId,
    pub amount: NEP141Wei,
    pub receiver_id: AccountId,
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
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferCallCallArgs {
    pub receiver_id: AccountId,
    pub amount: NEP141Wei,
    pub memo: Option<String>,
    pub msg: String,
}

/// `storage_balance_of` eth-connector call args
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct StorageBalanceOfCallArgs {
    pub account_id: AccountId,
}

/// `storage_deposit` eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageDepositCallArgs {
    pub account_id: Option<AccountId>,
    pub registration_only: Option<bool>,
}

/// `storage_withdraw` eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageWithdrawCallArgs {
    pub amount: Option<Yocto>,
}

/// transfer args for json invocation
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: NEP141Wei,
    pub memo: Option<String>,
}

/// `balance_of` args for json invocation
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct BalanceOfCallArgs {
    pub account_id: AccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct BalanceOfEthCallArgs {
    pub address: Address,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RegisterRelayerCallArgs {
    pub address: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct PauseEthConnectorCallArgs {
    pub paused_mask: PausedMask,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PausePrecompilesCallArgs {
    pub paused_mask: u32,
}

pub mod error {
    use aurora_engine_types::{account_id::ParseAccountError, String, ToString};

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
