use crate::fmt::Formatter;
use crate::{Add, Display, Div, Mul};
use borsh::{BorshDeserialize, BorshSerialize};

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

impl Add<Fee> for u128 {
    type Output = Fee;

    fn add(self, rhs: Fee) -> Self::Output {
        Fee(self + rhs.0)
    }
}

impl Add<u128> for Fee {
    type Output = Fee;

    fn add(self, rhs: u128) -> Self::Output {
        Fee(self.0 + rhs)
    }
}

impl Mul<Fee> for Fee {
    type Output = Fee;

    fn mul(self, rhs: Fee) -> Self::Output {
        Fee(self.0 * rhs.0)
    }
}

impl Mul<Fee> for u128 {
    type Output = Fee;

    fn mul(self, rhs: Fee) -> Self::Output {
        Fee(self * rhs.0)
    }
}

impl Mul<u128> for Fee {
    type Output = Fee;

    fn mul(self, rhs: u128) -> Self::Output {
        Fee(self.0 * rhs)
    }
}

impl Div<Fee> for Fee {
    type Output = Fee;

    fn div(self, rhs: Fee) -> Self::Output {
        Fee(self.0 / rhs.0)
    }
}

impl Div<Fee> for u128 {
    type Output = Fee;

    fn div(self, rhs: Fee) -> Self::Output {
        Fee(self / rhs.0)
    }
}

impl Div<u128> for Fee {
    type Output = Fee;

    fn div(self, rhs: u128) -> Self::Output {
        Fee(self.0 / rhs)
    }
}

impl From<u128> for Fee {
    fn from(fee: u128) -> Self {
        Self(fee)
    }
}

impl From<Fee> for u128 {
    fn from(fee: Fee) -> Self {
        fee.0
    }
}
