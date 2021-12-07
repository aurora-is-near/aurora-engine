use crate::fmt::Formatter;
use crate::{Add, Display, Div, Mul, Sub, SubAssign};
use borsh::{BorshDeserialize, BorshSerialize};

pub const ZERO_BALANCE: Balance = Balance::new(0);

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

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
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

impl Sub<Balance> for Balance {
    type Output = Balance;

    fn sub(self, rhs: Balance) -> Self::Output {
        Balance(self.0 - rhs.0)
    }
}

impl SubAssign<Balance> for Balance {
    fn sub_assign(&mut self, rhs: Balance) {
        *self = *self - rhs;
    }
}

impl Mul<Balance> for Balance {
    type Output = Balance;

    fn mul(self, rhs: Balance) -> Self::Output {
        Balance(self.0 * rhs.0)
    }
}

impl Div<Balance> for Balance {
    type Output = Balance;

    fn div(self, rhs: Balance) -> Self::Output {
        Balance(self.0 / rhs.0)
    }
}

impl From<u128> for Balance {
    fn from(amount: u128) -> Self {
        Self(amount)
    }
}

impl From<u64> for Balance {
    fn from(amount: u64) -> Self {
        Self(amount as u128)
    }
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
