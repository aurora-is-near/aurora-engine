use crate::prelude::{sdk, BorshDeserialize, BorshSerialize, String, ToString, Vec};

#[derive(Debug, Default, BorshDeserialize, BorshSerialize, Clone)]
#[cfg_attr(feature = "impl-serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
}

impl Proof {
    pub fn key(&self) -> String {
        let mut data = self.log_index.try_to_vec().unwrap();
        data.extend(self.receipt_index.try_to_vec().unwrap());
        data.extend(self.header_data.clone());
        sdk::sha256(&data[..])
            .0
            .iter()
            .map(|n| n.to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deposit_event::{DepositedEvent, TokenMessageData, DEPOSITED_EVENT};
    use crate::log_entry::LogEntry;
    use aurora_engine_precompiles::make_address;
    use aurora_engine_types::types::{Address, Fee, NEP141Wei, Wei};
    use aurora_engine_types::{H160, U256};

    const ETH_CUSTODIAN_ADDRESS: Address =
        make_address(0xd045f7e1, 0x9b2488924b97f9c145b5e51d0d895a65);

    #[test]
    fn test_proof_key_generates_successfully() {
        let recipient_address = Address::new(H160([22u8; 20]));
        let deposit_amount = Wei::new_u64(123_456_789);
        let proof = self::create_proof(recipient_address, deposit_amount);

        let expected_key =
            "1297721518512077871939115641114233180253108247225100248224214775219368216419218177247";
        let actual_key = proof.key();

        assert_eq!(expected_key, actual_key);
    }

    fn create_proof(recipient_address: Address, deposit_amount: Wei) -> Proof {
        let eth_custodian_address = ETH_CUSTODIAN_ADDRESS;

        let fee = Fee::new(NEP141Wei::new(0));
        let message = ["aurora", ":", recipient_address.encode().as_str()].concat();
        let token_message_data: TokenMessageData =
            TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
                .unwrap();

        let deposit_event = DepositedEvent {
            eth_custodian_address,
            sender: Address::new(H160([0u8; 20])),
            token_message_data,
            amount: NEP141Wei::new(deposit_amount.raw().as_u128()),
            fee,
        };

        let event_schema = ethabi::Event {
            name: DEPOSITED_EVENT.into(),
            inputs: DepositedEvent::event_params(),
            anonymous: false,
        };
        let log_entry = LogEntry {
            address: eth_custodian_address.raw(),
            topics: vec![
                event_schema.signature(),
                // the sender is not important
                crate::prelude::H256::zero(),
            ],
            data: ethabi::encode(&[
                ethabi::Token::String(message),
                ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
                ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
            ]),
        };

        Proof {
            log_index: 1,
            // Only this field matters for the purpose of this test
            log_entry_data: rlp::encode(&log_entry).to_vec(),
            receipt_index: 1,
            receipt_data: Vec::new(),
            header_data: Vec::new(),
            proof: Vec::new(),
        }
    }
}
