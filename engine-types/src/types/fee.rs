use crate::fmt::Formatter;
use crate::types::NEP141Wei;
use crate::{Add, Display};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(
    Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize,
)]
/// Engine `fee` type which wraps an underlying u128.
pub struct Fee(NEP141Wei);

impl Display for Fee {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Fee {
    /// Constructs a new `Fee` with a given u128 value.
    pub const fn new(fee: NEP141Wei) -> Fee {
        Self(fee)
    }

    /// Consumes `Fee` and returns the underlying type.
    pub fn as_u128(self) -> u128 {
        self.0.as_u128()
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
        Self(NEP141Wei::new(fee))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_add() {
        let fee = Fee::new(NEP141Wei::new(100));
        assert_eq!(fee + fee, Fee::from(200));
        assert_eq!(fee.add(200.into()), Fee::from(300));
    }

    #[test]
    fn test_fee_from() {
        let fee = Fee::new(NEP141Wei::new(100));
        let fee2 = Fee::from(100u128);
        assert_eq!(fee, fee2);
        let res: u128 = fee.as_u128();
        assert_eq!(res, 100);
    }
}
