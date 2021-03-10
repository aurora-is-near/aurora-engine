#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use primitive_types::{H160, H256, U256};

pub struct Engine {
    chain_id: U256,
    origin: H160,
}

impl Engine {
    pub fn new(chain_id: u64, origin: H160) -> Self {
        Self {
            chain_id: U256::from(chain_id),
            origin,
        }
    }
}
