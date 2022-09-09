use crate::prelude::{sdk, U256};
use crate::runner::AURORA_LOCAL_CHAIN_ID;
use crate::test_utils::solidity::ContractConstructor;
use aurora_engine_transactions::legacy::{LegacyEthSignedTransaction, TransactionLegacy};
use aurora_engine_types::types::Address;
use ethabi::Bytes;
use libsecp256k1::{Message, PublicKey, SecretKey};
use rlp::RlpStream;

pub(crate) fn address_from_secret_key(sk: &SecretKey) -> Address {
    let pk = PublicKey::from_secret_key(sk);
    let hash = sdk::keccak(&pk.serialize()[1..]);
    Address::try_from_slice(&hash[12..]).unwrap()
}

pub(crate) fn sign_transaction(
    tx: TransactionLegacy,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    let mut rlp_stream = RlpStream::new();
    tx.rlp_append_unsigned(&mut rlp_stream, chain_id);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let v: u64 = match chain_id {
        Some(chain_id) => (recovery_id.serialize() as u64) + 2 * chain_id + 35,
        None => (recovery_id.serialize() as u64) + 27,
    };
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());
    LegacyEthSignedTransaction {
        transaction: tx,
        v,
        r,
        s,
    }
}

// TODO: a sort of "account" which tracks nonce and others would be a better
// approach.
pub struct Signer {
    pub nonce: u64,
    pub secret_key: SecretKey,
}

impl Signer {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            nonce: 0,
            secret_key,
        }
    }

    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let sk = SecretKey::random(&mut rng);
        Self::new(sk)
    }

    pub fn use_nonce(&mut self) -> u64 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }

    pub fn address(&self) -> Address {
        address_from_secret_key(&self.secret_key)
    }

    pub(crate) fn sign_tx(&mut self, tx: TransactionLegacy) -> LegacyEthSignedTransaction {
        sign_transaction(tx, Some(AURORA_LOCAL_CHAIN_ID), &self.secret_key)
    }

    pub(crate) fn construct_tx_and_sign<F: FnOnce(U256) -> TransactionLegacy>(
        &mut self,
        make_tx: F,
    ) -> LegacyEthSignedTransaction {
        let nonce = self.use_nonce();
        let tx = make_tx(nonce.into());
        sign_transaction(tx, Some(AURORA_LOCAL_CHAIN_ID), &self.secret_key)
        // rlp::encode(&signed_tx).to_vec()
    }

    pub(crate) fn construct_deploy_tx_and_sign<
        F: FnOnce(&T) -> TransactionLegacy,
        T: Into<ContractConstructor>,
    >(
        &mut self,
        constructor_tx: F,
        contract_constructor: T,
    ) -> LegacyEthSignedTransaction {
        let tx = constructor_tx(&contract_constructor);
        sign_transaction(tx, Some(AURORA_LOCAL_CHAIN_ID), &self.secret_key)
    }
}
