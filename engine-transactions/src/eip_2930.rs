use crate::Error;
use aurora_engine_precompiles::secp256k1::ecrecover;
use aurora_engine_sdk as sdk;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{Vec, H160, H256, U256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Type indicator (per EIP-2718) for access list transactions
pub const TYPE_BYTE: u8 = 0x01;

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AccessTuple {
    pub address: H160,
    pub storage_keys: Vec<H256>,
}

impl Decodable for AccessTuple {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        let address = rlp.val_at(0)?;
        let storage_keys = rlp.list_at(1)?;

        Ok(Self {
            address,
            storage_keys,
        })
    }
}

/// See https://eips.ethereum.org/EIPS/eip-2930
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Transaction2930 {
    pub chain_id: u64,
    pub nonce: U256,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub to: Option<Address>,
    pub value: Wei,
    pub data: Vec<u8>,
    pub access_list: Vec<AccessTuple>,
}

impl Transaction2930 {
    /// RLP encoding of the data for an unsigned message (used to make signature)
    pub fn rlp_append_unsigned(&self, s: &mut RlpStream) {
        self.rlp_append(s, 8);
    }

    /// RLP encoding for a signed message (used to encode the transaction for sending to tx pool)
    pub fn rlp_append_signed(&self, s: &mut RlpStream) {
        self.rlp_append(s, 11);
    }

    fn rlp_append(&self, s: &mut RlpStream, list_len: usize) {
        s.begin_list(list_len);
        s.append(&self.chain_id);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas_limit);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(address) => s.append(&address.raw()),
        };
        s.append(&self.value.raw());
        s.append(&self.data);
        s.begin_list(self.access_list.len());
        for tuple in self.access_list.iter() {
            s.begin_list(2);
            s.append(&tuple.address);
            s.begin_list(tuple.storage_keys.len());
            for key in tuple.storage_keys.iter() {
                s.append(key);
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct SignedTransaction2930 {
    pub transaction: Transaction2930,
    /// The parity (0 for even, 1 for odd) of the y-value of a secp256k1 signature.
    pub parity: u8,
    pub r: U256,
    pub s: U256,
}

impl SignedTransaction2930 {
    pub fn sender(&self) -> Result<Address, Error> {
        let mut rlp_stream = RlpStream::new();
        rlp_stream.append(&TYPE_BYTE);
        self.transaction.rlp_append_unsigned(&mut rlp_stream);
        let message_hash = sdk::keccak(rlp_stream.as_raw());
        ecrecover(
            message_hash,
            &super::vrs_to_arr(self.parity, self.r, self.s),
        )
        .map_err(|_e| Error::EcRecover)
    }
}

impl Encodable for SignedTransaction2930 {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.transaction.rlp_append_signed(s);
        s.append(&self.parity);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Decodable for SignedTransaction2930 {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        if rlp.item_count() != Ok(11) {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }
        let chain_id = rlp.val_at(0)?;
        let nonce = rlp.val_at(1)?;
        let gas_price = rlp.val_at(2)?;
        let gas_limit = rlp.val_at(3)?;
        let to = super::rlp_extract_to(rlp, 4)?;
        let value = Wei::new(rlp.val_at(5)?);
        let data = rlp.val_at(6)?;
        let access_list = rlp.list_at(7)?;
        let parity = rlp.val_at(8)?;
        let r = rlp.val_at(9)?;
        let s = rlp.val_at(10)?;
        Ok(Self {
            transaction: Transaction2930 {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to,
                value,
                data,
                access_list,
            },
            parity,
            r,
            s,
        })
    }
}
