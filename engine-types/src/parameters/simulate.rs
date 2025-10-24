#[cfg(not(any(feature = "std", feature = "contracts-std")))]
use alloc::collections::BTreeMap;
#[cfg(not(any(feature = "std", feature = "contracts-std")))]
use alloc::vec::Vec;

#[cfg(any(feature = "std", feature = "contracts-std"))]
use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use primitive_types::{H256, U256};

use crate::types::{Address, GasLimit, Wei};

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct U256BorshWrapper(pub [u64; 4]);

impl From<U256> for U256BorshWrapper {
    fn from(U256(v): U256) -> Self {
        U256BorshWrapper(v)
    }
}

impl From<U256BorshWrapper> for U256 {
    fn from(U256BorshWrapper(v): U256BorshWrapper) -> Self {
        U256(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub struct H256BorshWrapper(pub [u8; 32]);

impl From<H256> for H256BorshWrapper {
    fn from(H256(v): H256) -> Self {
        H256BorshWrapper(v)
    }
}

impl From<H256BorshWrapper> for H256 {
    fn from(H256BorshWrapper(v): H256BorshWrapper) -> Self {
        H256(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct StateOverride {
    pub balance: Option<U256BorshWrapper>,
    pub nonce: Option<U256BorshWrapper>,
    pub code: Option<Vec<u8>>,
    pub state: Option<BTreeMap<H256BorshWrapper, H256BorshWrapper>>,
    pub state_diff: Option<Vec<(H256BorshWrapper, H256BorshWrapper)>>,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct SimulateEthCallArgs {
    pub from: Address,
    pub to: Option<Address>,
    pub gas_limit: GasLimit,
    pub gas_price: U256BorshWrapper,
    pub value: Wei,
    pub data: Vec<u8>,
    pub nonce: Option<u64>,
    pub state_override: Vec<(Address, StateOverride)>,
}
