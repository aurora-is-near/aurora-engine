use crate::fmt::Formatter;
use crate::{Add, Display, Sub};
use borsh::{BorshDeserialize, BorshSerialize};

pub const ZERO_BALANCE: Balance = Balance::new(0);
pub const ZERO_YOCTO: Yocto = Yocto::new(0);

#[derive(
    Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize,
)]
/// A generic type for 128-bit balances, especially for NEP-141 tokens. This generic type should not be used
/// to represent NEAR balances (`Yocto` is designed for this purpose) or for eth-connector balances (`NEP141Wei`
/// is designed for this purpose). The reason we have specific types for NEAR and eth-connector is because of the
/// significant role they play in our system; therefore we do not want to mix them up with generic token balances.
pub struct Balance(u128);

impl Display for Balance {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Balance {
    /// Constructs a new `Balance` with a given u128 value.
    pub const fn new(amount: u128) -> Balance {
        Self(amount)
    }

    /// Consumes `Balance` and returns the underlying type.
    pub fn as_u128(self) -> u128 {
        self.0
    }
}

#[derive(
    Default, BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd,
)]
/// Near Yocto type which wraps an underlying u128.
/// 1 NEAR = 10^24 yoctoNEAR
pub struct Yocto(u128);

impl Display for Yocto {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Yocto {
    /// Constructs a new `Yocto NEAR` with a given u128 value.
    pub const fn new(yocto: u128) -> Yocto {
        Self(yocto)
    }

    /// Consumes `Yocto NEAR` and returns the underlying type.
    pub fn as_u128(self) -> u128 {
        self.0
    }
}

impl Add<Yocto> for Yocto {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Yocto> for Yocto {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
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
