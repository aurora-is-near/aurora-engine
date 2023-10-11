use crate::{format, AsBytes, String, H160};
#[cfg(not(feature = "borsh-compat"))]
use borsh::{maybestd::io, BorshDeserialize, BorshSerialize};
#[cfg(feature = "borsh-compat")]
use borsh_compat::{maybestd::io, BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// Base Eth Address type
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Address(H160);

impl Address {
    /// Construct Address from H160
    #[must_use]
    pub const fn new(val: H160) -> Self {
        Self(val)
    }

    /// Get raw H160 data
    #[must_use]
    pub const fn raw(&self) -> H160 {
        self.0
    }

    /// Encode address to string
    #[must_use]
    pub fn encode(&self) -> String {
        hex::encode(self.0.as_bytes())
    }

    pub fn decode(address: &str) -> Result<Self, error::AddressError> {
        if address.len() != 40 {
            return Err(error::AddressError::IncorrectLength);
        }
        let mut result = [0u8; 20];
        hex::decode_to_slice(address, &mut result)
            .map_err(|_| error::AddressError::FailedDecodeHex)?;
        Ok(Self::new(H160(result)))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn try_from_slice(raw_addr: &[u8]) -> Result<Self, error::AddressError> {
        if raw_addr.len() != 20 {
            return Err(error::AddressError::IncorrectLength);
        }
        Ok(Self::new(H160::from_slice(raw_addr)))
    }

    #[must_use]
    pub const fn from_array(array: [u8; 20]) -> Self {
        Self(H160(array))
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::new(H160([0u8; 20]))
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = error::AddressError;

    fn try_from(raw_addr: &[u8]) -> Result<Self, Self::Error> {
        Self::try_from_slice(raw_addr).map_err(|_| error::AddressError::IncorrectLength)
    }
}

impl AsBytes for Address {
    fn as_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl BorshSerialize for Address {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(self.0.as_bytes())
    }
}

#[cfg(not(feature = "borsh-compat"))]
impl BorshDeserialize for Address {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 20];
        let maybe_read = reader.read_exact(&mut buf);
        if maybe_read.as_ref().err().map(io::Error::kind) == Some(io::ErrorKind::UnexpectedEof) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{}", error::AddressError::IncorrectLength),
            ));
        }
        maybe_read?;
        let address = Self(H160(buf));
        Ok(address)
    }
}

#[cfg(feature = "borsh-compat")]
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
        Self::zero()
    }
}

/// fn for making an address by concatenating the bytes from two given numbers,
/// Note that 32 + 128 = 160 = 20 bytes (the length of an address). This function is used
/// as a convenience for specifying the addresses of the various precompiles.
#[must_use]
pub const fn make_address(x: u32, y: u128) -> Address {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    Address::new(H160([
        x_bytes[0],
        x_bytes[1],
        x_bytes[2],
        x_bytes[3],
        y_bytes[0],
        y_bytes[1],
        y_bytes[2],
        y_bytes[3],
        y_bytes[4],
        y_bytes[5],
        y_bytes[6],
        y_bytes[7],
        y_bytes[8],
        y_bytes[9],
        y_bytes[10],
        y_bytes[11],
        y_bytes[12],
        y_bytes[13],
        y_bytes[14],
        y_bytes[15],
    ]))
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
            write!(f, "{msg}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    const fn u8_to_address(x: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = x;
        Address::new(H160(bytes))
    }

    // Inverse function of `super::make_address`.
    fn split_address(a: Address) -> (u32, u128) {
        let mut x_bytes = [0u8; 4];
        let mut y_bytes = [0u8; 16];

        x_bytes.copy_from_slice(&a.raw()[0..4]);
        y_bytes.copy_from_slice(&a.raw()[4..20]);

        (u32::from_be_bytes(x_bytes), u128::from_be_bytes(y_bytes))
    }

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

    #[test]
    fn test_make_address() {
        for i in 0..u8::MAX {
            assert_eq!(make_address(0, i.into()), u8_to_address(i));
        }

        let mut rng = rand::thread_rng();
        for _ in 0..u8::MAX {
            let address = Address::new(H160(rng.gen()));
            let (x, y) = split_address(address);
            assert_eq!(address, make_address(x, y));
        }
    }
}
