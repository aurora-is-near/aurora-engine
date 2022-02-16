use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::U256;

use crate::test_utils::solidity;

pub struct WethConstructor(solidity::ContractConstructor);

impl WethConstructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_source(
            "src/tests/res",
            "src/tests/res",
            "weth.sol",
            "weth",
        ))
    }

    pub fn deploy(&self, nonce: U256) -> TransactionLegacy {
        self.0.deploy_without_constructor(nonce)
    }

    #[allow(dead_code)]
    pub fn deployed_at(self, address: Address) -> Weth {
        Weth(self.0.deployed_at(address))
    }
}

// We never need to access deployed WETH in current tests because we are replaying mainnet
// transactions. But this might still be useful in the future.
#[allow(dead_code)]
pub struct Weth(solidity::DeployedContract);

impl Weth {
    #[allow(dead_code)]
    pub fn deposit(&self, amount: Wei, nonce: U256) -> TransactionLegacy {
        let mut result = self.0.call_method_without_args("deposit", nonce);
        result.value = amount;
        result
    }
}
