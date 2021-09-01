use crate::prelude::sdk;
use borsh::{BorshDeserialize, BorshSerialize};
use prelude::{self, String, ToString, Vec};

#[derive(Default, BorshDeserialize, BorshSerialize, Clone)]
#[cfg_attr(test, derive(serde::Deserialize, serde::Serialize))]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
}

impl Proof {
    pub fn get_key(&self) -> String {
        let mut data = self.log_index.try_to_vec().unwrap();
        data.extend(self.receipt_index.try_to_vec().unwrap());
        data.extend(self.header_data.clone());
        sdk::sha256(&data[..])
            .0
            .iter()
            .map(|n| n.to_string())
            .collect()
    }
}
