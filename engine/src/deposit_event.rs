use crate::deposit_event::error::ParseEventMessageError;
use crate::log_entry::LogEntry;
use crate::prelude::account_id::AccountId;
use crate::prelude::{
    types_new::Address, vec, Balance, BorshDeserialize, BorshSerialize, Fee, String, ToString,
    TryFrom, TryInto, Vec, U256,
};
use aurora_engine_types::types_new::address::error::AddressError;
use byte_slice_cast::AsByteSlice;
use ethabi::{Event, EventParam, Hash, Log, ParamType, RawLog};

pub const DEPOSITED_EVENT: &str = "Deposited";

pub type EventParams = Vec<EventParam>;

/// On-transfer message. Used for `ft_transfer_call` and  `ft_on_transfer` functions.
/// Message parsed from input args with `parse_on_transfer_message`.
#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct FtTransferMessageData {
    pub relayer: AccountId,
    pub recipient: Address,
    pub fee: Fee,
}

impl FtTransferMessageData {
    /// Get on-transfer data from arguments message field.
    /// Used for `ft_transfer_call` and `ft_on_transfer`
    pub fn parse_on_transfer_message(
        message: &str,
    ) -> Result<Self, error::ParseOnTransferMessageError> {
        // Split message by separator
        let data: Vec<_> = message.split(':').collect();
        // Message data array should contain 2 elements
        if data.len() != 2 {
            return Err(error::ParseOnTransferMessageError::TooManyParts);
        }

        // Check relayer account id from 1-th data element
        let account_id = AccountId::try_from(data[0].as_bytes())
            .map_err(|_| error::ParseOnTransferMessageError::InvalidAccount)?;

        // Decode message array from 2-th element of data array
        let msg =
            hex::decode(data[1]).map_err(|_| error::ParseOnTransferMessageError::InvalidHexData)?;
        // Length = fee[32] + eth_address[20] bytes
        if msg.len() != 52 {
            return Err(error::ParseOnTransferMessageError::WrongMessageFormat);
        }

        // Parse fee from message slice. It should contain 32 bytes
        // But after that in will be parse to u128
        // That logic for compatability.
        let mut raw_fee: [u8; 32] = Default::default();
        raw_fee.copy_from_slice(&msg[..32]);
        let fee_u128: u128 = U256::from_little_endian(&raw_fee)
            .try_into()
            .map_err(|_| error::ParseOnTransferMessageError::OverflowNumber)?;
        let fee: Fee = fee_u128.into();

        // Get recipient Eth address from message slice
        let recipient = Address::from_slice(&msg[32..52]);

        Ok(FtTransferMessageData {
            relayer: account_id,
            recipient,
            fee,
        })
    }

    /// Encode to String with specific rules
    pub fn encode(&self) -> String {
        // The first data section should contain fee data.
        // Pay attention, that for compatibility reasons we used U256 type
        // it means 32 bytes for fee data
        let mut data = U256::from(self.fee.into_u128()).as_byte_slice().to_vec();
        // Second data section should contain Eth address
        data.extend(self.recipient.as_bytes());
        // Add `:` separator between relayer_id and data message
        [self.relayer.as_ref(), &hex::encode(data)].join(":")
    }

    /// Prepare message for `ft_transfer_call` -> `ft_on_transfer`
    pub fn prepare_message_for_on_transfer(
        relayer_account_id: &AccountId,
        fee: Fee,
        recipient: String,
    ) -> Result<Self, ParseEventMessageError> {
        // The first data section should contain fee data.
        // Pay attention, that for compatibility reasons we used U256 type
        // it means 32 bytes for fee data
        let mut data = U256::from(fee.into_u128()).as_byte_slice().to_vec();

        // Check message length.
        let address = if recipient.len() == 42 {
            recipient
                .strip_prefix("0x")
                .ok_or(ParseEventMessageError::EthAddressValidationError(
                    AddressError::FailedDecodeHex,
                ))?
                .to_string()
        } else {
            recipient
        };
        let recipient_address =
            Address::decode(address).map_err(ParseEventMessageError::EthAddressValidationError)?;
        // Second data section should contain Eth address
        data.extend(recipient_address.as_bytes());
        // Add `:` separator between relayer_id and data message
        //Ok([relayer_account_id.as_ref(), &hex::encode(data)].join(":"))
        Ok(Self {
            relayer: relayer_account_id.clone(),
            recipient: recipient_address,
            fee,
        })
    }
}

/// Token message data used for Deposit flow.
/// It contains two basic data structure: Near, Eth
/// The message parsed from event `recipient` field - `log_entry_data`
/// after fetching proof `log_entry_data`
#[derive(BorshSerialize, BorshDeserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum TokenMessageData {
    /// Deposit no NEAR account
    Near(AccountId),
    ///Deposit to Eth accounts fee is being minted in the `ft_on_transfer` callback method
    Eth {
        receiver_id: AccountId,
        message: FtTransferMessageData,
    },
}

impl TokenMessageData {
    /// Parse event message data for tokens. Data parsed form event `recipient` field.
    /// Used for Deposit flow.
    /// For Eth logic flow message validated and prepared for  `ft_on_transfer` logic.
    /// It mean validating Eth address correctness and preparing message for
    /// parsing for `ft_on_transfer` message parsing with correct and validated data.
    pub fn parse_event_message_and_prepare_token_message_data(
        message: &str,
        fee: Fee,
    ) -> Result<TokenMessageData, error::ParseEventMessageError> {
        let data: Vec<_> = message.split(':').collect();
        // Data array can contain 1 or 2 elements
        if data.len() >= 3 {
            return Err(error::ParseEventMessageError::TooManyParts);
        }
        let account_id = AccountId::try_from(data[0].as_bytes())
            .map_err(|_| error::ParseEventMessageError::InvalidAccount)?;

        // If data array contain only one element it should return NEAR account id
        if data.len() == 1 {
            Ok(TokenMessageData::Near(account_id))
        } else {
            let raw_message = data[1].into();
            let message = FtTransferMessageData::prepare_message_for_on_transfer(
                &account_id,
                fee,
                raw_message,
            )?;

            Ok(TokenMessageData::Eth {
                receiver_id: account_id,
                message,
            })
        }
    }

    // Get recipient account id from Eth part of Token message data
    pub fn get_recipient(&self) -> AccountId {
        match self {
            Self::Near(acc) => acc.clone(),
            Self::Eth {
                receiver_id,
                message: _,
            } => receiver_id.clone(),
        }
    }
}

/// Ethereum event
pub struct EthEvent {
    pub eth_custodian_address: Address,
    pub log: Log,
}

#[allow(dead_code)]
impl EthEvent {
    /// Get Ethereum event from `log_entry_data`
    pub fn fetch_log_entry_data(
        name: &str,
        params: EventParams,
        data: &[u8],
    ) -> Result<Self, error::DecodeError> {
        let event = Event {
            name: name.to_string(),
            inputs: params,
            anonymous: false,
        };
        let log_entry: LogEntry = rlp::decode(data).map_err(|_| error::DecodeError::RlpFailed)?;
        let eth_custodian_address = Address::from_slice(&log_entry.address.0);
        let topics = log_entry.topics.iter().map(|h| Hash::from(h.0)).collect();

        let raw_log = RawLog {
            topics,
            data: log_entry.data,
        };
        let log = event
            .parse_log(raw_log)
            .map_err(|_| error::DecodeError::SchemaMismatch)?;

        Ok(Self {
            eth_custodian_address,
            log,
        })
    }
}

/// Data that was emitted by Deposited event.
pub struct DepositedEvent {
    pub eth_custodian_address: Address,
    pub sender: Address,
    pub token_message_data: TokenMessageData,
    pub amount: Balance,
    pub fee: Fee,
}

impl DepositedEvent {
    #[allow(dead_code)]
    pub fn event_params() -> EventParams {
        vec![
            EventParam {
                name: "sender".to_string(),
                kind: ParamType::Address,
                indexed: true,
            },
            EventParam {
                name: "recipient".to_string(),
                kind: ParamType::String,
                indexed: false,
            },
            EventParam {
                name: "amount".to_string(),
                kind: ParamType::Uint(256),
                indexed: false,
            },
            EventParam {
                name: "fee".to_string(),
                kind: ParamType::Uint(256),
                indexed: false,
            },
        ]
    }

    /// Parses raw Ethereum logs proof's entry data
    pub fn from_log_entry_data(data: &[u8]) -> Result<Self, error::ParseError> {
        let event = EthEvent::fetch_log_entry_data(DEPOSITED_EVENT, Self::event_params(), data)
            .map_err(error::ParseError::LogParseFailed)?;
        let raw_sender = event.log.params[0]
            .value
            .clone()
            .into_address()
            .ok_or(error::ParseError::InvalidSender)?
            .0;
        let sender = Address::from_slice(&raw_sender);

        // parse_event_message
        let event_message_data: String = event.log.params[1].value.clone().to_string();

        let amount: u128 = event.log.params[2]
            .value
            .clone()
            .into_uint()
            .ok_or(error::ParseError::InvalidAmount)?
            .try_into()
            .map_err(|_| error::ParseError::OverflowNumber)?;
        let raw_fee: u128 = event.log.params[3]
            .value
            .clone()
            .into_uint()
            .ok_or(error::ParseError::InvalidFee)?
            .try_into()
            .map_err(|_| error::ParseError::OverflowNumber)?;
        let fee: Fee = raw_fee.into();

        let token_message_data =
            TokenMessageData::parse_event_message_and_prepare_token_message_data(
                &event_message_data,
                fee,
            )?;

        Ok(Self {
            eth_custodian_address: event.eth_custodian_address,
            sender,
            token_message_data,
            amount,
            fee,
        })
    }
}

pub mod error {
    use super::*;

    #[derive(Debug)]
    pub enum DecodeError {
        RlpFailed,
        SchemaMismatch,
    }
    impl AsRef<[u8]> for DecodeError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::RlpFailed => b"ERR_RLP_FAILED",
                Self::SchemaMismatch => b"ERR_PARSE_DEPOSIT_EVENT",
            }
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
    pub enum ParseEventMessageError {
        TooManyParts,
        InvalidAccount,
        EthAddressValidationError(AddressError),
        ParseMessageError(ParseOnTransferMessageError),
    }

    impl AsRef<[u8]> for ParseEventMessageError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::TooManyParts => b"ERR_INVALID_EVENT_MESSAGE_FORMAT",
                Self::InvalidAccount => b"ERR_INVALID_ACCOUNT_ID",
                Self::EthAddressValidationError(e) => e.as_ref(),
                Self::ParseMessageError(e) => e.as_ref(),
            }
        }
    }

    impl From<ParseEventMessageError> for ParseError {
        fn from(e: ParseEventMessageError) -> Self {
            Self::MessageParseFailed(e)
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
    pub enum ParseError {
        LogParseFailed(DecodeError),
        InvalidSender,
        InvalidAmount,
        InvalidFee,
        MessageParseFailed(ParseEventMessageError),
        OverflowNumber,
    }
    impl AsRef<[u8]> for ParseError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::LogParseFailed(e) => e.as_ref(),
                Self::InvalidSender => b"ERR_INVALID_SENDER",
                Self::InvalidAmount => b"ERR_INVALID_AMOUNT",
                Self::InvalidFee => b"ERR_INVALID_FEE",
                Self::MessageParseFailed(e) => e.as_ref(),
                Self::OverflowNumber => b"ERR_OVERFLOW_NUMBER",
            }
        }
    }

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
