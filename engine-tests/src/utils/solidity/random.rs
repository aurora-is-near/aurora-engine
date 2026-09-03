use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::H256;
use aurora_engine_types::types::Wei;
use ethabi::Constructor;

use crate::prelude::U256;
use crate::utils::{self, AuroraRunner, Signer, solidity};

const DEFAULT_GAS: u64 = 1_000_000_000;

pub struct RandomConstructor(pub solidity::ContractConstructor);

impl RandomConstructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_extended_json(
            "../etc/eth-contracts/artifacts/contracts/test/Random.sol/Random.json",
        ))
    }

    pub fn deploy(&self, nonce: u64) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap_or(&Constructor { inputs: vec![] })
            .encode_input(self.0.code.clone(), &[])
            .unwrap();

        TransactionLegacy {
            nonce: nonce.into(),
            gas_price: U256::default(),
            gas_limit: U256::from(DEFAULT_GAS),
            to: None,
            value: Wei::default(),
            data,
        }
    }
}

impl From<RandomConstructor> for solidity::ContractConstructor {
    fn from(c: RandomConstructor) -> Self {
        c.0
    }
}

pub struct Random {
    contract: solidity::DeployedContract,
}

impl Random {
    pub fn random_seed(&self, runner: &mut AuroraRunner, signer: &mut Signer) -> H256 {
        let data = self
            .contract
            .abi
            .function("randomSeed")
            .unwrap()
            .encode_input(&[])
            .unwrap();

        let tx = TransactionLegacy {
            nonce: signer.use_nonce().into(),
            gas_price: U256::default(),
            gas_limit: U256::from(DEFAULT_GAS),
            to: Some(self.contract.address),
            value: Wei::default(),
            data,
        };

        let result = runner.submit_transaction(&signer.secret_key, tx).unwrap();
        let result = utils::unwrap_success(result);

        let mut random_seed = [0; 32];
        random_seed.copy_from_slice(result.as_slice());
        H256::from(random_seed)
    }
}

impl From<solidity::DeployedContract> for Random {
    fn from(contract: solidity::DeployedContract) -> Self {
        Self { contract }
    }
}
