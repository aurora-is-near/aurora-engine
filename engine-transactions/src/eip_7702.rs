use crate::eip_2930::AccessTuple;
use crate::Error;
use aurora_engine_sdk::ecrecover;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{Vec, H160, U256};
use aurora_evm::executor::stack::Authorization;
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
    /// The receiving address.
    pub to: Address,
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
        s.append(&self.to.raw());
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
        let current_tx_chain_id = U256::from(self.transaction.chain_id);
        let mut authorization_list = Vec::with_capacity(self.transaction.authorization_list.len());
        let mut rlp_stream = RlpStream::new();
        let mut message_bytes = vec![MAGIC; 1];
        // According to EIP-7702, we should validate each authorization. We shouldn't skip any of them.
        // And just put the `is_valid` flag to `false` if any of them is invalid. It's related to
        // gas calculation, as each `authorization_list` must be charged, even if it's invalid.
        for auth in &self.transaction.authorization_list {
            // According to EIP-7702 step 1. validation, we should verify it as
            // `chain_id = 0 || current_chain_id`.
            // AS `current_chain_id` we used `transaction.chain_id` as we will validate `chain_id` in
            // Engine `submit_transaction` method.

            // Step 2 - validation logic inside EVM itself.
            // Step 3. Checking: authority = ecrecover(keccak(MAGIC || rlp([chain_id, address, nonce])), y_parity, r, s])
            // Validate the signature, as in tests it is possible to have invalid signature values.
            // Value `v` shouldn't be greater than 1
            let v = u8::try_from(auth.parity.as_u64()).map_err(|_| Error::InvalidV)?;
            let mut is_valid = if auth.s > SECP256K1N_HALF {
                false
            } else {
                (auth.chain_id.is_zero() || auth.chain_id == current_tx_chain_id) && v <= 1
            };

            rlp_stream.begin_list(3);
            rlp_stream.append(&auth.chain_id);
            rlp_stream.append(&auth.address);
            rlp_stream.append(&auth.nonce);

            message_bytes.extend_from_slice(rlp_stream.as_raw());

            let signature_hash = aurora_engine_sdk::keccak(&message_bytes);
            let auth_address = ecrecover(signature_hash, &super::vrs_to_arr(v, auth.r, auth.s))
                .unwrap_or_else(|_| {
                    is_valid = false;
                    Address::default()
                });

            // Validations steps 2,4-9 0f EIP-7702 provided by EVM itself.
            authorization_list.push(Authorization {
                authority: auth_address.raw(),
                address: auth.address,
                nonce: auth.nonce,
                is_valid,
            });

            message_bytes.truncate(1);
            rlp_stream.clear();
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
        let to = Address::new(rlp.val_at(5)?);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec;
    use rlp::RlpStream;

    #[test]
    fn test_authorization_tuple_decode() {
        let chain_id = 1.into();
        let address = H160::from_low_u64_be(0x1234);
        let nonce = 1u64;
        let parity = U256::zero();
        let r = 2.into();
        let s = 3.into();

        let mut stream = RlpStream::new_list(6);
        stream.append(&chain_id);
        stream.append(&address);
        stream.append(&nonce);
        stream.append(&parity);
        stream.append(&r);
        stream.append(&s);

        let rlp = Rlp::new(stream.as_raw());
        let decoded: AuthorizationTuple = rlp.as_val().unwrap();

        assert_eq!(decoded.chain_id, chain_id);
        assert_eq!(decoded.address, address);
        assert_eq!(decoded.nonce, nonce);
        assert_eq!(decoded.parity, parity);
        assert_eq!(decoded.r, r);
        assert_eq!(decoded.s, s);
    }

    #[test]
    fn test_transaction7702_rlp_append_unsigned() {
        let tx = Transaction7702 {
            chain_id: 1,
            nonce: 1.into(),
            max_priority_fee_per_gas: 2.into(),
            max_fee_per_gas: U256::from(3),
            gas_limit: 4.into(),
            to: Address::new(H160::from_low_u64_be(0x1234)),
            value: Wei::new(5.into()),
            data: vec![0x6],
            access_list: vec![],
            authorization_list: vec![AuthorizationTuple {
                chain_id: 1.into(),
                address: H160::from_low_u64_be(0x1234),
                nonce: 1u64,
                parity: U256::zero(),
                r: 2.into(),
                s: 3.into(),
            }],
        };

        let mut stream = RlpStream::new();
        tx.rlp_append_unsigned(&mut stream);

        let rlp = Rlp::new(stream.as_raw());
        assert_eq!(
            rlp.item_count().unwrap(),
            Transaction7702::TRANSACTION_FIELDS
        );
    }

    #[test]
    fn test_signed_transaction7702_rlp_encode_decode() {
        let tx = Transaction7702 {
            chain_id: 1,
            nonce: 1.into(),
            max_priority_fee_per_gas: 2.into(),
            max_fee_per_gas: 3.into(),
            gas_limit: 4.into(),
            to: Address::new(H160::from_low_u64_be(0x1234)),
            value: Wei::new(5.into()),
            data: vec![0x6],
            access_list: vec![],
            authorization_list: vec![AuthorizationTuple {
                chain_id: 1.into(),
                address: H160::from_low_u64_be(0x1234),
                nonce: 1u64,
                parity: U256::zero(),
                r: 2.into(),
                s: 3.into(),
            }],
        };

        let signed_tx = SignedTransaction7702 {
            transaction: tx,
            parity: 0,
            r: 7.into(),
            s: 8.into(),
        };

        let mut stream = RlpStream::new();
        signed_tx.rlp_append(&mut stream);

        let rlp = Rlp::new(stream.as_raw());
        let decoded: SignedTransaction7702 = rlp.as_val().unwrap();

        assert_eq!(decoded, signed_tx);
    }

    #[test]
    fn test_signed_transaction7702_invalid_chain_id() {
        let mut tx = Transaction7702 {
            chain_id: 1,
            nonce: 1.into(),
            max_priority_fee_per_gas: 2.into(),
            max_fee_per_gas: 3.into(),
            gas_limit: 4.into(),
            to: Address::new(H160::from_low_u64_be(0x1234)),
            value: Wei::new(5.into()),
            data: vec![0x6],
            access_list: vec![],
            authorization_list: vec![AuthorizationTuple {
                chain_id: 2.into(),
                address: H160::from_low_u64_be(0x1234),
                nonce: 1u64,
                parity: 0.into(),
                r: 2.into(),
                s: 3.into(),
            }],
        };

        let signed_tx = SignedTransaction7702 {
            transaction: tx.clone(),
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };

        // Fail
        let auth_list = signed_tx.authorization_list().unwrap();
        assert_eq!(auth_list.len(), 1);
        assert!(!auth_list[0].is_valid);

        // Success
        tx.chain_id = 2;
        let signed_tx = SignedTransaction7702 {
            transaction: tx.clone(),
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };
        let auth_list = signed_tx.authorization_list().unwrap();
        assert!(auth_list[0].is_valid);

        // Success
        tx.authorization_list = vec![AuthorizationTuple {
            chain_id: U256::zero(),
            address: H160::from_low_u64_be(0x1234),
            nonce: 1u64,
            parity: 0.into(),
            r: 2.into(),
            s: 3.into(),
        }];
        let signed_tx = SignedTransaction7702 {
            transaction: tx,
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };
        let auth_list = signed_tx.authorization_list().unwrap();
        assert!(auth_list[0].is_valid);
    }

    #[test]
    fn test_signed_transaction7702_empty_auth_list() {
        let tx = Transaction7702 {
            chain_id: 1,
            nonce: 1.into(),
            max_priority_fee_per_gas: 2.into(),
            max_fee_per_gas: 3.into(),
            gas_limit: 4.into(),
            to: Address::new(H160::from_low_u64_be(0x1234)),
            value: Wei::new(5.into()),
            data: vec![0x6],
            access_list: vec![],
            authorization_list: vec![],
        };

        let signed_tx = SignedTransaction7702 {
            transaction: tx,
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };

        if let Err(err) = signed_tx.authorization_list() {
            assert_eq!(err, Error::EmptyAuthorizationList);
        }
    }

    #[test]
    fn test_signed_transaction7702_invalid_signature_v() {
        let mut tx = Transaction7702 {
            chain_id: 1,
            nonce: 1.into(),
            max_priority_fee_per_gas: 2.into(),
            max_fee_per_gas: 3.into(),
            gas_limit: 4.into(),
            to: Address::new(H160::from_low_u64_be(0x1234)),
            value: Wei::new(5.into()),
            data: vec![0x6],
            access_list: vec![],
            authorization_list: vec![AuthorizationTuple {
                chain_id: 1.into(),
                address: H160::from_low_u64_be(0x1234),
                nonce: 1u64,
                parity: 2.into(),
                r: 2.into(),
                s: 3.into(),
            }],
        };

        let signed_tx = SignedTransaction7702 {
            transaction: tx.clone(),
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };

        let auth_list = signed_tx.authorization_list().unwrap();
        assert_eq!(auth_list.len(), 1);
        assert!(!auth_list[0].is_valid);

        tx.authorization_list = vec![AuthorizationTuple {
            chain_id: 1.into(),
            address: H160::from_low_u64_be(0x1234),
            nonce: 1u64,
            parity: u8::MAX.into(),
            r: 2.into(),
            s: 3.into(),
        }];
        let signed_tx = SignedTransaction7702 {
            transaction: tx.clone(),
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };
        let auth_list = signed_tx.authorization_list().unwrap();
        assert!(!auth_list[0].is_valid);

        // Success
        tx.authorization_list = vec![AuthorizationTuple {
            chain_id: 1.into(),
            address: H160::from_low_u64_be(0x1234),
            nonce: 1u64,
            parity: 0.into(),
            r: 2.into(),
            s: 3.into(),
        }];
        let signed_tx = SignedTransaction7702 {
            transaction: tx,
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };
        let auth_list = signed_tx.authorization_list().unwrap();
        assert!(auth_list[0].is_valid);
    }

    #[test]
    fn test_signed_transaction7702_invalid_signature_s() {
        let mut tx = Transaction7702 {
            chain_id: 1,
            nonce: 1.into(),
            max_priority_fee_per_gas: 2.into(),
            max_fee_per_gas: 3.into(),
            gas_limit: 4.into(),
            to: Address::new(H160::from_low_u64_be(0x1234)),
            value: Wei::new(5.into()),
            data: vec![0x6],
            access_list: vec![],
            authorization_list: vec![AuthorizationTuple {
                chain_id: 1.into(),
                address: H160::from_low_u64_be(0x1234),
                nonce: 1u64,
                parity: 1.into(),
                r: 2.into(),
                s: SECP256K1N_HALF,
            }],
        };

        let signed_tx = SignedTransaction7702 {
            transaction: tx.clone(),
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };

        // Success
        let auth_list = signed_tx.authorization_list().unwrap();
        assert_eq!(auth_list.len(), 1);
        assert!(auth_list[0].is_valid);

        // Fails
        tx.authorization_list = vec![AuthorizationTuple {
            chain_id: U256::MAX,
            address: H160::from_slice(&[255; 20]),
            nonce: u64::MAX,
            parity: u8::MAX.into(),
            r: 2.into(),
            s: SECP256K1N_HALF + U256::from(1),
        }];
        let signed_tx = SignedTransaction7702 {
            transaction: tx,
            parity: 0,
            r: 2.into(),
            s: 3.into(),
        };
        let auth_list = signed_tx.authorization_list().unwrap();
        assert!(!auth_list[0].is_valid);
    }
}
