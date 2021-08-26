use prelude::types::Wei;
use prelude::{Address, Vec, H256, U256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

/// Type indicator (per EIP-2718) for access list transactions
pub const TYPE_BYTE: u8 = 0x01;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct AccessTuple {
    pub address: Address,
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
pub struct AccessListEthTransaction {
    pub chain_id: u64,
    pub nonce: U256,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub to: Option<Address>,
    pub value: Wei,
    pub data: Vec<u8>,
    pub access_list: Vec<AccessTuple>,
}

impl AccessListEthTransaction {
    /// RLP encoding of the data for an unsigned message (used to make signature)
    pub fn rlp_append_unsigned(&self, s: &mut RlpStream) {
        self.rlp_append(s, 8);
    }

    /// RLP encoding for a signed message (used to encode the transaction for sending to tx pool)
    pub fn rlp_append_signed(&self, s: &mut RlpStream) {
        self.rlp_append(s, 11);
    }

    #[inline]
    pub fn intrinsic_gas(&self, config: &evm::Config) -> Option<u64> {
        super::intrinsic_gas(self.to.is_none(), &self.data, &self.access_list, config)
    }

    fn rlp_append(&self, s: &mut RlpStream, list_len: usize) {
        s.begin_list(list_len);
        s.append(&self.chain_id);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas_limit);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(address) => s.append(address),
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

#[derive(Debug, Eq, PartialEq)]
pub struct AccessListEthSignedTransaction {
    pub transaction_data: AccessListEthTransaction,
    /// The parity (0 for even, 1 for odd) of the y-value of a secp256k1 signature.
    pub parity: u8,
    pub r: U256,
    pub s: U256,
}

impl AccessListEthSignedTransaction {
    pub fn sender(&self) -> Option<Address> {
        let mut rlp_stream = RlpStream::new();
        rlp_stream.append(&TYPE_BYTE);
        self.transaction_data.rlp_append_unsigned(&mut rlp_stream);
        let message_hash = sdk::keccak(rlp_stream.as_raw());
        crate::precompiles::ecrecover(
            message_hash,
            &super::vrs_to_arr(self.parity, self.r, self.s),
        )
        .ok()
    }
}

impl Encodable for AccessListEthSignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.transaction_data.rlp_append_signed(s);
        s.append(&self.parity);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Decodable for AccessListEthSignedTransaction {
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
            transaction_data: AccessListEthTransaction {
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
