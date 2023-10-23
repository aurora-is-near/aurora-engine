use crate::fmt::Formatter;
use crate::types::Wei;
use crate::{Add, AddAssign, Display, Div, Mul, Sub};
#[cfg(not(feature = "borsh-compat"))]
use borsh::{BorshDeserialize, BorshSerialize};
#[cfg(feature = "borsh-compat")]
use borsh_compat::{self as borsh, BorshDeserialize, BorshSerialize};
use core::num::NonZeroU64;
use primitive_types::U256;
use serde::{Deserialize, Serialize};

#[derive(
    Default, BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd,
)]
/// Near gas type which wraps an underlying u64.
pub struct NearGas(u64);

impl Display for NearGas {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Sub for NearGas {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Add for NearGas {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl NearGas {
    /// Constructs a new `NearGas` with a given u64 value.
    #[must_use]
    pub const fn new(gas: u64) -> Self {
        Self(gas)
    }

    /// Consumes `NearGas` and returns the underlying type.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
)]
/// Ethereum gas type which wraps an underlying u64.
pub struct EthGas(u64);

impl Display for EthGas {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl EthGas {
    /// Constructs a new `EthGas` from a value of type `u64`.
    #[must_use]
    pub const fn new(gas: u64) -> Self {
        Self(gas)
    }

    /// Convert `EthGas` to `u64` type.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Convert `EthGas` to `U256` type.
    #[must_use]
    pub fn as_u256(self) -> U256 {
        self.as_u64().into()
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        self.0.checked_mul(rhs.0).map(Self)
    }
}

impl Add for EthGas {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for EthGas {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Div<NonZeroU64> for EthGas {
    type Output = Self;

    fn div(self, rhs: NonZeroU64) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Mul<EthGas> for u32 {
    type Output = EthGas;

    fn mul(self, rhs: EthGas) -> Self::Output {
        EthGas(u64::from(self) * rhs.0)
    }
}

impl Mul<u32> for EthGas {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * u64::from(rhs))
    }
}

impl Mul<u64> for EthGas {
    type Output = Self;

    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Mul<EthGas> for u64 {
    type Output = EthGas;

    fn mul(self, rhs: EthGas) -> Self::Output {
        EthGas(self * rhs.0)
    }
}

impl Mul<Wei> for EthGas {
    type Output = Wei;

    fn mul(self, rhs: Wei) -> Self::Output {
        Wei::new(self.as_u256() * rhs.raw())
    }
}
