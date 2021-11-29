use crate::log_entry::LogEntry;
use crate::prelude::{vec, EthAddress, String, ToString, Vec, U256};
use ethabi::{Event, EventParam, Hash, Log, ParamType, RawLog};

pub const DEPOSITED_EVENT: &str = "Deposited";

pub type EventParams = Vec<EventParam>;

/// Ethereum event
pub struct EthEvent {
    pub eth_custodian_address: EthAddress,
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
        let eth_custodian_address = log_entry.address.0;
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
#[derive(Debug, PartialEq)]
pub struct DepositedEvent {
    pub eth_custodian_address: EthAddress,
    pub sender: EthAddress,
    pub recipient: String,
    pub amount: U256,
    pub fee: U256,
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
        let sender = event.log.params[0]
            .value
            .clone()
            .into_address()
            .ok_or(error::ParseError::InvalidSender)?
            .0;
        let recipient: String = event.log.params[1].value.clone().to_string();
        let amount = event.log.params[2]
            .value
            .clone()
            .into_uint()
            .ok_or(error::ParseError::InvalidAmount)?;
        let fee = event.log.params[3]
            .value
            .clone()
            .into_uint()
            .ok_or(error::ParseError::InvalidFee)?;
        Ok(Self {
            eth_custodian_address: event.eth_custodian_address,
            sender,
            recipient,
            amount,
            fee,
        })
    }
}

pub mod error {
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

    #[derive(Debug)]
    pub enum ParseError {
        LogParseFailed(DecodeError),
        InvalidSender,
        InvalidAmount,
        InvalidFee,
    }
    impl AsRef<[u8]> for ParseError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::LogParseFailed(e) => e.as_ref(),
                Self::InvalidSender => b"ERR_INVALID_SENDER",
                Self::InvalidAmount => b"ERR_INVALID_AMOUNT",
                Self::InvalidFee => b"ERR_INVALID_FEE",
            }
        }
    }
}
