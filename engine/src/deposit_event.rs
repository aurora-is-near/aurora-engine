#[cfg(not(feature = "contract"))]
use prelude::Vec;
use prelude::{vec, String, ToString, Vec};

use crate::log_entry::LogEntry;
use crate::types::*;
use ethabi::{Event, EventParam, Hash, Log, ParamType, RawLog};
use primitive_types::U256;

const DEPOSITED_EVENT: &str = "Deposited";

pub type EventParams = Vec<EventParam>;

/// Ethereum event
pub struct EthEvent {
    pub eth_custodian_address: EthAddress,
    pub log: Log,
}

#[allow(dead_code)]
impl EthEvent {
    /// Get Ethereum event from `log_entry_data`
    pub fn fetch_log_entry_data(name: &str, params: EventParams, data: &[u8]) -> Self {
        let event = Event {
            name: name.to_string(),
            inputs: params,
            anonymous: false,
        };
        let log_entry: LogEntry = rlp::decode(data).expect("INVALID_RLP");
        let eth_custodian_address = log_entry.address.0;
        let topics = log_entry.topics.iter().map(|h| Hash::from(h.0)).collect();

        let raw_log = RawLog {
            topics,
            data: log_entry.data,
        };
        let log = event.parse_log(raw_log).expect("Failed to parse event log");

        Self {
            eth_custodian_address,
            log,
        }
    }

    /// Build log_entry_data from ethereum event
    #[cfg(not(feature = "contract"))]
    #[allow(dead_code)]
    pub fn params_to_log_entry_data(
        name: &str,
        params: EventParams,
        locker_address: EthAddress,
        indexes: Vec<Vec<u8>>,
        values: Vec<Token>,
    ) -> Vec<u8> {
        let event = Event {
            name: name.to_string(),
            inputs: params.into_iter().collect(),
            anonymous: false,
        };
        let params: Vec<ParamType> = event.inputs.iter().map(|p| p.kind.clone()).collect();
        let topics = indexes
            .into_iter()
            .map(|value| {
                let mut result: [u8; 32] = Default::default();
                result[12..].copy_from_slice(value.as_slice());
                H256::from(result)
            })
            .collect();
        let log_entry = LogEntry {
            address: locker_address.into(),
            topics: vec![vec![long_signature(&event.name, &params).0.into()], topics].concat(),
            data: ethabi::encode(&values),
        };
        rlp::encode(&log_entry).to_vec()
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
    fn event_params() -> EventParams {
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
    pub fn from_log_entry_data(data: &[u8]) -> Self {
        let event = EthEvent::fetch_log_entry_data(DEPOSITED_EVENT, Self::event_params(), data);
        let sender = event.log.params[0].value.clone().into_address().unwrap().0;

        let recipient = event.log.params[1].value.clone().to_string();
        let amount = event.log.params[2].value.clone().into_uint().unwrap();
        let fee = event.log.params[3].value.clone().into_uint().unwrap();
        Self {
            eth_custodian_address: event.eth_custodian_address,
            sender,
            recipient,
            amount,
            fee,
        }
    }

    #[cfg(not(feature = "contract"))]
    #[allow(dead_code)]
    pub fn to_log_entry_data(&self) -> Vec<u8> {
        EthEvent::params_to_log_entry_data(
            DEPOSITED_EVENT,
            DepositedEvent::event_params(),
            self.eth_custodian_address,
            vec![self.sender.to_vec()],
            vec![
                ethabi::Token::String(self.recipient.clone()),
                ethabi::Token::Uint(self.amount),
                ethabi::Token::Uint(self.fee),
            ],
        )
    }
}
