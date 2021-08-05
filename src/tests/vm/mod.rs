mod utils;

use primitive_types::{U256, H256, H160};
use serde::Deserialize;
use evm::backend::{MemoryBackend, MemoryAccount, MemoryVicinity};
use sha3::{Digest, Keccak256};
use std::collections::BTreeMap;
use std::rc::Rc;
use self::utils::*;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::vm::Vm);

impl Test {
    pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
        unwrap_to_state(&self.0.pre_state)
    }

    pub fn unwrap_to_vicinity(&self) -> MemoryVicinity {
        MemoryVicinity {
            gas_price: self.0.transaction.gas_price.clone().into(),
            origin: self.0.transaction.origin.clone().into(),
            block_hashes: Vec::new(),
            block_number: self.0.env.number.clone().into(),
            block_coinbase: self.0.env.author.clone().into(),
            block_timestamp: self.0.env.timestamp.clone().into(),
            block_difficulty: self.0.env.difficulty.clone().into(),
            block_gas_limit: self.0.env.gas_limit.clone().into(),
            chain_id: U256::zero(),
        }
    }

    pub fn unwrap_to_code(&self) -> Rc<Vec<u8>> {
        Rc::new(self.0.transaction.code.clone().into())
    }

    pub fn unwrap_to_data(&self) -> Rc<Vec<u8>> {
        Rc::new(self.0.transaction.data.clone().into())
    }

    pub fn unwrap_to_context(&self) -> evm::Context {
        evm::Context {
            address: self.0.transaction.address.clone().into(),
            caller: self.0.transaction.sender.clone().into(),
            apparent_value: self.0.transaction.value.clone().into(),
        }
    }

    pub fn unwrap_to_return_value(&self) -> Vec<u8> {
        self.0.output.clone().unwrap().into()
    }

    pub fn unwrap_to_gas_limit(&self) -> u64 {
        self.0.transaction.gas.clone().into()
    }

    pub fn unwrap_to_post_gas(&self) -> u64 {
        self.0.gas_left.clone().unwrap().into()
    }
}


fn executor(eth_test: Test) {
    let original_state = eth_test.unwrap_to_pre_state();
    let vicinity = eth_test.unwrap_to_vicinity();
    let _backend = MemoryBackend::new(&vicinity, original_state);
}

#[test]
fn test_eth_vm_1() {
    assert!(true);
}
