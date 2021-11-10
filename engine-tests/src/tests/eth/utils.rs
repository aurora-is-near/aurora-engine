use crate::prelude::{H160, H256, U256};
use evm::backend::MemoryAccount;
use sha3::{Digest, Keccak256};
use std::collections::BTreeMap;

pub fn u256_to_h256(u: U256) -> H256 {
    let mut h = H256::default();
    u.to_big_endian(&mut h[..]);
    h
}

pub fn unwrap_to_account(s: &ethjson::spec::Account) -> MemoryAccount {
    MemoryAccount {
        balance: s.balance.clone().unwrap().into(),
        nonce: s.nonce.clone().unwrap().into(),
        code: s.code.clone().unwrap().into(),
        storage: s
            .storage
            .as_ref()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                (
                    u256_to_h256(k.clone().into()),
                    u256_to_h256(v.clone().into()),
                )
            })
            .collect(),
    }
}

pub fn unwrap_to_state(a: &ethjson::spec::State) -> BTreeMap<H160, MemoryAccount> {
    match &a.0 {
        ethjson::spec::HashOrMap::Map(m) => m
            .iter()
            .map(|(k, v)| (k.clone().into(), unwrap_to_account(v)))
            .collect(),
        ethjson::spec::HashOrMap::Hash(_) => panic!("Hash can not be converted."),
    }
}

/// Basic account type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrieAccount {
    /// Nonce of the account.
    pub nonce: U256,
    /// Balance of the account.
    pub balance: U256,
    /// Storage root of the account.
    pub storage_root: H256,
    /// Code hash of the account.
    pub code_hash: H256,
    /// Code version of the account.
    pub code_version: U256,
}

impl rlp::Encodable for TrieAccount {
    fn rlp_append(&self, stream: &mut rlp::RlpStream) {
        let use_short_version = self.code_version == U256::zero();

        match use_short_version {
            true => {
                stream.begin_list(4);
            }
            false => {
                stream.begin_list(5);
            }
        }

        stream.append(&self.nonce);
        stream.append(&self.balance);
        stream.append(&self.storage_root);
        stream.append(&self.code_hash);

        if !use_short_version {
            stream.append(&self.code_version);
        }
    }
}

impl rlp::Decodable for TrieAccount {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let use_short_version = match rlp.item_count()? {
            4 => true,
            5 => false,
            _ => return Err(rlp::DecoderError::RlpIncorrectListLen),
        };

        Ok(TrieAccount {
            nonce: rlp.val_at(0)?,
            balance: rlp.val_at(1)?,
            storage_root: rlp.val_at(2)?,
            code_hash: rlp.val_at(3)?,
            code_version: if use_short_version {
                U256::zero()
            } else {
                rlp.val_at(4)?
            },
        })
    }
}

pub fn assert_valid_state(a: &ethjson::spec::State, b: &BTreeMap<H160, MemoryAccount>) {
    match &a.0 {
        ethjson::spec::HashOrMap::Map(m) => {
            assert_eq!(
                &m.iter()
                    .map(|(k, v)| { (k.clone().into(), unwrap_to_account(v)) })
                    .collect::<BTreeMap<_, _>>(),
                b
            );
        }
        ethjson::spec::HashOrMap::Hash(h) => assert_valid_hash(&h.clone().into(), b),
    }
}

pub fn assert_valid_hash(h: &H256, b: &BTreeMap<H160, MemoryAccount>) {
    let tree = b
        .iter()
        .map(|(address, account)| {
            let storage_root = triehash_ethereum::sec_trie_root(
                account
                    .storage
                    .iter()
                    .map(|(k, v)| (k, rlp::encode(&U256::from_big_endian(&v[..])))),
            );
            let code_hash = H256::from_slice(Keccak256::digest(&account.code).as_slice());

            let account = TrieAccount {
                nonce: account.nonce,
                balance: account.balance,
                storage_root,
                code_hash,
                code_version: U256::zero(),
            };

            (address, rlp::encode(&account))
        })
        .collect::<Vec<_>>();

    let root = triehash_ethereum::sec_trie_root(tree);
    let expect = h.clone().into();

    if root != expect {
        panic!(
            "Hash not equal; calculated: {:?}, expect: {:?}\nState: {:?}",
            root, expect, b
        );
    }
}
