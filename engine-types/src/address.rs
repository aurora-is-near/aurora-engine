use crate::{String, H160};
use borsh::maybestd::io;
use borsh::{BorshDeserialize, BorshSerialize};

/// Base Eth Address type
pub struct Address(H160);

impl Address {
    /// Construct Address from H160
    pub fn new(val: H160) -> Self {
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
}

impl BorshSerialize for Address {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(self.0.as_bytes())
    }
}

impl BorshDeserialize for Address {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        Ok(Self(H160::from_slice(buf)))
    }

    fn try_from_slice(v: &[u8]) -> io::Result<Self> {
        let mut v_mut = v;
        Self::deserialize(&mut v_mut)
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
    #[should_panic]
    fn test_wrong_address_19() {
        let serialized_addr = [0u8; 19];
        let addr = Address::try_from_slice(&serialized_addr);
        assert!(addr.is_err());

        let serialized_addr = [0u8; 21];
        let _ = Address::try_from_slice(&serialized_addr);
    }

    #[test]
    #[should_panic]
    fn test_wrong_address_21() {
        let serialized_addr = [0u8; 21];
        let _ = Address::try_from_slice(&serialized_addr);
    }
}
