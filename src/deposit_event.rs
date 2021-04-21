use crate::prover::*;
use crate::types::*;
use alloc::{
    string::{String, ToString},
    vec,
};
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
                kind: ParamType::Bytes,
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

    /// Parse raw log Etherium proof entry data.
    pub fn from_log_entry_data(data: &[u8]) -> Self {
        let event = EthEvent::fetch_log_entry_data(DEPOSITED_EVENT, Self::event_params(), data);
        let sender = event.log.params[0].value.clone().into_address().unwrap().0;

        let recipient = event.log.params[1].value.clone().to_string();
        let amount = U256::from(event.log.params[2].value.clone().into_uint().unwrap());
        let fee = U256::from(event.log.params[3].value.clone().into_uint().unwrap());
        Self {
            eth_custodian_address: event.eth_custodian_address,
            sender,
            recipient,
            amount,
            fee,
        }
    }
}
