//! Argument and result types for cross-contract calls between the Aurora
//! Engine and the Eth-connector contract.
//!
//! Covers connector initialization, ETH deposits and withdrawals, refunds,
//! NEP-141 / NEP-245 token transfers, storage management, and the
//! `exit_to_near` precompile callback.
use borsh::{BorshDeserialize, BorshSerialize};
use primitive_types::U256;
use serde::{Deserialize, Serialize};

use crate::account_id::AccountId;
use crate::types::{Address, Balance, Fee, NEP141Wei, RawU256, Yocto};
use crate::{Cow, String, ToBytes, ToString, TokenId, Vec};

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
    pub transfer_call: TransferCall,
}

/// Selects exit callback behavior; the default preserves legacy refunds and plain transfers.
#[derive(Debug, Clone, Copy, BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub enum TransferCall {
    /// Plain transfer or wNEAR unwrap; tag 0 retains the legacy callback behavior.
    #[default]
    None,
    /// Settle a single-token `mt_transfer_call` from its one-element used-amount array.
    Nep245,
    /// Settle `ft_transfer_call` from its used amount. Appended to preserve existing tags.
    Nep141,
}

impl ExitToNearPrecompileCallbackArgs {
    /// Decode current arguments or the original two-field layout with legacy refund behavior.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, borsh::io::Error> {
        Self::try_from_slice(bytes).or_else(|_| {
            <(Option<RefundCallArgs>, Option<TransferNearArgs>)>::try_from_slice(bytes).map(
                |(refund, transfer_near)| Self {
                    refund,
                    transfer_near,
                    ..Self::default()
                },
            )
        })
    }
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
/// Parameters for the `mt_on_transfer` transaction for regular NEP-245 tokens.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Deserialize, Serialize, PartialEq, Eq)]
pub struct MtOnTransferArgs {
    pub sender_id: AccountId,
    pub previous_owner_ids: Vec<AccountId>,
    pub token_ids: Vec<TokenId>,
    pub amounts: Vec<Balance>,
    pub msg: String,
}

impl MtOnTransferArgs {
    /// Returns the token ID and amount if the transfer contains exactly one token.
    ///
    /// Batches aren't supported: each token is minted by a separate EVM call, but a NEAR
    /// receipt can only represent a single EVM transaction.
    #[must_use]
    pub fn single_token(&self) -> Option<(&str, Balance)> {
        match (
            self.token_ids.as_slice(),
            self.amounts.as_slice(),
            self.previous_owner_ids.as_slice(),
        ) {
            ([token_id], [amount], [_]) => Some((token_id, *amount)),
            _ => None,
        }
    }
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
    /// NEAR token represented by the ERC-20 contract.
    pub token: BridgedToken,
}

impl MirrorErc20TokenArgs {
    /// Accepts tagged token arguments and the original NEP-141 encoding.
    pub fn deserialize(bytes: &[u8]) -> borsh::io::Result<Self> {
        Self::try_from_slice(bytes)
            .or_else(|_| LegacyMirrorErc20TokenArgs::try_from_slice(bytes).map(Into::into))
    }
}

/// Original Borsh parameters retained for NEP-141 callers and standalone history.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq, Clone)]
pub struct LegacyMirrorErc20TokenArgs {
    pub contract_id: AccountId,
    /// `AccountId` of the bridged NEP-141 token.
    pub nep141: AccountId,
}

impl From<LegacyMirrorErc20TokenArgs> for MirrorErc20TokenArgs {
    fn from(args: LegacyMirrorErc20TokenArgs) -> Self {
        Self {
            contract_id: args.contract_id,
            token: BridgedToken::Nep141(args.nep141.into()),
        }
    }
}

/// Parameters for `set_erc20_metadata` function.
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SetErc20MetadataArgs {
    /// Address or corresponding NEP-141 / NEP-245 token of the ERC-20 contract.
    pub erc20_identifier: Erc20Identifier,
    /// Metadata of the ERC-20 contract.
    pub metadata: Erc20Metadata,
}

#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Erc20Identifier {
    Erc20 {
        address: Address,
    },
    Nep141 {
        account_id: AccountId,
    },
    Nep245 {
        account_id: AccountId,
        token_id: crate::TokenId,
    },
}

impl<'de> Deserialize<'de> for Erc20Identifier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Address,
            AccountId,
            TokenId,
            #[serde(other)]
            Other,
        }

        struct IdentifierVisitor;

        impl<'input> serde::de::Visitor<'input> for IdentifierVisitor {
            type Value = Erc20Identifier;

            fn expecting(&self, formatter: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
                formatter.write_str("an ERC-20, NEP-141 or NEP-245 identifier object")
            }

            fn visit_map<A: serde::de::MapAccess<'input>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut address = None;
                let mut account_id = None;
                let mut token_id = None;

                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Address => {
                            if address.is_some() {
                                return Err(serde::de::Error::duplicate_field("address"));
                            }
                            address = Some(map.next_value::<Address>()?);
                        }
                        Field::AccountId => {
                            if account_id.is_some() {
                                return Err(serde::de::Error::duplicate_field("account_id"));
                            }
                            account_id = Some(map.next_value::<AccountId>()?);
                        }
                        Field::TokenId => {
                            if token_id.is_some() {
                                return Err(serde::de::Error::duplicate_field("token_id"));
                            }
                            token_id = Some(map.next_value::<crate::TokenId>()?);
                        }
                        Field::Other => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                match (address, account_id, token_id) {
                    (Some(address), None, None) => Ok(Erc20Identifier::Erc20 { address }),
                    (None, Some(account_id), None) => Ok(Erc20Identifier::Nep141 { account_id }),
                    (None, Some(account_id), Some(token_id)) => Ok(Erc20Identifier::Nep245 {
                        account_id,
                        token_id,
                    }),
                    (None, None, Some(_)) => Err(serde::de::Error::missing_field("account_id")),
                    (None, None, None) => Err(serde::de::Error::custom(
                        "expected `address` or `account_id`",
                    )),
                    _ => Err(serde::de::Error::custom(
                        "`address` cannot be combined with `account_id` or `token_id`",
                    )),
                }
            }
        }

        deserializer.deserialize_map(IdentifierVisitor)
    }
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

impl From<BridgedToken> for Erc20Identifier {
    fn from(token: BridgedToken) -> Self {
        match token {
            BridgedToken::Nep141(token) => Self::Nep141 {
                account_id: token.0,
            },
            BridgedToken::Nep245(token) => Self::Nep245 {
                account_id: token.account_id,
                token_id: token.token_id,
            },
        }
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

/// JSON metadata returned by `mt_metadata_base_by_token_id`.
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct Nep245Metadata {
    pub name: String,
    pub symbol: Option<String>,
    pub decimals: Option<Nep245Decimals>,
}

/// NEP-245 implementations encode decimals as either a JSON number or a string.
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Nep245Decimals {
    Number(u8),
    String(String),
}

impl TryFrom<Nep245Metadata> for Erc20Metadata {
    type Error = core::num::ParseIntError;

    fn try_from(metadata: Nep245Metadata) -> Result<Self, Self::Error> {
        let defaults = Self::default();
        let decimals = match metadata.decimals {
            Some(Nep245Decimals::Number(value)) => value,
            Some(Nep245Decimals::String(value)) => value.parse()?,
            None => defaults.decimals,
        };

        Ok(Self {
            name: metadata.name,
            symbol: metadata.symbol.unwrap_or(defaults.symbol),
            decimals,
        })
    }
}

/// Identifies a NEAR token bridged to an ERC-20 contract in Aurora.
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq)]
pub enum BridgedToken {
    /// NEP-141 fungible token identified by its contract account ID.
    Nep141(Nep141Token),
    /// NEP-245 token identified by its contract account ID and token ID.
    Nep245(Nep245Token),
}

impl BridgedToken {
    /// Accepts tagged token arguments and the original NEP-141 `AccountId` encoding.
    pub fn deserialize(bytes: &[u8]) -> borsh::io::Result<Self> {
        Self::try_from_slice(bytes).or_else(|_| {
            AccountId::try_from_slice(bytes).map(|account_id| Self::Nep141(account_id.into()))
        })
    }

    /// Account id of the token contract.
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        match self {
            Self::Nep141(token) => &token.0,
            Self::Nep245(token) => &token.account_id,
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq)]
pub struct Nep141Token(pub AccountId);

impl AsRef<[u8]> for Nep141Token {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl TryFrom<Vec<u8>> for Nep141Token {
    type Error = crate::account_id::ParseAccountError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        AccountId::try_from(bytes).map(Self)
    }
}

impl From<AccountId> for Nep141Token {
    fn from(account_id: AccountId) -> Self {
        Self(account_id)
    }
}

/// Identifies a NEP-245 token by its contract account ID and token ID.
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, Eq, PartialEq)]
pub struct Nep245Token {
    /// NEAR account ID of the NEP-245 contract.
    pub account_id: AccountId,
    /// Identifier of the token within the NEP-245 contract.
    pub token_id: crate::TokenId,
}

impl ToBytes for Nep245Token {
    fn to_bytes(&self) -> borsh::io::Result<Cow<'_, [u8]>> {
        borsh::to_vec(self).map(Cow::Owned)
    }
}

impl TryFrom<Vec<u8>> for Nep245Token {
    type Error = borsh::io::Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from_slice(&bytes)
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

#[cfg(test)]
mod tests {
    use super::{
        BridgedToken, Erc20Identifier, Erc20Metadata, MirrorErc20TokenArgs, Nep245Metadata,
        Nep245Token,
    };

    #[test]
    fn test_exit_callback_wire_compatibility() {
        use super::{
            ExitToNearPrecompileCallbackArgs, RefundCallArgs, TransferCall, TransferNearArgs,
        };
        use crate::types::Address;
        let refund = RefundCallArgs {
            recipient_address: Address::from_array([1; 20]),
            erc20_address: Some(Address::from_array([2; 20])),
            amount: crate::U256::from(100).to_big_endian(),
        };
        assert_eq!(TransferCall::default(), TransferCall::None);
        for current in [
            ExitToNearPrecompileCallbackArgs::default(),
            ExitToNearPrecompileCallbackArgs {
                refund: Some(refund.clone()),
                transfer_near: None,
                ..Default::default()
            },
            ExitToNearPrecompileCallbackArgs {
                refund: None,
                transfer_near: Some(TransferNearArgs {
                    target_account_id: "receiver.near".parse().unwrap(),
                    amount: 100,
                }),
                ..Default::default()
            },
        ] {
            let legacy_bytes = borsh::to_vec(&(&current.refund, &current.transfer_near)).unwrap();
            let bytes = borsh::to_vec(&current).unwrap();
            assert_eq!(&bytes[..bytes.len() - 1], legacy_bytes);
            assert_eq!(bytes.last(), Some(&0));
            assert_eq!(
                ExitToNearPrecompileCallbackArgs::deserialize(&legacy_bytes).unwrap(),
                current
            );
            assert_eq!(
                ExitToNearPrecompileCallbackArgs::deserialize(&bytes).unwrap(),
                current
            );
        }
        let current = ExitToNearPrecompileCallbackArgs {
            refund: Some(refund),
            transfer_near: None,
            transfer_call: TransferCall::Nep245,
        };
        let bytes = borsh::to_vec(&current).unwrap();
        assert_eq!(bytes.last(), Some(&1));
        assert_eq!(
            ExitToNearPrecompileCallbackArgs::deserialize(&bytes).unwrap(),
            current
        );
        assert_eq!(
            ExitToNearPrecompileCallbackArgs::deserialize(&bytes[..bytes.len() - 1]).unwrap(),
            ExitToNearPrecompileCallbackArgs {
                transfer_call: TransferCall::None,
                ..current
            }
        );
        assert!(ExitToNearPrecompileCallbackArgs::deserialize(&bytes[..bytes.len() - 2]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(ExitToNearPrecompileCallbackArgs::deserialize(&trailing).is_err());
        let ft_call = ExitToNearPrecompileCallbackArgs {
            transfer_call: TransferCall::Nep141,
            ..Default::default()
        };
        assert_eq!(borsh::to_vec(&ft_call).unwrap(), [0, 0, 2]);
        assert_eq!(
            ExitToNearPrecompileCallbackArgs::deserialize(&[0, 0, 2]).unwrap(),
            ft_call
        );
        for bytes in [
            &[][..],
            &[0][..],
            &[1][..],
            &[2][..],
            &[3][..],
            &[0, 0, 3][..],
            &[0, 0, 0, 0][..],
        ] {
            assert!(ExitToNearPrecompileCallbackArgs::deserialize(bytes).is_err());
        }
    }
    use crate::{account_id::AccountId, types::Address};
    use borsh::{self, BorshDeserialize};
    use serde_json::{Value, json};

    #[test]
    fn test_mirror_erc20_token_args_formats() {
        let contract_id = "aurora.near".parse::<AccountId>().unwrap();
        let nep141 = BridgedToken::Nep141("wrap.near".parse::<AccountId>().unwrap().into());
        let nep245 = BridgedToken::Nep245(Nep245Token {
            account_id: "multi.near".parse().unwrap(),
            token_id: "coin".into(),
        });
        let prefix = b"\x0b\x00\x00\x00aurora.near";
        for (token, suffix) in [
            (nep141.clone(), b"\x09\x00\x00\x00wrap.near".as_slice()),
            (nep141, b"\x00\x09\x00\x00\x00wrap.near".as_slice()),
            (
                nep245,
                b"\x01\x0a\x00\x00\x00multi.near\x04\x00\x00\x00coin".as_slice(),
            ),
        ] {
            let input = [prefix.as_slice(), suffix].concat();
            let args = MirrorErc20TokenArgs::deserialize(&input).unwrap();
            assert_eq!(args.contract_id, contract_id);
            assert_eq!(args.token, token);
            assert_eq!(
                MirrorErc20TokenArgs::deserialize(&borsh::to_vec(&args).unwrap()).unwrap(),
                args
            );
            let mut truncated = input.clone();
            truncated.pop();
            assert!(MirrorErc20TokenArgs::deserialize(&truncated).is_err());
            let mut trailing = input;
            trailing.push(0);
            assert!(MirrorErc20TokenArgs::deserialize(&trailing).is_err());
        }
    }

    #[test]
    fn test_bridged_token_formats() {
        let nep141 = BridgedToken::Nep141("wrap.near".parse::<AccountId>().unwrap().into());
        let nep245 = BridgedToken::Nep245(Nep245Token {
            account_id: "multi.near".parse().unwrap(),
            token_id: "coin".into(),
        });
        for (token, input) in [
            (nep141.clone(), b"\x09\x00\x00\x00wrap.near".as_slice()),
            (nep141, b"\x00\x09\x00\x00\x00wrap.near".as_slice()),
            (
                nep245,
                b"\x01\x0a\x00\x00\x00multi.near\x04\x00\x00\x00coin".as_slice(),
            ),
        ] {
            assert_eq!(BridgedToken::deserialize(input).unwrap(), token);
            assert_eq!(
                BridgedToken::deserialize(&borsh::to_vec(&token).unwrap()).unwrap(),
                token
            );
            let mut truncated = input.to_vec();
            truncated.pop();
            assert!(BridgedToken::deserialize(&truncated).is_err());
            let mut trailing = input.to_vec();
            trailing.push(0);
            assert!(BridgedToken::deserialize(&trailing).is_err());
        }
        assert!(BridgedToken::deserialize(&[2, 0, 0, 0]).is_err());
    }

    #[test]
    fn test_erc20_identifier_json_formats() {
        let account_id: AccountId = "multi.near".parse().unwrap();
        for (json, expected) in [
            (
                r#"{"address":"0x1111111111111111111111111111111111111111"}"#,
                Erc20Identifier::Erc20 {
                    address: Address::from_array([0x11; 20]),
                },
            ),
            (
                r#"{"account_id":"multi.near"}"#,
                Erc20Identifier::Nep141 {
                    account_id: account_id.clone(),
                },
            ),
            (
                r#"{"token_id":"coin","account_id":"multi.near"}"#,
                Erc20Identifier::Nep245 {
                    account_id,
                    token_id: "coin".into(),
                },
            ),
        ] {
            let parsed: Erc20Identifier = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(
                serde_json::to_value(&expected).unwrap(),
                serde_json::from_str::<Value>(json).unwrap()
            );
            assert_eq!(
                serde_json::from_slice::<Erc20Identifier>(&serde_json::to_vec(&expected).unwrap())
                    .unwrap(),
                expected
            );

            let mut extended = serde_json::to_value(&expected).unwrap();
            extended["future_extension"] = json!({ "token_id": null, "other": [1, true] });
            assert_eq!(
                serde_json::from_value::<Erc20Identifier>(extended).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn test_erc20_identifier_rejects_invalid_fields() {
        let address = Address::from_array([0x11; 20]);
        for json in [
            json!({}),
            json!(null),
            json!([]),
            json!("multi.near"),
            json!({ "token_id": "coin" }),
            json!({ "account_id": "multi.near", "token_id": null }),
            json!({ "account_id": "multi.near", "token_id": 7 }),
            json!({ "account_id": "multi.near", "token_id": true }),
            json!({ "account_id": "multi.near", "token_id": {} }),
            json!({ "account_id": "multi.near", "token_id": [] }),
            json!({ "account_id": null }),
            json!({ "account_id": 7 }),
            json!({ "account_id": "INVALID!", "token_id": "coin" }),
            json!({ "address": null }),
            json!({ "address": "invalid" }),
            json!({ "address": address, "account_id": "multi.near" }),
            json!({ "address": address, "token_id": "coin" }),
            json!({ "address": address, "account_id": "multi.near", "token_id": "coin" }),
        ] {
            let bytes = serde_json::to_vec(&json).unwrap();
            assert!(
                serde_json::from_slice::<Erc20Identifier>(&bytes).is_err(),
                "Accepted invalid identifier: {json}"
            );
        }
    }

    #[test]
    fn test_erc20_identifier_rejects_duplicate_fields() {
        for json in [
            r#"{"address":"0x1111111111111111111111111111111111111111","address":"0x2222222222222222222222222222222222222222"}"#,
            r#"{"account_id":"first.near","account_id":"second.near"}"#,
            r#"{"account_id":"multi.near","token_id":"first","token_id":"second"}"#,
        ] {
            let error = serde_json::from_str::<Erc20Identifier>(json).unwrap_err();
            assert!(error.to_string().starts_with("duplicate field"));
        }
    }

    #[test]
    fn test_erc20_identifier_preserves_borsh_encoding() {
        let account_id: AccountId = "multi.near".parse().unwrap();
        for (expected, bytes) in [
            (
                Erc20Identifier::Erc20 {
                    address: Address::from_array([0x11; 20]),
                },
                [&[0][..], &[0x11; 20]].concat(),
            ),
            (
                Erc20Identifier::Nep141 {
                    account_id: account_id.clone(),
                },
                b"\x01\x0a\x00\x00\x00multi.near".to_vec(),
            ),
            (
                Erc20Identifier::Nep245 {
                    account_id,
                    token_id: "coin".into(),
                },
                b"\x02\x0a\x00\x00\x00multi.near\x04\x00\x00\x00coin".to_vec(),
            ),
        ] {
            assert_eq!(borsh::to_vec(&expected).unwrap(), bytes);
            assert_eq!(Erc20Identifier::try_from_slice(&bytes).unwrap(), expected);
        }
    }

    fn to_erc20_metadata(metadata: Value) -> Option<Erc20Metadata> {
        serde_json::from_value::<Nep245Metadata>(metadata)
            .ok()
            .and_then(|metadata| Erc20Metadata::try_from(metadata).ok())
    }

    #[test]
    fn test_nep245_metadata_into_erc20_metadata() {
        for (metadata, symbol, decimals) in [
            // Fields that `Erc20Metadata` doesn't have are ignored.
            (
                json!({ "id": "wnear", "name": "Wrapped NEAR", "symbol": "wNEAR", "decimals": 24, "icon": null }),
                "wNEAR",
                24,
            ),
            (json!({ "name": "Wrapped NEAR", "decimals": 0 }), "EMPTY", 0),
            (
                json!({ "name": "Wrapped NEAR", "decimals": 255 }),
                "EMPTY",
                255,
            ),
            (
                json!({ "name": "Wrapped NEAR", "decimals": "0" }),
                "EMPTY",
                0,
            ),
            (
                json!({ "name": "Wrapped NEAR", "decimals": "255" }),
                "EMPTY",
                255,
            ),
            (json!({ "name": "Wrapped NEAR" }), "EMPTY", 0),
            (
                json!({ "name": "Wrapped NEAR", "symbol": null, "decimals": null }),
                "EMPTY",
                0,
            ),
        ] {
            assert_eq!(
                to_erc20_metadata(metadata),
                Some(Erc20Metadata {
                    name: "Wrapped NEAR".into(),
                    symbol: symbol.into(),
                    decimals,
                })
            );
        }
    }

    #[test]
    fn test_nep245_metadata_rejects_invalid_fields() {
        let invalid_decimals = [
            json!(-1),
            json!(256),
            json!(1.5),
            json!(24.0),
            json!(true),
            json!({}),
            json!([]),
            json!("-1"),
            json!("256"),
            json!("1.5"),
            json!("invalid"),
            json!(""),
        ]
        .map(|decimals| json!({ "name": "Wrapped NEAR", "symbol": "wNEAR", "decimals": decimals }));
        let invalid_name_or_symbol = [
            json!({ "symbol": "wNEAR", "decimals": 24 }),
            json!({ "name": null, "symbol": "wNEAR", "decimals": 24 }),
            json!({ "name": 24, "symbol": "wNEAR", "decimals": 24 }),
            json!({ "name": "Wrapped NEAR", "symbol": true, "decimals": 24 }),
        ];

        for metadata in invalid_decimals.into_iter().chain(invalid_name_or_symbol) {
            let description = metadata.to_string();
            assert_eq!(
                to_erc20_metadata(metadata),
                None,
                "Accepted invalid metadata: {description}"
            );
        }
    }
}
