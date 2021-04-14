use super::prelude::*;
use super::sdk;
use crate::engine::Engine;
use crate::json::{self, FAILED_PARSE};
use crate::log_entry::LogEntry;
use crate::precompiles::ecrecover;
use crate::types::{str_from_slice, AccountId, EthAddress};
use alloc::format;
use borsh::{BorshDeserialize, BorshSerialize};
use ethabi::{Bytes, Event, EventParam, Hash, Log, ParamType, RawLog, Token};

/// Validate Etherium address from string and return EthAddress
#[allow(dead_code)]
pub fn validate_eth_address(address: String) -> EthAddress {
    let data = hex::decode(address).expect("ETH_ADDRESS_FAILED");
    assert_eq!(data.len(), 20, "ETH_WRONG_ADDRESS_LENGTH");
    let mut result = [0u8; 20];
    result.copy_from_slice(&data);
    result
}

/// Encodes vector of tokens using non-standard Packed mode into ABI.encodePacked() compliant vector of bytes.
pub fn encode_packed(tokens: &[Token]) -> Bytes {
    tokens.iter().flat_map(encode_token_packed).collect()
}

fn encode_token_packed(token: &Token) -> Vec<u8> {
    match *token {
        Token::Address(ref address) => {
            let mut padded = [0u8; 32];
            padded[12..].copy_from_slice(address.as_ref());
            padded[..].to_vec()
        }
        Token::Bytes(ref bytes) => bytes.to_vec(),
        Token::String(ref s) => s.as_bytes().to_vec(),
        Token::FixedBytes(ref bytes) => bytes.to_vec(),
        Token::Int(int) => {
            let data: [u8; 32] = int.into();
            data[..].to_vec()
        }
        Token::Uint(uint) => {
            let data: [u8; 32] = uint.into();
            data[..].to_vec()
        }
        Token::Bool(b) => {
            vec![b.into()]
        }
        Token::Array(_) | Token::FixedArray(_) | Token::Tuple(_) => {
            panic!("These token types are not supported in packed mode");
        }
    }
}

#[derive(Default, BorshDeserialize, BorshSerialize, Clone)]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
    pub skip_bridge_call: bool,
}

#[allow(dead_code)]
impl Proof {
    pub fn get_key(&self) -> String {
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

/// Parameters of Etherium event
pub type EthEventParams = Vec<(String, ParamType, bool)>;

/// Etherium event
pub struct EthEvent {
    pub eth_custodian_address: EthAddress,
    pub log: Log,
}

#[allow(dead_code)]
impl EthEvent {
    /// Get Etherium event from `log_entry_data`
    pub fn fetch_log_entry_data(name: &str, params: EthEventParams, data: &[u8]) -> Self {
        let event = Event {
            name: name.to_string(),
            inputs: params
                .into_iter()
                .map(|(name, kind, indexed)| EventParam {
                    name,
                    kind,
                    indexed,
                })
                .collect(),
            anonymous: false,
        };
        let log_entry: LogEntry = rlp::decode(data).expect("IVALID_RLP");
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
}

impl From<json::JsonValue> for Proof {
    fn from(v: json::JsonValue) -> Self {
        let log_index = v.u64("log_index").expect(str_from_slice(FAILED_PARSE));
        let log_entry_data: Vec<u8> = v
            .array("log_entry_data", json::JsonValue::parse_u8)
            .expect(str_from_slice(FAILED_PARSE));
        let receipt_index = v.u64("receipt_index").expect(str_from_slice(FAILED_PARSE));
        let receipt_data: Vec<u8> = v
            .array("receipt_data", json::JsonValue::parse_u8)
            .expect(str_from_slice(FAILED_PARSE));
        let header_data: Vec<u8> = v
            .array("header_data", json::JsonValue::parse_u8)
            .expect(str_from_slice(FAILED_PARSE));
        let proof = v
            .array("proof", |v1| match v1 {
                json::JsonValue::Array(arr) => arr.iter().map(json::JsonValue::parse_u8).collect(),
                _ => sdk::panic_utf8(FAILED_PARSE),
            })
            .expect(str_from_slice(FAILED_PARSE));

        let skip_bridge_call = v
            .bool("skip_bridge_call")
            .expect(str_from_slice(FAILED_PARSE));
        Self {
            log_index,
            log_entry_data,
            receipt_index,
            receipt_data,
            header_data,
            proof,
            skip_bridge_call,
        }
    }
}

const EIP_712_MSG_PREFIX: &[u8] = &[0x19, 0x01];
const EIP_712_DOMAIN_TYPEHASH: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const AURORA_DOMAIN_NAME: &str = "Aurora-Engine domain";
const AURORA_DOMAIN_VERSION: &str = "1.0";
const WITHDRAW_FROM_EVM_TYPEHASH: &str =
    "WithdrawFromEVMRequest(address ethRecipient,uint256 amount,address verifyingContract)";

/// Encode EIP712 withdraw message data
pub fn encode_withdraw_eip712(
    eth_recipient: EthAddress,
    amount: U256,
    custodian_address: EthAddress,
) -> H256 {
    let chain_id = U256::from(Engine::get_state().chain_id);

    let domain_separator_encoded = encode_packed(&[
        Token::FixedBytes(
            sdk::keccak(&encode_packed(&[Token::Bytes(
                EIP_712_DOMAIN_TYPEHASH.as_bytes().to_vec(),
            )]))
            .as_bytes()
            .to_vec(),
        ),
        Token::FixedBytes(encode_packed(&[
            // Domain
            Token::Bytes(
                sdk::keccak(AURORA_DOMAIN_NAME.as_bytes())
                    .as_bytes()
                    .to_vec(),
            ),
            // Version
            Token::Bytes(
                sdk::keccak(AURORA_DOMAIN_VERSION.as_bytes())
                    .as_bytes()
                    .to_vec(),
            ),
            // ChainID
            Token::Uint(chain_id),
            // Custodian address
            Token::Address(H160::from(custodian_address)),
        ])),
    ]);
    sdk::log(format!(
        "Domain_separator encoded: {}",
        hex::encode(domain_separator_encoded.clone())
    ));

    let domain_separator = sdk::keccak(&domain_separator_encoded);
    sdk::log(format!(
        "Domain_separator hash: {}",
        hex::encode(domain_separator)
    ));

    let withdraw_from_evm_struct_encoded = encode_packed(&[
        Token::FixedBytes(
            sdk::keccak(&encode_packed(&[Token::Bytes(
                WITHDRAW_FROM_EVM_TYPEHASH.as_bytes().to_vec(),
            )]))
            .as_bytes()
            .to_vec(),
        ),
        Token::FixedBytes(encode_packed(&[
            Token::Address(H160::from(eth_recipient)),
            Token::Uint(amount),
            Token::Address(H160::from(custodian_address)),
        ])),
    ]);
    sdk::log(format!(
        "WithdrawFromEVM struct encoded: {}",
        hex::encode(withdraw_from_evm_struct_encoded.clone()),
    ));

    let withdraw_from_evm_struct_hash = sdk::keccak(&withdraw_from_evm_struct_encoded);
    sdk::log(format!(
        "WithdrawFromEVM struct hash: {}",
        hex::encode(withdraw_from_evm_struct_hash)
    ));

    let digest_encoded = encode_packed(&[
        Token::Bytes(EIP_712_MSG_PREFIX.to_vec()),
        Token::FixedBytes(domain_separator.as_bytes().to_vec()),
        Token::FixedBytes(withdraw_from_evm_struct_hash.as_bytes().to_vec()),
    ]);
    sdk::log(format!(
        "digest_encoded: {}",
        hex::encode(digest_encoded.clone())
    ));

    let digest = sdk::keccak(&digest_encoded);
    sdk::log(format!("digest: {}", hex::encode(digest)));
    digest
}

pub fn verify_withdraw_eip712(
    sender: EthAddress,
    eth_recipient: EthAddress,
    custodian_address: EthAddress,
    amount: U256,
    eip712_signature: Vec<u8>,
) -> bool {
    let res = encode_withdraw_eip712(eth_recipient, amount, custodian_address);
    let withdraw_msg_signer = ecrecover(res, &eip712_signature[..]).unwrap();
    sdk::log(format!("sender: {}", hex::encode(sender)));
    sdk::log(format!("ecrecover: {}", hex::encode(withdraw_msg_signer)));

    //H160::from(sender) == withdraw_msg_signer
    true
}

#[allow(unused_variables)]
pub fn verify_transfer_eip712(
    sender: EthAddress,
    near_recipient: AccountId,
    amount: U256,
    eip712_signature: Vec<u8>,
) -> bool {
    // TODO: modify
    true
}
