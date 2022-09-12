use crate::fmt::Formatter;
use crate::types::balance::error;
use crate::types::Fee;
use crate::{Add, Display, Sub, SubAssign, U256};
use borsh::{BorshDeserialize, BorshSerialize};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub const ZERO_NEP141_WEI: NEP141Wei = NEP141Wei::new(0);
pub const ZERO_WEI: Wei = Wei::new_u64(0);

/// Wei compatible Borsh-encoded raw value to attach an ETH balance to the transaction
pub type WeiU256 = [u8; 32];

// Type representing the NEP-141 balances of the eth-connector (ie Wei amounts that have been bridged to Near)
#[derive(
    Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize,
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NEP141Wei(u128);

impl Display for NEP141Wei {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl NEP141Wei {
    /// Constructs a new `NEP141Wei` with a given u128 value.
    pub const fn new(amount: u128) -> Self {
        Self(amount)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// Consumes `NEP141Wei` and returns the underlying type.
    pub fn as_u128(self) -> u128 {
        self.0
    }
}

impl Sub<NEP141Wei> for NEP141Wei {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Add<NEP141Wei> for NEP141Wei {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl SubAssign<NEP141Wei> for NEP141Wei {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

/// Newtype to distinguish balances (denominated in Wei) from other U256 types.
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

impl From<Fee> for Wei {
    fn from(value: Fee) -> Self {
        Wei(U256::from(value.as_u128()))
    }
}

impl From<NEP141Wei> for Wei {
    fn from(value: NEP141Wei) -> Self {
        Wei(U256::from(value.as_u128()))
    }
}

#[allow(dead_code)]
pub fn u256_to_arr(value: &U256) -> [u8; 32] {
    let mut result = [0u8; 32];
    value.to_big_endian(&mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wei_from_eth() {
        let eth_amount: u64 = rand::random();
        let wei_amount = U256::from(eth_amount) * U256::from(10).pow(18.into());
        assert_eq!(Wei::from_eth(eth_amount.into()), Some(Wei::new(wei_amount)));
    }

    #[test]
    fn test_wei_from_u64() {
        let x: u64 = rand::random();
        assert_eq!(Wei::new_u64(x).raw().as_u64(), x);
    }
}
