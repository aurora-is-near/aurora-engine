use crate::prelude::{self, Address, String, Vec, H256, U256};
#[cfg(feature = "engine")]
use alloc::str;
use borsh::{BorshDeserialize, BorshSerialize};

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

/// Newtype to distinguish balances (denominated in Wei) from other U256 types.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct Wei(U256);
impl Wei {
    const ETH_TO_WEI: U256 = U256([1_000_000_000_000_000_000, 0, 0, 0]);

    pub const fn zero() -> Self {
        Self(U256([0, 0, 0, 0]))
    }

    pub fn new(amount: U256) -> Self {
        Self(amount)
    }

    // Purposely not implementing `From<u64>` because I want the call site to always
    // say `Wei::<something>`. If `From` is implemented then the caller might write
    // `amount.into()` without thinking too hard about the units. Explicitly writing
    // `Wei` reminds the developer to think about whether the amount they enter is really
    // in units of `Wei` or not.
    pub const fn new_u64(amount: u64) -> Self {
        Self(U256([amount, 0, 0, 0]))
    }

    pub fn from_eth(amount: U256) -> Option<Self> {
        amount.checked_mul(Self::ETH_TO_WEI).map(Self)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        u256_to_arr(&self.0)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn raw(self) -> U256 {
        self.0
    }
}
impl prelude::Sub for Wei {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self(self.0 - other.0)
    }
}
impl prelude::Add for Wei {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self(self.0 + other.0)
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct U128(pub u128);

pub const STORAGE_PRICE_PER_BYTE: u128 = 10_000_000_000_000_000_000; // 1e19yN, 0.00001N
pub const ERR_FAILED_PARSE: &str = "ERR_FAILED_PARSE";

/// Internal args format for meta call.
#[derive(Debug)]
pub struct InternalMetaCallArgs {
    pub sender: Address,
    pub nonce: U256,
    pub fee_amount: Wei,
    pub fee_address: Address,
    pub contract_address: Address,
    pub value: Wei,
    pub input: Vec<u8>,
}

pub struct StorageBalanceBounds {
    pub min: Balance,
    pub max: Option<Balance>,
}

/// promise results structure
#[cfg(feature = "engine")]
pub enum PromiseResult {
    NotReady,
    Successful(Vec<u8>),
    Failed,
}

/// ft_resolve_transfer result of eth-connector
#[cfg(feature = "engine")]
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

#[cfg(feature = "engine")]
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

    #[test]
    fn test_wei_from_u64() {
        let x: u64 = rand::random();
        assert_eq!(Wei::new_u64(x).raw().as_u64(), x);
    }

    #[test]
    fn test_wei_from_eth() {
        let eth_amount: u64 = rand::random();
        let wei_amount = U256::from(eth_amount) * U256::from(10).pow(18.into());
        assert_eq!(Wei::from_eth(eth_amount.into()), Some(Wei::new(wei_amount)));
    }
}
