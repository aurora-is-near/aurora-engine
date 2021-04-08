use crate::prover::*;
use crate::types::*;
use alloc::{string::ToString, vec};
use ethabi::ParamType;
use primitive_types::U128;

const EVENT_DEPOSIT_TO_NEAR: &str = "DepositedToNear";
const EVENT_DEPOSIT_TO_ETH: &str = "DepositedToEVM";

/// Data that was emitted by the Ethereum Deposited NEAR-token event.
#[derive(Debug, PartialEq)]
pub struct EthDepositedNearEvent {
    pub eth_custodian_address: EthAddress,
    pub sender: AccountId,
    pub recipient: AccountId,
    pub amount: U128,
    pub fee: U128,
}

/// Data that was emitted by the Ethereum Deposited ETH-token event.
#[derive(Debug, PartialEq)]
pub struct EthDepositedEthEvent {
    pub eth_custodian_address: EthAddress,
    pub sender: EthAddress,
    pub recipient: EthAddress,
    pub amount: U128,
    pub fee: U128,
    pub relayer_eth_account: EthAddress,
}

impl EthDepositedNearEvent {
    #[allow(dead_code)]
    fn event_params() -> EthEventParams {
        vec![
            ("sender".to_string(), ParamType::Address, true),
            ("nearRecipient".to_string(), ParamType::String, false),
            ("amount".to_string(), ParamType::Uint(256), false),
            ("fee".to_string(), ParamType::Uint(256), false),
        ]
    }

    /// Parse raw log Etherium proof entry data.
    #[allow(dead_code)]
    pub fn from_log_entry_data(data: &[u8]) -> Self {
        let event =
            EthEvent::fetch_log_entry_data(EVENT_DEPOSIT_TO_NEAR, Self::event_params(), data);
        let sender = event.log.params[0].value.clone().into_address().unwrap().0;
        let sender = hex::encode(sender);

        let recipient = event.log.params[1].value.clone().to_string();
        let amount = U128::from(
            event.log.params[2]
                .value
                .clone()
                .into_uint()
                .unwrap()
                .as_u128(),
        );
        let fee = U128::from(
            event.log.params[3]
                .value
                .clone()
                .into_uint()
                .unwrap()
                .as_u128(),
        );
        Self {
            eth_custodian_address: event.eth_custodian_address,
            sender,
            recipient,
            amount,
            fee,
        }
    }
}

impl EthDepositedEthEvent {
    #[allow(dead_code)]
    fn event_params() -> EthEventParams {
        vec![
            ("sender".to_string(), ParamType::Address, true),
            ("ethRecipientOnNear".to_string(), ParamType::Address, false),
            ("amount".to_string(), ParamType::Uint(256), false),
            ("fee".to_string(), ParamType::Uint(256), false),
            ("relayerEthAccount".to_string(), ParamType::Address, true),
        ]
    }

    /// Parse raw log Etherium proof entry data.
    #[allow(dead_code)]
    pub fn from_log_entry_data(data: &[u8]) -> Self {
        let event =
            EthEvent::fetch_log_entry_data(EVENT_DEPOSIT_TO_ETH, Self::event_params(), data);
        let sender = event.log.params[0].value.clone().into_address().unwrap().0;

        let recipient = event.log.params[1].value.clone().into_address().unwrap().0;
        let amount = U128::from(
            event.log.params[2]
                .value
                .clone()
                .into_uint()
                .unwrap()
                .as_u128(),
        );
        let fee = U128::from(
            event.log.params[3]
                .value
                .clone()
                .into_uint()
                .unwrap()
                .as_u128(),
        );
        let relayer_eth_account = event.log.params[4].value.clone().into_address().unwrap().0;
        Self {
            eth_custodian_address: event.eth_custodian_address,
            sender,
            recipient,
            amount,
            fee,
            relayer_eth_account,
        }
    }
}
