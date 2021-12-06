use crate::fmt::Formatter;
use crate::types::balance::error;
use crate::{Add, Display, Sub, U256};

/// Wei compatible Borsh-encoded raw value to attach an ETH balance to the transaction
pub type WeiU256 = [u8; 32];

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

#[allow(dead_code)]
pub fn u256_to_arr(value: &U256) -> [u8; 32] {
    let mut result = [0u8; 32];
    value.to_big_endian(&mut result);
    result
}

#[cfg(test)]
mod tests {
    use crate::*;

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
