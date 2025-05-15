use crate::eip_2930::AccessTuple;
use crate::Error;
use aurora_engine_sdk as sdk;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{Vec, U256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

/// Type indicator (per EIP-1559)
pub const TYPE_BYTE: u8 = 0x02;

/// EIP-1559 transaction kind from the London hard fork.
///
/// See [EIP-1559](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1559.md)
/// for more details.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Transaction1559 {
    /// ID of chain which the transaction belongs.
    pub chain_id: u64,
    /// A monotonically increasing transaction counter for this sender
    pub nonce: U256,
    /// Determined by the sender and is optional. Priority Fee is also known as Miner Tip as it is
    /// paid directly to block producers.
    pub max_priority_fee_per_gas: U256,
    /// Maximum amount the sender is willing to pay to get their transaction included in a block.
    pub max_fee_per_gas: U256,
    /// The maximum amount of gas the sender is willing to consume on a transaction.
    pub gas_limit: U256,
    /// The receiving address (`None` for the zero address).
    pub to: Option<Address>,
    /// The amount of ETH to transfer.
    pub value: Wei,
    /// Arbitrary binary data for a contract call invocation.
    pub data: Vec<u8>,
    /// A list of addresses and storage keys that the transaction plans to access.
    /// Accesses outside the list are possible, but become more expensive.
    pub access_list: Vec<AccessTuple>,
}

impl Transaction1559 {
    /// RLP encoding of the data for an unsigned message (used to make signature)
    pub fn rlp_append_unsigned(&self, s: &mut RlpStream) {
        self.rlp_append(s, 9);
    }

    /// RLP encoding for a signed message (used to encode the transaction for sending to tx pool)
    pub fn rlp_append_signed(&self, s: &mut RlpStream) {
        self.rlp_append(s, 12);
    }

    fn rlp_append(&self, s: &mut RlpStream, list_len: usize) {
        s.begin_list(list_len);
        s.append(&self.chain_id);
        s.append(&self.nonce);
        s.append(&self.max_priority_fee_per_gas);
        s.append(&self.max_fee_per_gas);
        s.append(&self.gas_limit);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(address) => s.append(&address.raw()),
        };
        s.append(&self.value.raw());
        s.append(&self.data);
        s.begin_list(self.access_list.len());
        for tuple in &self.access_list {
            s.begin_list(2);
            s.append(&tuple.address);
            s.begin_list(tuple.storage_keys.len());
            for key in &tuple.storage_keys {
                s.append(key);
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct SignedTransaction1559 {
    pub transaction: Transaction1559,
    /// The parity (0 for even, 1 for odd) of the y-value of a secp256k1 signature.
    pub parity: u8,
    pub r: U256,
    pub s: U256,
}

impl SignedTransaction1559 {
    pub fn sender(&self) -> Result<Address, Error> {
        let mut rlp_stream = RlpStream::new();
        rlp_stream.append(&TYPE_BYTE);
        self.transaction.rlp_append_unsigned(&mut rlp_stream);
        let message_hash = aurora_engine_sdk::keccak(rlp_stream.as_raw());
        sdk::ecrecover(
            message_hash,
            &super::vrs_to_arr(self.parity, self.r, self.s),
        )
        .map_err(|_| Error::EcRecover)
    }
}

impl Encodable for SignedTransaction1559 {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.transaction.rlp_append_signed(s);
        s.append(&self.parity);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Decodable for SignedTransaction1559 {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        if rlp.item_count() != Ok(12) {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        let chain_id = rlp.val_at(0)?;
        let nonce = rlp.val_at(1)?;
        let max_priority_fee_per_gas = rlp.val_at(2)?;
        let max_fee_per_gas = rlp.val_at(3)?;
        let gas_limit = rlp.val_at(4)?;
        let to = super::rlp_extract_to(rlp, 5)?;
        let value = Wei::new(rlp.val_at(6)?);
        let data = rlp.val_at(7)?;
        let access_list = rlp.list_at(8)?;
        let parity = rlp.val_at(9)?;
        let r = rlp.val_at(10)?;
        let s = rlp.val_at(11)?;
        Ok(Self {
            transaction: Transaction1559 {
                chain_id,
                nonce,
                max_priority_fee_per_gas,
                max_fee_per_gas,
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
