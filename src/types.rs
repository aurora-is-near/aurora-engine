use crate::prelude::{self, Address, String, ToString, Vec, H256, U256};
#[cfg(not(feature = "contract"))]
use crate::prelude::{format, vec};

use crate::prelude::str;
use borsh::{BorshDeserialize, BorshSerialize};
use ethabi::{Event, EventParam, Hash, Log, RawLog};

#[cfg(not(feature = "contract"))]
use ethabi::{ParamType, Token};

#[cfg(not(feature = "contract"))]
use sha3::{Digest, Keccak256};

use crate::engine::EngineResult;
use crate::log_entry::LogEntry;
use crate::sdk;

#[cfg(not(feature = "contract"))]
use ethabi::param_type::Writer;

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

    /// Build log_entry_data from ethereum event
    #[cfg(not(feature = "contract"))]
    #[allow(dead_code)]
    pub fn params_to_log_entry_data(
        name: &str,
        params: EventParams,
        locker_address: EthAddress,
        indexes: Vec<Vec<u8>>,
        values: Vec<Token>,
    ) -> Vec<u8> {
        let event = Event {
            name: name.to_string(),
            inputs: params.into_iter().collect(),
            anonymous: false,
        };
        let params: Vec<ParamType> = event.inputs.iter().map(|p| p.kind.clone()).collect();
        let topics = indexes
            .into_iter()
            .map(|value| {
                let mut result: [u8; 32] = Default::default();
                result[12..].copy_from_slice(value.as_slice());
                H256::from(result)
            })
            .collect();
        let log_entry = LogEntry {
            address: locker_address.into(),
            topics: vec![vec![long_signature(&event.name, &params).0.into()], topics].concat(),
            data: ethabi::encode(&values),
        };
        rlp::encode(&log_entry).to_vec()
    }
}

#[cfg(not(feature = "contract"))]
fn long_signature(name: &str, params: &[ParamType]) -> Hash {
    let types = params
        .iter()
        .map(Writer::write)
        .collect::<Vec<String>>()
        .join(",");

    let data: Vec<u8> = From::from(format!("{}({})", name, types).as_str());

    let mut sponge = sha3::Keccak256::default();
    sponge.update(&data);
    let mut result: [u8; 32] = Default::default();
    result.copy_from_slice(sponge.finalize().as_slice());
    H256::from(result)
}

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

    pub fn to_bytes(self) -> [u8; 32] {
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

#[derive(Default)]
pub struct Stack<T> {
    stack: Vec<T>,
    boundaries: Vec<usize>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            boundaries: crate::prelude::vec![0],
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

pub trait SdkExpect<T> {
    fn sdk_expect(self, msg: &str) -> T;
}

impl<T> SdkExpect<T> for Option<T> {
    fn sdk_expect(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => sdk::panic_utf8(msg.as_ref()),
        }
    }
}

impl<T, E> SdkExpect<T> for core::result::Result<T, E> {
    fn sdk_expect(self, msg: &str) -> T {
        match self {
            Ok(t) => t,
            Err(_) => sdk::panic_utf8(msg.as_ref()),
        }
    }
}

pub trait SdkUnwrap<T> {
    fn sdk_unwrap(self) -> T;
}

impl<T> SdkUnwrap<T> for Option<T> {
    fn sdk_unwrap(self) -> T {
        match self {
            Some(t) => t,
            None => sdk::panic_utf8("ERR_UNWRAP".as_bytes()),
        }
    }
}

impl<T, E: AsRef<[u8]>> SdkUnwrap<T> for core::result::Result<T, E> {
    fn sdk_unwrap(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => sdk::panic_utf8(e.as_ref()),
        }
    }
}

pub(crate) trait SdkProcess<T> {
    fn sdk_process(self);
}

impl<T: AsRef<[u8]>> SdkProcess<T> for EngineResult<T> {
    fn sdk_process(self) {
        match self {
            Ok(r) => sdk::return_output(r.as_ref()),
            Err(e) => sdk::panic_utf8(e.as_ref()),
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

    /// Build view of the stack. Intervals between None values are scopes.
    fn view_stack(stack: &Stack<i32>) -> Vec<Option<i32>> {
        let mut res = vec![];
        let mut pnt = 0;

        for &pos in stack.boundaries.iter() {
            while pnt < pos {
                res.push(Some(stack.stack[pnt]));
                pnt += 1;
            }
            res.push(None);
        }

        while pnt < stack.stack.len() {
            res.push(Some(stack.stack[pnt]));
            pnt += 1;
        }

        res
    }

    fn check_stack(stack: &Stack<i32>, expected: Vec<Option<i32>>) {
        if let Some(&last) = stack.boundaries.last() {
            assert!(last <= stack.stack.len());
        }
        assert_eq!(view_stack(stack), expected);
    }

    #[test]
    fn test_stack() {
        let mut stack = Stack::new(); // [ $ ]
        check_stack(&stack, vec![None]);

        stack.push(1); // [ $, 1]
        check_stack(&stack, vec![None, Some(1)]);
        stack.push(2); // [ $, 1, 2 ]
        check_stack(&stack, vec![None, Some(1), Some(2)]);
        stack.enter(); // [$, 1, 2, $]
        check_stack(&stack, vec![None, Some(1), Some(2), None]);
        stack.push(3); // [$, 1, 2, $, 3]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(3)]);
        stack.discard(); // [$, 1, 2]
        check_stack(&stack, vec![None, Some(1), Some(2)]);
        stack.enter();
        check_stack(&stack, vec![None, Some(1), Some(2), None]);
        stack.push(4); // [$, 1, 2, $, 4]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(4)]);
        stack.enter(); // [$, 1, 2, $, 4, $]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(4), None]);
        stack.push(5); // [$, 1, 2, $, 4, $, 5]
        check_stack(
            &stack,
            vec![None, Some(1), Some(2), None, Some(4), None, Some(5)],
        );
        stack.commit(); // [$, 1, 2, $, 4, 5]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(4), Some(5)]);
        stack.discard(); // [$, 1, 2]
        check_stack(&stack, vec![None, Some(1), Some(2)]);
        stack.push(6); // [$, 1, 2, 6]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6)]);
        stack.enter(); // [$, 1, 2, 6, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None]);
        stack.enter(); // [$, 1, 2, 6, $, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None, None]);
        stack.enter(); // [$, 1, 2, 6, $, $, $]
        check_stack(
            &stack,
            vec![None, Some(1), Some(2), Some(6), None, None, None],
        );
        stack.commit(); // [$, 1, 2, 6, $, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None, None]);
        stack.discard(); // [$, 1, 2, 6, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None]);
        stack.push(7); // [$, 1, 2, 6, $, 7]

        assert_eq!(stack.into_vec(), vec![1, 2, 6, 7]);
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
