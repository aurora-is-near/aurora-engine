use crate::fmt::Formatter;
use crate::{Add, Display, Div, Mul, Sub};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(
    Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize,
)]
/// Engine `balance` type which wraps an underlying u128.
pub struct Balance(u128);

impl Display for Balance {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Balance {
    /// Constructs a new `Fee` with a given u128 value.
    pub const fn new(amount: u128) -> Balance {
        Self(amount)
    }

    /// Consumes `Fee` and returns the underlying type.
    pub fn into_u128(self) -> u128 {
        self.0
    }
}

impl Add<Balance> for Balance {
    type Output = Balance;

    fn add(self, rhs: Balance) -> Self::Output {
        Balance(self.0 + rhs.0)
    }
}

impl Add<Balance> for u128 {
    type Output = Balance;

    fn add(self, rhs: Balance) -> Self::Output {
        Balance(self + rhs.0)
    }
}

impl Add<u128> for Balance {
    type Output = Balance;

    fn add(self, rhs: u128) -> Self::Output {
        Balance(self.0 + rhs)
    }
}

impl Sub<Balance> for Balance {
    type Output = Balance;

    fn sub(self, rhs: Balance) -> Self::Output {
        Balance(self.0 - rhs.0)
    }
}

impl Sub<Balance> for u128 {
    type Output = Balance;

    fn sub(self, rhs: Balance) -> Self::Output {
        Balance(self - rhs.0)
    }
}

impl Sub<u128> for Balance {
    type Output = Balance;

    fn sub(self, rhs: u128) -> Self::Output {
        Balance(self.0 - rhs)
    }
}

impl Mul<Balance> for Balance {
    type Output = Balance;

    fn mul(self, rhs: Balance) -> Self::Output {
        Balance(self.0 * rhs.0)
    }
}

impl Mul<Balance> for u128 {
    type Output = Balance;

    fn mul(self, rhs: Balance) -> Self::Output {
        Balance(self * rhs.0)
    }
}

impl Mul<u128> for Balance {
    type Output = Balance;

    fn mul(self, rhs: u128) -> Self::Output {
        Balance(self.0 * rhs)
    }
}

impl Div<Balance> for Balance {
    type Output = Balance;

    fn div(self, rhs: Balance) -> Self::Output {
        Balance(self.0 / rhs.0)
    }
}

impl Div<Balance> for u128 {
    type Output = Balance;

    fn div(self, rhs: Balance) -> Self::Output {
        Balance(self / rhs.0)
    }
}

impl Div<u128> for Balance {
    type Output = Balance;

    fn div(self, rhs: u128) -> Self::Output {
        Balance(self.0 / rhs)
    }
}

impl From<u128> for Balance {
    fn from(amount: u128) -> Self {
        Self(amount)
    }
}

impl From<Balance> for u128 {
    fn from(amount: Balance) -> Self {
        amount.0
    }
}
