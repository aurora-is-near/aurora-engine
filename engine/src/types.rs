use borsh::{BorshDeserialize, BorshSerialize};
use prelude::{self, str, Address, String, ToString, Vec, H256, U256};

#[cfg(not(feature = "contract"))]
use sha3::{Digest, Keccak256};

pub type AccountId = String;
pub type Balance = u128;
pub type RawAddress = [u8; 20];
pub type RawU256 = [u8; 32]; // Big-endian large integer type.
pub type RawH256 = [u8; 32]; // Unformatted binary data of fixed length.
pub type EthAddress = [u8; 20];
pub type Gas = u64;
pub type StorageUsage = u64;

/// Selector to call mint function in ERC 20 contract
///
/// keccak("mint(address,uint256)".as_bytes())[..4];
#[allow(dead_code)]
pub(crate) const ERC20_MINT_SELECTOR: &[u8] = &[64, 193, 15, 25];

#[derive(Debug)]
pub enum ValidationError {
    EthAddressFailedDecode,
    WrongEthAddress,
}

impl AsRef<[u8]> for ValidationError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::EthAddressFailedDecode => b"FAILED_DECODE_ETH_ADDRESS",
            Self::WrongEthAddress => b"WRONG_ETH_ADDRESS",
        }
    }
}

/// Validate Etherium address from string and return EthAddress
pub fn validate_eth_address(address: String) -> Result<EthAddress, ValidationError> {
    let data = hex::decode(address).map_err(|_| ValidationError::EthAddressFailedDecode)?;
    if data.len() != 20 {
        return Err(ValidationError::WrongEthAddress);
    }
    assert_eq!(data.len(), 20, "ETH_WRONG_ADDRESS_LENGTH");
    let mut result = [0u8; 20];
    result.copy_from_slice(&data);
    Ok(result)
}

#[derive(Default, BorshDeserialize, BorshSerialize, Clone)]
#[cfg_attr(test, derive(serde::Deserialize, serde::Serialize))]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
}

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

/// Newtype to distinguish balances (denominated in Wei) from other U256 types.
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Copy, Clone, Default)]
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

    pub fn to_bytes(self) -> [u8; 32] {
        u256_to_arr(&self.0)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn raw(self) -> U256 {
        self.0
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
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
pub enum PromiseResult {
    NotReady,
    Successful(Vec<u8>),
    Failed,
}

/// ft_resolve_transfer result of eth-connector
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

#[derive(Default)]
pub struct Stack<T> {
    stack: Vec<T>,
    boundaries: Vec<usize>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            boundaries: prelude::vec![0],
        }
    }

    pub fn enter(&mut self) {
        self.boundaries.push(self.stack.len());
    }

    pub fn commit(&mut self) {
        self.boundaries.pop().unwrap();
    }

    pub fn discard(&mut self) {
        let boundary = self.boundaries.pop().unwrap();
        self.stack.truncate(boundary);
    }

    pub fn push(&mut self, value: T) {
        self.stack.push(value);
    }

    pub fn into_vec(self) -> Vec<T> {
        self.stack
    }
}
pub fn str_from_slice(inp: &[u8]) -> &str {
    str::from_utf8(inp).unwrap()
}

#[cfg(feature = "contract")]
pub trait ExpectUtf8<T> {
    fn expect_utf8(self, message: &[u8]) -> T;
}
