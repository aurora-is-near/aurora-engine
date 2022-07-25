use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, BorshDeserialize, BorshSerialize, Hash)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}
