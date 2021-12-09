use crate::{str, vec, Add, Address, Display, Div, Mul, String, Sub, Vec, U256};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::fmt::Formatter;

// TODO: introduce new Balance type for more strict typing
pub type Balance = u128;
pub type RawAddress = [u8; 20];
pub type RawU256 = [u8; 32];
// Big-endian large integer type.
pub type RawH256 = [u8; 32]; // Unformatted binary data of fixed length.

// TODO: introduce new type. Add encode/decode/validation methods
pub type EthAddress = [u8; 20];
pub type StorageUsage = u64;
/// Wei compatible Borsh-encoded raw value to attach an ETH balance to the transaction
pub type WeiU256 = [u8; 32];

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
/// Near gas type which wraps an underlying u64.
pub struct NearGas(u64);

impl Sub<NearGas> for NearGas {
    type Output = NearGas;

    fn sub(self, rhs: NearGas) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Display for NearGas {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl NearGas {
    /// Constructs a new `NearGas` with a given u64 value.
    pub const fn new(gas: u64) -> NearGas {
        Self(gas)
    }

    /// Consumes `NearGas` and returns the underlying type.
    pub fn into_u64(self) -> u64 {
        self.0
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
/// Ethereum gas type which wraps an underlying u64.
pub struct EthGas(u64);

impl Display for EthGas {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl EthGas {
    /// Constructs a new `EthGas` with a given u64 value.
    pub const fn new(gas: u64) -> EthGas {
        Self(gas)
    }

    /// Consumes `EthGas` and returns the underlying type.
    pub fn into_u64(self) -> u64 {
        self.0
    }
}

impl Add<EthGas> for EthGas {
    type Output = EthGas;

    fn add(self, rhs: EthGas) -> Self::Output {
        EthGas(self.0 + rhs.0)
    }
}

impl Div<usize> for EthGas {
    type Output = EthGas;

    fn div(self, rhs: usize) -> Self::Output {
        EthGas(self.0 / rhs as u64)
    }
}

impl Mul<EthGas> for u32 {
    type Output = EthGas;

    fn mul(self, rhs: EthGas) -> Self::Output {
        EthGas(self as u64 * rhs.0)
    }
}

impl Mul<u32> for EthGas {
    type Output = EthGas;

    fn mul(self, rhs: u32) -> Self::Output {
        EthGas(self.0 * rhs as u64)
    }
}

impl Mul<usize> for EthGas {
    type Output = EthGas;

    fn mul(self, rhs: usize) -> Self::Output {
        EthGas(self.0 * rhs as u64)
    }
}

impl Mul<EthGas> for u64 {
    type Output = EthGas;

    fn mul(self, rhs: EthGas) -> Self::Output {
        EthGas(self * rhs.0)
    }
}

#[derive(
    Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize,
)]
/// Engine `fee` type which wraps an underlying u128.
pub struct Fee(u128);

impl Display for Fee {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Fee {
    /// Constructs a new `Fee` with a given u128 value.
    pub const fn new(fee: u128) -> Fee {
        Self(fee)
    }

    /// Consumes `Fee` and returns the underlying type.
    pub fn into_u128(self) -> u128 {
        self.0
    }
}

impl Add<Fee> for Fee {
    type Output = Fee;

    fn add(self, rhs: Fee) -> Self::Output {
        Fee(self.0 + rhs.0)
    }
}

impl From<u128> for Fee {
    fn from(fee: u128) -> Self {
        Self(fee)
    }
}

/// Selector to call mint function in ERC 20 contract
///
/// keccak("mint(address,uint256)".as_bytes())[..4];
#[allow(dead_code)]
pub const ERC20_MINT_SELECTOR: &[u8] = &[64, 193, 15, 25];

#[derive(Debug)]
pub enum AddressValidationError {
    FailedDecodeHex,
    IncorrectLength,
}

impl AsRef<[u8]> for AddressValidationError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::FailedDecodeHex => b"FAILED_DECODE_ETH_ADDRESS",
            Self::IncorrectLength => b"ETH_WRONG_ADDRESS_LENGTH",
        }
    }
}

/// Validate Ethereum address from string and return Result data EthAddress or Error data
pub fn validate_eth_address(address: String) -> Result<EthAddress, AddressValidationError> {
    let data = hex::decode(address).map_err(|_| AddressValidationError::FailedDecodeHex)?;
    if data.len() != 20 {
        return Err(AddressValidationError::IncorrectLength);
    }
    let mut result = [0u8; 20];
    result.copy_from_slice(&data);
    Ok(result)
}

/// Newtype to distinguish balances (denominated in Wei) from other U256 types.
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Wei(U256);

impl Wei {
    const ETH_TO_WEI: U256 = U256([1_000_000_000_000_000_000, 0, 0, 0]);

    pub const fn zero() -> Self {
        Self(U256([0, 0, 0, 0]))
    }

    pub const fn new(amount: U256) -> Self {
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

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// Try convert U256 to u128 with checking overflow.
    /// NOTICE: Error can contain only overflow
    pub fn try_into_u128(self) -> Result<u128, error::BalanceOverflowError> {
        use crate::TryInto;
        self.0.try_into().map_err(|_| error::BalanceOverflowError)
    }
}

impl Display for Wei {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Add<Self> for Wei {
    type Output = Wei;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Self> for Wei {
    type Output = Wei;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

/// Type casting from Wei compatible Borsh-encoded raw value into the Wei value, to attach an ETH balance to the transaction
impl From<WeiU256> for Wei {
    fn from(value: WeiU256) -> Self {
        Wei(U256::from_big_endian(&value))
    }
}

pub const STORAGE_PRICE_PER_BYTE: u128 = 10_000_000_000_000_000_000;
// 1e19yN, 0.00001N
pub const ERR_FAILED_PARSE: &str = "ERR_FAILED_PARSE";
pub const ERR_INVALID_ETH_ADDRESS: &str = "ERR_INVALID_ETH_ADDRESS";

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
#[derive(Debug, Clone, PartialEq, Eq)]
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
            boundaries: vec![0],
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

pub mod error {
    use crate::{fmt, String};

    #[derive(Eq, Hash, Clone, Debug, PartialEq)]
    pub struct BalanceOverflowError;

    impl AsRef<[u8]> for BalanceOverflowError {
        fn as_ref(&self) -> &[u8] {
            b"ERR_BALANCE_OVERFLOW"
        }
    }

    impl fmt::Display for BalanceOverflowError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let msg = String::from_utf8(self.as_ref().to_vec()).unwrap();
            write!(f, "{}", msg)
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

    #[test]
    fn test_fee_add() {
        let fee = Fee::new(100);
        assert_eq!(fee + fee, Fee::new(200));
        assert_eq!(fee.add(200.into()), Fee::new(300));
    }

    #[test]
    fn test_fee_from() {
        let fee = Fee::new(100);
        let fee2 = Fee::from(100u128);
        assert_eq!(fee, fee2);
        let res: u128 = fee.into_u128();
        assert_eq!(res, 100);
    }
}
