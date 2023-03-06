use crate::fmt::Formatter;
use crate::{format, Add, Display, Sub, ToString};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    #[must_use]
    pub const fn new(amount: u128) -> Self {
        Self(amount)
    }

    /// Consumes `Balance` and returns the underlying type.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

impl Serialize for Balance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = self.0.to_string();
        serializer.serialize_str(&value)
    }
}

impl<'de> Deserialize<'de> for Balance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
        D::Error: serde::de::Error,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(
            value
                .as_str()
                .ok_or_else(|| Error::custom(format!("Wait for a string but got: {value}")))
                .and_then(|value| value.parse().map_err(Error::custom))?,
        ))
    }
}

#[derive(
    Default,
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
/// Near Yocto type which wraps an underlying u128.
/// 1 NEAR = 10^24 `yoctoNEAR`
pub struct Yocto(u128);

impl Display for Yocto {
    fn fmt(&self, f: &mut Formatter<'_>) -> crate::fmt::Result {
        self.0.fmt(f)
    }
}

impl Yocto {
    /// Constructs a new `Yocto NEAR` with a given u128 value.
    #[must_use]
    pub const fn new(yocto: u128) -> Self {
        Self(yocto)
    }

    /// Consumes `Yocto NEAR` and returns the underlying type.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

impl Add for Yocto {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Yocto {
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
            write!(f, "{msg}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Balance;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    struct SomeStruct {
        balance: Balance,
    }

    #[test]
    fn test_deserialize_balance() {
        let json = r#"{"balance": "340282366920938463463374607431768211455"}"#;
        let result: SomeStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.balance, Balance::new(u128::MAX));

        let json = r#"{"balance": "340282366920938463463374607431768211456"}"#; // Overflow
        let result = serde_json::from_str::<SomeStruct>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_balance() {
        let json = r#"{"balance":"340282366920938463463374607431768211455"}"#;
        let result = SomeStruct {
            balance: Balance::new(340_282_366_920_938_463_463_374_607_431_768_211_455),
        };

        assert_eq!(&serde_json::to_string(&result).unwrap(), json);
    }
}
