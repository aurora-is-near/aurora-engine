use super::prelude::*;
use super::sdk;
use crate::engine::Engine;
use crate::log_entry::LogEntry;
use crate::precompiles::ecrecover;
use crate::types::{AccountId, EthAddress};
#[cfg(feature = "log")]
use alloc::format;
use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use ethabi::{Bytes, Event, EventParam, Hash, Log, RawLog, Token};

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
}

const EIP_712_MSG_PREFIX: &[u8] = &[0x19, 0x01];
const EIP_712_DOMAIN_TYPEHASH: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const AURORA_DOMAIN_NAME: &str = "Aurora-Engine domain";
const AURORA_DOMAIN_VERSION: &str = "1.0";
const WITHDRAW_FROM_EVM_TYPEHASH: &str =
    "WithdrawFromEVMRequest(address ethRecipient,uint256 amount,address verifyingContract)";
const TRANSFER_FROM_EVM_TO_NEAR_TYPEHASH: &str =
    "TransferFromEVMtoNearRequest(string nearRecipient,uint256 amount,uint256 fee)";

enum EIP712Recipient {
    Eth(EthAddress),
    Near(AccountId),
}

/// Encode EIP712 withdraw message data
fn encode_eip712(
    eth_recipient: EIP712Recipient,
    amount: U256,
    custodian_address: EthAddress,
    type_hash: &str,
) -> H256 {
    let chain_id = U256::from(Engine::get_state().unwrap().chain_id);

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
    crate::log!(&format!(
        "Domain_separator encoded: {}",
        hex::encode(domain_separator_encoded.clone())
    ));

    let domain_separator = sdk::keccak(&domain_separator_encoded);
    crate::log!(&format!(
        "Domain_separator hash: {}",
        hex::encode(domain_separator)
    ));

    let token_address = match eth_recipient {
        EIP712Recipient::Eth(eth_recipient) => Token::Address(H160::from(eth_recipient)),
        EIP712Recipient::Near(account_id) => Token::String(account_id),
    };
    let withdraw_from_evm_struct_encoded = encode_packed(&[
        Token::FixedBytes(
            sdk::keccak(&encode_packed(&[Token::Bytes(
                type_hash.as_bytes().to_vec(),
            )]))
            .as_bytes()
            .to_vec(),
        ),
        Token::FixedBytes(encode_packed(&[
            token_address,
            Token::Uint(amount),
            Token::Address(H160::from(custodian_address)),
        ])),
    ]);
    crate::log!(&format!(
        "WithdrawFromEVM struct encoded: {}",
        hex::encode(withdraw_from_evm_struct_encoded.clone()),
    ));

    let withdraw_from_evm_struct_hash = sdk::keccak(&withdraw_from_evm_struct_encoded);
    crate::log!(&format!(
        "WithdrawFromEVM struct hash: {}",
        hex::encode(withdraw_from_evm_struct_hash)
    ));

    let digest_encoded = encode_packed(&[
        Token::Bytes(EIP_712_MSG_PREFIX.to_vec()),
        Token::FixedBytes(domain_separator.as_bytes().to_vec()),
        Token::FixedBytes(withdraw_from_evm_struct_hash.as_bytes().to_vec()),
    ]);
    crate::log!(&format!(
        "digest_encoded: {}",
        hex::encode(digest_encoded.clone())
    ));

    let digest = sdk::keccak(&digest_encoded);
    crate::log!(&format!("digest: {}", hex::encode(digest)));
    digest
}

#[allow(dead_code)]
pub fn verify_withdraw_eip712(
    sender: EthAddress,
    eth_recipient: EthAddress,
    custodian_address: EthAddress,
    amount: U256,
    eip712_signature: Vec<u8>,
) -> bool {
    let res = encode_eip712(
        EIP712Recipient::Eth(eth_recipient),
        amount,
        custodian_address,
        WITHDRAW_FROM_EVM_TYPEHASH,
    );
    let withdraw_msg_signer = ecrecover(res, &eip712_signature[..]).unwrap();
    crate::log!(&format!("sender: {}", hex::encode(sender)));
    crate::log!(&format!("ecrecover: {}", hex::encode(withdraw_msg_signer)));
    crate::log!(&format!(
        "ecrecover: {}",
        H160::from(sender) == withdraw_msg_signer
    ));

    H160::from(sender) == withdraw_msg_signer
}

#[allow(dead_code)]
pub fn verify_transfer_eip712(
    sender: EthAddress,
    near_recipient: AccountId,
    custodian_address: EthAddress,
    amount: U256,
    eip712_signature: Vec<u8>,
) -> bool {
    let res = encode_eip712(
        EIP712Recipient::Near(near_recipient),
        amount,
        custodian_address,
        TRANSFER_FROM_EVM_TO_NEAR_TYPEHASH,
    );
    let withdraw_msg_signer = ecrecover(res, &eip712_signature[..]).unwrap();
    crate::log!(&format!("sender: {}", hex::encode(sender)));
    crate::log!(&format!("ecrecover: {}", hex::encode(withdraw_msg_signer)));
    crate::log!(&format!(
        "ecrecover: {}",
        H160::from(sender) == withdraw_msg_signer
    ));

    H160::from(sender) == withdraw_msg_signer
}
