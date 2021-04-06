#[cfg(feature = "contract")]
use crate::json::{self, FAILED_PARSE};
use crate::prelude::{vec, Address, String, Vec, H256, U256};
#[cfg(feature = "contract")]
use alloc::str;

#[cfg(not(feature = "contract"))]
use sha3::{Digest, Keccak256};

use evm::backend::Log;

#[cfg(feature = "contract")]
use crate::sdk;

pub type AccountId = String;
pub type Balance = u128;
pub type RawAddress = [u8; 20];
pub type RawU256 = [u8; 32];
pub type RawH256 = [u8; 32];
pub type EthAddress = [u8; 20];
pub type Gas = u64;
pub type StorageUsage = u64;

pub const STORAGE_PRICE_PER_BYTE: u128 = 100_000_000_000_000_000_000; // 1e20yN, 0.0001N

/// Internal args format for meta call.
#[derive(Debug)]
pub struct InternalMetaCallArgs {
    pub sender: Address,
    pub nonce: U256,
    pub fee_amount: U256,
    pub fee_address: Address,
    pub contract_address: Address,
    pub value: U256,
    pub input: Vec<u8>,
}

/// eth-connector initial args
#[cfg(feature = "contract")]
pub struct InitCallArgs {
    pub prover_account: AccountId,
    pub eth_custodian_address: AccountId,
}

/// balance_of args for json invocation
#[cfg(feature = "contract")]
pub struct BalanceOfCallArgs {
    pub account_id: AccountId,
}

/// transfer args for json invocation
#[cfg(feature = "contract")]
pub struct TransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub memo: Option<String>,
}

/// withdraw eth-connector call args
#[cfg(feature = "contract")]
pub struct WithdrawCallArgs {
    pub recipient_id: AccountId,
    pub amount: Balance,
}

/// transfer eth-connector call args
#[cfg(feature = "contract")]
pub struct TransferCallCallArgs {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub memo: Option<String>,
    pub msg: String,
}

/// storage_balance_of eth-connector call args
#[cfg(feature = "contract")]
pub struct StorageBalanceOfCallArgs {
    pub account_id: AccountId,
}

/// storage_withdraw eth-connector call args
#[cfg(feature = "contract")]
pub struct StorageWithdrawCallArgs {
    pub amount: Option<u128>,
}

/// storage_deposit eth-connector call args
#[cfg(feature = "contract")]
pub struct StorageDepositCallArgs {
    pub account_id: Option<AccountId>,
    pub registration_only: Option<bool>,
}

pub struct StorageBalanceBounds {
    pub min: Balance,
    pub max: Option<Balance>,
}

/// promise results structure
#[cfg(feature = "contract")]
pub enum PromiseResult {
    NotReady,
    Successful(Vec<u8>),
    Failed,
}

/// ft_resolve_transfer result of eth-connector
#[cfg(feature = "contract")]
pub struct FtResolveTransferResult {
    pub amount: Balance,
    pub refund_amount: Balance,
}

/// Internal errors to propagate up and format in the single place.
pub enum ErrorKind {
    ArgumentParseError,
    InvalidMetaTransactionMethodName,
    InvalidMetaTransactionFunctionArg,
    InvalidEcRecoverSignature,
}

pub type Result<T> = core::result::Result<T, ErrorKind>;

#[allow(dead_code)]
pub fn u256_to_arr(value: &U256) -> [u8; 32] {
    let mut result = [0u8; 32];
    value.to_big_endian(&mut result);
    result
}

#[allow(dead_code)]
pub fn log_to_bytes(log: Log) -> Vec<u8> {
    let mut result = vec![0u8; 1 + log.topics.len() * 32 + log.data.len()];
    result[0] = log.topics.len() as u8;
    let mut index = 1;
    for topic in log.topics.iter() {
        result[index..index + 32].copy_from_slice(&topic.0);
        index += 32;
    }
    result[index..].copy_from_slice(&log.data);
    result
}

const HEX_ALPHABET: &[u8; 16] = b"0123456789abcdef";

#[allow(dead_code)]
pub fn bytes_to_hex(v: &[u8]) -> String {
    let mut result = String::new();
    for x in v {
        result.push(HEX_ALPHABET[(x / 16) as usize] as char);
        result.push(HEX_ALPHABET[(x % 16) as usize] as char);
    }
    result
}

#[cfg(feature = "contract")]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    sdk::keccak(data)
}

#[cfg(not(feature = "contract"))]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(data).as_slice())
}

#[allow(dead_code)]
pub fn near_account_to_evm_address(addr: &[u8]) -> Address {
    Address::from_slice(&keccak(addr)[12..])
}

#[cfg(feature = "contract")]
pub fn str_from_slice(inp: &[u8]) -> &str {
    str::from_utf8(inp).unwrap()
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for BalanceOfCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            account_id: v.string("account_id").expect(str_from_slice(FAILED_PARSE)),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for InitCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            eth_custodian_address: v
                .string("eth_custodian_address")
                .expect(str_from_slice(FAILED_PARSE)),
            prover_account: v
                .string("prover_account")
                .expect(str_from_slice(FAILED_PARSE)),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for WithdrawCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            recipient_id: v
                .string("recipient_id")
                .expect(str_from_slice(FAILED_PARSE)),
            amount: v.u128("amount").expect(str_from_slice(FAILED_PARSE)),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for StorageWithdrawCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            amount: v.u128("amount").ok(),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for StorageBalanceOfCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            account_id: v.string("account_id").expect(str_from_slice(FAILED_PARSE)),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for StorageDepositCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            account_id: v.string("account_id").ok(),
            registration_only: v.bool("registration_only").ok(),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for TransferCallCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            receiver_id: v.string("receiver_id").expect(str_from_slice(FAILED_PARSE)),
            amount: v.u128("amount").expect(str_from_slice(FAILED_PARSE)),
            memo: v.string("memo").ok(),
            msg: v.string("msg").expect(str_from_slice(FAILED_PARSE)),
        }
    }
}

#[cfg(feature = "contract")]
impl From<json::JsonValue> for TransferCallArgs {
    fn from(v: json::JsonValue) -> Self {
        Self {
            receiver_id: v.string("receiver_id").expect(str_from_slice(FAILED_PARSE)),
            amount: v.u128("amount").expect(str_from_slice(FAILED_PARSE)),
            memo: v.string("memo").ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex() {
        assert_eq!(
            bytes_to_hex(&[0u8, 1u8, 255u8, 16u8]),
            "0001ff10".to_string()
        );
    }
}
