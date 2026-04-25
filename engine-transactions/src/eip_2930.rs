use aurora_engine_sdk as sdk;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H160, H256, U256, Vec};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::Error;

/// Type indicator (per EIP-2718) for access list transactions
pub const TYPE_BYTE: u8 = 0x01;

/// Hard cap on the number of entries in a transaction's `access_list`.
/// Enforced during RLP decoding to bound NEAR-gas cost against malicious
/// payloads. Applied uniformly to EIP-2930, EIP-1559 and EIP-7702 since
/// they all share `AccessTuple`.
pub const ACCESS_LIST_LENGTH: usize = 512;

/// Hard cap on the number of `storage_keys` inside a single `AccessTuple`.
/// Enforced during RLP decoding of each tuple (`AccessTuple::decode`).
/// Combined with `ACCESS_LIST_LENGTH`, the total number of storage slots
/// warmed from a single tx is bounded by `ACCESS_LIST_LENGTH * ACCESS_LIST_STORAGE_KEY_LENGTH`.
pub const ACCESS_LIST_STORAGE_KEY_LENGTH: usize = 16;

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AccessTuple {
    pub address: H160,
    pub storage_keys: Vec<H256>,
}

impl Decodable for AccessTuple {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        let address = rlp.val_at(0)?;

        // Gate storage_keys length BEFORE per-item decode (each key is 32 bytes
        // + RLP overhead). `take(MAX + 1).count()` bounds iteration cost to O(MAX)
        // regardless of attacker-supplied list size.
        let keys_rlp = rlp.at(1)?;
        if keys_rlp
            .iter()
            .take(ACCESS_LIST_STORAGE_KEY_LENGTH + 1)
            .count()
            > ACCESS_LIST_STORAGE_KEY_LENGTH
        {
            return Err(DecoderError::Custom("ERR_STORAGE_KEYS_TOO_LARGE"));
        }
        let storage_keys: Vec<H256> = keys_rlp.as_list()?;

        Ok(Self {
            address,
            storage_keys,
        })
    }
}

/// See `https://eips.ethereum.org/EIPS/eip-2930`
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Transaction2930 {
    /// ID of chain which the transaction belongs.
    pub chain_id: u64,
    /// A monotonically increasing transaction counter for this sender
    pub nonce: U256,
    /// The fee the sender pays per unit of gas.
    pub gas_price: U256,
    /// The maximum amount of gas the sender is willing to consume on a transaction.
    pub gas_limit: U256,
    /// The receiving address (`None` for the zero address)
    pub to: Option<Address>,
    /// The amount of ETH to transfer.
    pub value: Wei,
    /// Arbitrary binary data for a contract call invocation
    pub data: Vec<u8>,
    /// A list of addresses and storage keys that the transaction plans to access.
    /// Accesses outside the list are possible, but become more expensive.
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
        sdk::ecrecover(
            message_hash,
            &super::vrs_to_arr(self.parity, self.r, self.s),
        )
        .map_err(|_| Error::EcRecover)
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
            return Err(DecoderError::RlpIncorrectListLen);
        }
        let chain_id = rlp.val_at(0)?;
        let nonce = rlp.val_at(1)?;
        let gas_price = rlp.val_at(2)?;
        let gas_limit = rlp.val_at(3)?;
        let to = super::rlp_extract_to(rlp, 4)?;
        let value = Wei::new(rlp.val_at(5)?);
        let data = rlp.val_at(6)?;

        // Gate access_list length BEFORE the expensive per-item decode.
        // `take(MAX + 1).count()` bounds iteration regardless of attacker-supplied
        // list size, so the check cost is O(MAX) instead of O(N_actual) worst-case.
        let access_list_rlp = rlp.at(7)?;
        if access_list_rlp.iter().take(ACCESS_LIST_LENGTH + 1).count() > ACCESS_LIST_LENGTH {
            return Err(DecoderError::Custom("ERR_ACCESS_LIST_TOO_LARGE"));
        }
        let access_list: Vec<AccessTuple> = access_list_rlp.as_list()?;

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
