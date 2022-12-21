use crate::{format, String, H160};
use borsh::maybestd::io;
use borsh::{BorshDeserialize, BorshSerialize};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Base Eth Address type
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Address(H160);

impl Address {
    /// Construct Address from H160
    pub const fn new(val: H160) -> Self {
        Self(val)
    }

    /// Get raw H160 data
    pub fn raw(&self) -> H160 {
        self.0
    }

    /// Encode address to string
    pub fn encode(&self) -> String {
        hex::encode(self.0.as_bytes())
    }

    pub fn decode(address: &str) -> Result<Address, error::AddressError> {
        if address.len() != 40 {
            return Err(error::AddressError::IncorrectLength);
        }
        let mut result = [0u8; 20];
        hex::decode_to_slice(address, &mut result)
            .map_err(|_| error::AddressError::FailedDecodeHex)?;
        Ok(Address::new(H160(result)))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn try_from_slice(raw_addr: &[u8]) -> Result<Self, error::AddressError> {
        if raw_addr.len() != 20 {
            return Err(error::AddressError::IncorrectLength);
        }
        Ok(Self::new(H160::from_slice(raw_addr)))
    }

    pub const fn from_array(array: [u8; 20]) -> Self {
        Self(H160(array))
    }

    pub const fn zero() -> Self {
        Address::new(H160([0u8; 20]))
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = error::AddressError;

    fn try_from(raw_addr: &[u8]) -> Result<Self, Self::Error> {
        Self::try_from_slice(raw_addr).map_err(|_| error::AddressError::IncorrectLength)
    }
}

impl BorshSerialize for Address {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(self.0.as_bytes())
    }
}

impl BorshDeserialize for Address {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        if buf.len() < 20 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{}", error::AddressError::IncorrectLength),
            ));
        }
        // Guaranty no panics. The length checked early
        let address = Self(H160::from_slice(&buf[..20]));
        *buf = &buf[20..];
        Ok(address)
    }
}

impl Default for Address {
    fn default() -> Self {
        Address::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_serializer() {
        let eth_address = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";
        // borsh serialize
        let serialized_addr =
            Address::new(H160::from_slice(&hex::decode(eth_address).unwrap()[..]))
                .try_to_vec()
                .unwrap();
        assert_eq!(serialized_addr.len(), 20);

        let addr = Address::try_from_slice(&serialized_addr).unwrap();
        assert_eq!(
            addr.encode(),
            "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E".to_lowercase()
        );
    }

    #[test]
    fn test_address_decode() {
        // Test compatibility with previous typ RawAddress.
        // It was: type RawAddress = [u8;20];
        let eth_address_vec = hex::decode("096DE9C2B8A5B8c22cEe3289B101f6960d68E51E").unwrap();
        let mut eth_address = [0u8; 20];
        eth_address.copy_from_slice(&eth_address_vec[..]);

        let aurora_eth_address =
            Address::decode("096DE9C2B8A5B8c22cEe3289B101f6960d68E51E").unwrap();
        assert_eq!(eth_address, aurora_eth_address.as_bytes());

        let serialized_addr = eth_address.try_to_vec().unwrap();
        let aurora_serialized_addr = aurora_eth_address.try_to_vec().unwrap();

        assert_eq!(serialized_addr.len(), 20);
        assert_eq!(aurora_serialized_addr.len(), 20);
        assert_eq!(serialized_addr, aurora_serialized_addr);

        // Used serialized data from `RawAddress`
        let addr = Address::try_from_slice(&serialized_addr).unwrap();
        assert_eq!(
            addr.encode(),
            "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E".to_lowercase()
        );
    }

    #[test]
    fn test_wrong_address_19() {
        let serialized_addr = [0u8; 19];
        let addr = Address::try_from_slice(&serialized_addr);
        let err = addr.unwrap_err();
        matches!(err, error::AddressError::IncorrectLength);
    }
}

pub mod error {
    use crate::{fmt, String};

    #[derive(Eq, Hash, Clone, Debug, PartialEq)]
    pub enum AddressError {
        FailedDecodeHex,
        IncorrectLength,
    }

    impl AsRef<[u8]> for AddressError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::FailedDecodeHex => b"FAILED_DECODE_ETH_ADDRESS",
                Self::IncorrectLength => b"ETH_WRONG_ADDRESS_LENGTH",
            }
        }
    }

    impl fmt::Display for AddressError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let msg = String::from_utf8(self.as_ref().to_vec()).unwrap();
            write!(f, "{}", msg)
        }
    }
}
