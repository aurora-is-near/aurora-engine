use borsh::{BorshDeserialize, BorshSerialize};
use primitive_types::U256;
use serde::{Deserialize, Serialize};

use crate::account_id::AccountId;
use crate::types::{Address, Balance, Fee, NEP141Wei, RawU256, Yocto};
use crate::{String, ToString, Vec};

/// Eth-connector initial args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct InitCallArgs {
    pub prover_account: AccountId,
    pub eth_custodian_address: String,
    pub metadata: FungibleTokenMetadata,
}

/// Eth-connector Set contract data call args
pub type SetContractDataCallArgs = InitCallArgs;

/// Withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct WithdrawCallArgs {
    pub recipient_address: Address,
    pub amount: NEP141Wei,
}

/// On-transfer message. Used in the `ft_transfer_call` and  `ft_on_transfer` transactions.
/// A message is parsed with the `parse_on_transfer_message` method.
#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq, Eq))]
pub struct FtTransferMessageData {
    pub recipient: Address,
    #[deprecated]
    pub fee: Option<FtTransferFee>,
}

impl TryFrom<&str> for FtTransferMessageData {
    type Error = errors::ParseOnTransferMessageError;

    fn try_from(message: &str) -> Result<Self, Self::Error> {
        if message.len() == 40 {
            // Parse message to determine recipient
            let recipient = {
                // Message format:
                // Recipient of the transaction - 40 characters (Address in hex)
                let mut address_bytes = [0; 20];
                hex::decode_to_slice(message, &mut address_bytes)
                    .map_err(|_| errors::ParseOnTransferMessageError::InvalidHexData)?;
                Address::from_array(address_bytes)
            };

            #[allow(deprecated)]
            return Ok(Self {
                recipient,
                fee: None,
            });
        }

        // This logic is for backward compatibility to parse the message of the deprecated format.
        // "{relayer_id}:0000000000000000000000000000000000000000000000000000000000000000{hex_address}"

        // Split message by separator
        let (account, msg) = message
            .split_once(':')
            .ok_or(errors::ParseOnTransferMessageError::TooManyParts)?;

        // Check relayer account id from 1-th data element
        let account_id = account
            .parse()
            .map_err(|_| errors::ParseOnTransferMessageError::InvalidAccount)?;

        // Decode message array from 2-th element of data array
        // Length = fee[32] + eth_address[20] bytes
        let mut data = [0; 52];
        hex::decode_to_slice(msg, &mut data).map_err(|e| match e {
            hex::FromHexError::InvalidHexCharacter { .. } | hex::FromHexError::OddLength => {
                errors::ParseOnTransferMessageError::InvalidHexData
            }
            hex::FromHexError::InvalidStringLength => {
                errors::ParseOnTransferMessageError::WrongMessageFormat
            }
        })?;

        // Parse the fee from the message slice.
        // The fee is expected to be represented as a 32-byte value in the message.
        // However, it will be parsed and converted to u128 for further processing.
        // This parsing logic is implemented to ensure compatibility
        let fee_u128: u128 = U256::from_little_endian(&data[..32])
            .try_into()
            .map_err(|_| errors::ParseOnTransferMessageError::OverflowNumber)?;
        let fee_amount: Fee = fee_u128.into();

        // Get recipient Eth address from message slice
        let recipient = Address::try_from_slice(&data[32..]).unwrap();

        #[allow(deprecated)]
        Ok(Self {
            recipient,
            fee: Some(FtTransferFee {
                relayer: account_id,
                amount: fee_amount,
            }),
        })
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq, Eq))]
pub struct FtTransferFee {
    pub relayer: AccountId,
    pub amount: Fee,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct RefundCallArgs {
    pub recipient_address: Address,
    pub erc20_address: Option<Address>,
    pub amount: RawU256,
}

/// Arguments for `ft_transfer` transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct TransferNearArgs {
    pub target_account_id: AccountId,
    pub amount: u128,
}

/// Arguments for callback used in the `exit_to_near` precompile.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub struct ExitToNearPrecompileCallbackArgs {
    pub refund: Option<RefundCallArgs>,
    pub transfer_near: Option<TransferNearArgs>,
}

/// Arguments for the `ft_transfer_call` transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Deserialize, Serialize, PartialEq, Eq)]
pub struct FtTransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: NEP141Wei,
    pub memo: Option<String>,
    pub msg: String,
}

/// Arguments for the `storage_deposit` transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Deserialize, Serialize, PartialEq, Eq)]
pub struct StorageDepositArgs {
    pub account_id: Option<AccountId>,
    pub registration_only: Option<bool>,
}

/// Arguments for the `storage_withdraw` transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Deserialize, Serialize, PartialEq, Eq)]
pub struct StorageWithdrawArgs {
    pub amount: Option<Yocto>,
}

/// Arguments for the `ft_transfer` transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Deserialize, Serialize, PartialEq, Eq)]
pub struct FtTransferArgs {
    pub receiver_id: AccountId,
    pub amount: NEP141Wei,
    pub memo: Option<String>,
}

/// A result produced by the `withdraw` transaction.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct WithdrawResult {
    pub amount: NEP141Wei,
    pub recipient_id: Address,
    pub eth_custodian_address: Address,
}

/// Arguments for the `ft_resolve_transfer` transaction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct FtResolveTransferArgs {
    pub sender_id: AccountId,
    pub amount: NEP141Wei,
    pub receiver_id: AccountId,
}

/// Finish deposit NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct FinishDepositArgs {
    pub new_owner_id: AccountId,
    pub amount: NEP141Wei,
    pub proof_key: String,
    pub relayer_id: AccountId,
    pub fee: Fee,
    pub msg: Option<Vec<u8>>,
}

/// Parameters for the `ft_on_transfer` transaction for regular NEP-141 tokens.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Deserialize, Serialize, PartialEq, Eq)]
pub struct FtOnTransferArgs {
    pub sender_id: AccountId,
    pub amount: Balance,
    pub msg: String,
}

#[derive(BorshSerialize)]
pub struct EngineWithdrawCallArgs {
    pub sender_id: AccountId,
    pub recipient_address: Address,
    pub amount: NEP141Wei,
}

/// `storage_unregister` eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageUnregisterArgs {
    pub force: Option<bool>,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct SetEthConnectorContractAccountArgs {
    pub account: AccountId,
    pub withdraw_serialize_type: WithdrawSerializeType,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum WithdrawSerializeType {
    Json,
    Borsh,
}

pub type PausedMask = u8;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PauseEthConnectorArgs {
    pub paused_mask: PausedMask,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FungibleTokenMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<FungibleReferenceHash>,
    pub decimals: u8,
}

impl Default for FungibleTokenMetadata {
    fn default() -> Self {
        Self {
            spec: "ft-1.0.0".to_string(),
            name: "Ether".to_string(),
            symbol: "ETH".to_string(),
            icon: Some("data:image/png;base64,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".to_string()),
            reference: None,
            reference_hash: None,
            decimals: 18,
        }
    }
}

/// Fungible token Reference hash type. Used for `FungibleTokenMetadata`
#[derive(Debug, BorshDeserialize, BorshSerialize, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct FungibleReferenceHash([u8; 32]);

impl AsRef<[u8]> for FungibleReferenceHash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Borsh-encoded parameters for `mirror_erc20_token` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq, Clone)]
pub struct MirrorErc20TokenArgs {
    /// `AccountId` of the main Aurora contract which has previously deployed ERC-20.
    pub contract_id: AccountId,
    /// `AccountId` of the bridged NEP-141 token.
    pub nep141: AccountId,
}

/// Parameters for `set_erc20_metadata` function.
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SetErc20MetadataArgs {
    /// Address or corresponding NEP-141 account id of the ERC-20 contract.
    pub erc20_identifier: Erc20Identifier,
    /// Metadata of the ERC-20 contract.
    pub metadata: Erc20Metadata,
}

#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Erc20Identifier {
    Erc20 { address: Address },
    Nep141 { account_id: AccountId },
}

impl From<Address> for Erc20Identifier {
    fn from(address: Address) -> Self {
        Self::Erc20 { address }
    }
}

impl From<AccountId> for Erc20Identifier {
    fn from(account_id: AccountId) -> Self {
        Self::Nep141 { account_id }
    }
}

/// Metadata of ERC-20 contract.
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Erc20Metadata {
    /// Name of the token.
    pub name: String,
    /// Symbol of the token.
    pub symbol: String,
    /// Number of decimals.
    pub decimals: u8,
}

impl Default for Erc20Metadata {
    fn default() -> Self {
        Self {
            name: "Empty".to_string(),
            symbol: "EMPTY".to_string(),
            decimals: 0,
        }
    }
}

pub mod errors {
    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
    pub enum ParseOnTransferMessageError {
        TooManyParts,
        InvalidHexData,
        WrongMessageFormat,
        InvalidAccount,
        OverflowNumber,
    }

    impl AsRef<[u8]> for ParseOnTransferMessageError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::TooManyParts => b"ERR_INVALID_ON_TRANSFER_MESSAGE_FORMAT",
                Self::InvalidHexData => b"ERR_INVALID_ON_TRANSFER_MESSAGE_HEX",
                Self::WrongMessageFormat => b"ERR_INVALID_ON_TRANSFER_MESSAGE_DATA",
                Self::InvalidAccount => b"ERR_INVALID_ACCOUNT_ID",
                Self::OverflowNumber => b"ERR_OVERFLOW_NUMBER",
            }
        }
    }
}
