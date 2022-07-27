use aurora_engine_types::account_id::AccountId;
use crate::prelude::transactions::{
    eip_1559::{self, SignedTransaction1559, Transaction1559},
    eip_2930::{self, SignedTransaction2930, Transaction2930},
    legacy::{LegacyEthSignedTransaction, TransactionLegacy},
};
use rlp::RlpStream;
use crate::prelude::{sdk, Address, Wei, H256, U256};
use libsecp256k1::{self, Message, PublicKey, SecretKey};


#[cfg(test)]
pub(crate) mod engine;
#[cfg(test)]
pub(crate) mod erc20;
#[cfg(test)]
pub(crate) mod solidity;

pub(crate) fn str_to_account_id(account_id: &str) -> AccountId {
    use aurora_engine_types::str::FromStr;
    AccountId::from_str(account_id).unwrap()
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