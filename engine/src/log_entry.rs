use crate::prelude::{Vec, H160, H256};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub address: H160,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
}

impl rlp::Decodable for LogEntry {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let result = LogEntry {
            address: rlp.val_at(0usize)?,
            topics: rlp.list_at(1usize)?,
            data: rlp.val_at(2usize)?,
        };
        Ok(result)
    }
}

impl rlp::Encodable for LogEntry {
    fn rlp_append(&self, stream: &mut rlp::RlpStream) {
        stream.begin_list(3usize);
        stream.append(&self.address);
        stream.append_list::<H256, _>(&self.topics);
        stream.append(&self.data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlp::{Decodable, Encodable, Rlp, RlpStream};

    #[test]
    fn test_roundtrip_rlp_encoding() {
        let address = H160::from_low_u64_le(32u64);
        let topics = vec![H256::zero()];
        let data = vec![0u8, 1u8, 2u8, 3u8];
        let expected_log_entry = LogEntry {
            address,
            topics,
            data,
        };

        let mut stream = RlpStream::new();

        expected_log_entry.rlp_append(&mut stream);

        let bytes = stream.out();
        let rlp = Rlp::new(bytes.as_ref());
        let actual_log_entry = LogEntry::decode(&rlp).unwrap();

        assert_eq!(expected_log_entry, actual_log_entry);
    }
}
