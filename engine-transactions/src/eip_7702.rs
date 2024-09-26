use crate::eip_2930::AccessTuple;
use crate::Error;
use aurora_engine_precompiles::secp256k1::ecrecover;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{Vec, H160, U256};
use evm::executor::stack::Authorization;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Type indicator (per EIP-7702)
pub const TYPE_BYTE: u8 = 0x04;

// EIP-7702 `MAGIC` number
pub const MAGIC: u8 = 0x5;

/// The order of the secp256k1 curve, divided by two. Signatures that should be checked according
/// to EIP-2 should have an S value less than or equal to this.
///
/// `57896044618658097711785492504343953926418782139537452191302581570759080747168`
pub const SECP256K1N_HALF: U256 = U256([
    0xDFE9_2F46_681B_20A0,
    0x5D57_6E73_57A4_501D,
    0xFFFF_FFFF_FFFF_FFFF,
    0x7FFF_FFFF_FFFF_FFFF,
]);

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuthorizationTuple {
    pub chain_id: U256,
    pub address: H160,
    pub nonce: u64,
    pub parity: U256,
    pub r: U256,
    pub s: U256,
}

impl Decodable for AuthorizationTuple {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        let chain_id = rlp.val_at(0)?;
        let address = rlp.val_at(1)?;
        let nonce = rlp.val_at(2)?;
        let parity = rlp.val_at(3)?;
        let r = rlp.val_at(4)?;
        let s = rlp.val_at(5)?;
        Ok(Self {
            chain_id,
            address,
            nonce,
            parity,
            r,
            s,
        })
    }
}

/// EIP-7702 transaction kind from the Prague hard fork.
///
/// See [EIP-7702](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-7702.md)
/// for more details.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Transaction7702 {
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
    /// A list of authorizations for EIP-7702
    pub authorization_list: Vec<AuthorizationTuple>,
}

impl Transaction7702 {
    const TRANSACTION_FIELDS: usize = 10;
    /// RLP encoding of the data for an unsigned message (used to make signature)
    pub fn rlp_append_unsigned(&self, s: &mut RlpStream) {
        self.rlp_append(s, Self::TRANSACTION_FIELDS);
    }

    /// RLP encoding for a signed message (used to encode the transaction for sending to tx pool)
    pub fn rlp_append_signed(&self, s: &mut RlpStream) {
        self.rlp_append(s, SignedTransaction7702::TRANSACTION_FIELDS);
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
        s.begin_list(self.authorization_list.len());
        for tuple in &self.authorization_list {
            s.begin_list(6);
            s.append(&tuple.chain_id);
            s.append(&tuple.address);
            s.append(&tuple.nonce);
            s.append(&tuple.parity);
            s.append(&tuple.r);
            s.append(&tuple.s);
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct SignedTransaction7702 {
    pub transaction: Transaction7702,
    /// The parity (0 for even, 1 for odd) of the y-value of a secp256k1 signature.
    pub parity: u8,
    pub r: U256,
    pub s: U256,
}

impl SignedTransaction7702 {
    const TRANSACTION_FIELDS: usize = 13;

    pub fn sender(&self) -> Result<Address, Error> {
        let mut rlp_stream = RlpStream::new();
        rlp_stream.append(&TYPE_BYTE);
        self.transaction.rlp_append_unsigned(&mut rlp_stream);
        let message_hash = aurora_engine_sdk::keccak(rlp_stream.as_raw());
        ecrecover(
            message_hash,
            &super::vrs_to_arr(self.parity, self.r, self.s),
        )
        .map_err(|_e| Error::EcRecover)
    }

    pub fn authorization_list(&self) -> Result<Vec<Authorization>, Error> {
        if self.transaction.authorization_list.is_empty() {
            return Err(Error::EmptyAuthorizationList);
        }
        let mut authorization_list = Vec::with_capacity(self.transaction.authorization_list.len());
        // According to EIP-7702 we should validate each authorization. We shouldn't skip any of them.
        // And just put `is_valid` flag to `false` if any of them is invalid. It's related to
        // gas calculation, as each `authorization_list` must be charged, even if it's invalid.
        // The exception is invalid signature, as it's indicate wrong transaction.
        for auth in &self.transaction.authorization_list {
            // Validate the signature, as in tests it is possible to have invalid signatures values.
            let v = auth.parity.0;
            if !(v[0] < u64::from(u8::MAX) && v[1..4].iter().all(|&elem| elem == 0)) {
                return Err(Error::InvalidAuthorizationSignature);
            }
            // Value `v` shouldn't be greater then 1
            let v = u8::try_from(v[0]).map_err(|_| Error::InvalidAuthorizationSignature)?;
            if v > 1 {
                return Err(Error::InvalidAuthorizationSignature);
            }
            // EIP-2 validation
            if auth.s > SECP256K1N_HALF {
                return Err(Error::InvalidAuthorizationSignature);
            }

            // According to EIP-7702 step 1. validation, we should verify is `chain_id` is related to current network.
            // We just ensure that `chain_id` is not zero, as it's possible different `chain_id` and
            // `chain_id == 0` is related to Ethereum `mainnet`, that's not correct for Aurora.
            let mut is_valid = !auth.chain_id.is_zero();

            // 2. Checking: authority = ecrecover(keccak(MAGIC || rlp([chain_id, address, nonce])), y_parity, r, s])
            let mut rlp_stream = RlpStream::new();
            rlp_stream.begin_list(4);
            rlp_stream.append(&MAGIC);
            rlp_stream.append(&auth.chain_id);
            rlp_stream.append(&auth.address);
            rlp_stream.append(&auth.nonce);

            let signature_hash = aurora_engine_sdk::keccak(rlp_stream.as_raw());

            let auth_address = ecrecover(signature_hash, &super::vrs_to_arr(v, auth.r, auth.s));
            let auth_address = auth_address.unwrap_or_else(|_| {
                is_valid = false;
                Address::default()
            });

            // Validations steps 3-8 0f EIP-7702 provided by EVM itself.
            authorization_list.push(Authorization {
                authority: auth_address.raw(),
                address: auth.address,
                nonce: auth.nonce,
                is_valid,
            });
        }
        Ok(authorization_list)
    }
}

impl Encodable for SignedTransaction7702 {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.transaction.rlp_append_signed(s);
        s.append(&self.parity);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Decodable for SignedTransaction7702 {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        if rlp.item_count() != Ok(Self::TRANSACTION_FIELDS) {
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
        let authorization_list = rlp.list_at(9)?;
        let parity = rlp.val_at(10)?;
        let r = rlp.val_at(11)?;
        let s = rlp.val_at(12)?;
        Ok(Self {
            transaction: Transaction7702 {
                chain_id,
                nonce,
                max_priority_fee_per_gas,
                max_fee_per_gas,
                gas_limit,
                to,
                value,
                data,
                access_list,
                authorization_list,
            },
            parity,
            r,
            s,
        })
    }
}
