#[cfg(not(feature = "contract"))]
use crate::prelude::Vec;
use crate::prelude::{vec, String, ToString};

use crate::types::*;
use ethabi::{EventParam, ParamType};
use primitive_types::U256;

const DEPOSITED_EVENT: &str = "Deposited";

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
