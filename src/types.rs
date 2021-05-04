use crate::prelude::{vec, Address, String, Vec, H256, U256};
#[cfg(feature = "contract")]
use alloc::str;

#[cfg(not(feature = "contract"))]
use sha3::{Digest, Keccak256};

#[cfg(feature = "contract")]
use crate::sdk;

pub type AccountId = String;
pub type Balance = u128;
pub type RawAddress = [u8; 20];
pub type RawU256 = [u8; 32]; // Little-endian large integer type.
pub type RawH256 = [u8; 32]; // Unformatted binary data of fixed length.
pub type EthAddress = [u8; 20];
pub type Gas = u64;
pub type StorageUsage = u64;

pub const STORAGE_PRICE_PER_BYTE: u128 = 100_000_000_000_000_000_000; // 1e20yN, 0.0001N
pub const ERR_FAILED_PARSE: &str = "ERR_FAILED_PARSE";

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
pub trait ExpectUtf8<T> {
    fn expect_utf8(self, message: &[u8]) -> T;
}

#[cfg(feature = "contract")]
impl<T> ExpectUtf8<T> for Option<T> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Some(t) => t,
            None => sdk::panic_utf8(message),
        }
    }
}

#[cfg(feature = "contract")]
impl<T, E> ExpectUtf8<T> for core::result::Result<T, E> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Ok(t) => t,
            Err(_) => sdk::panic_utf8(message),
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
