use crate::prelude::U256;
use crate::test_utils::{self, solidity, AuroraRunner, Signer};
use aurora_engine::transaction::legacy::TransactionLegacy;
use ethabi::Constructor;

const DEFAULT_GAS: u64 = 1_000_000_000;

pub(crate) struct RandomConstructor(pub solidity::ContractConstructor);

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
            gas_price: Default::default(),
            gas_limit: U256::from(DEFAULT_GAS),
            to: None,
            value: Default::default(),
            data,
        }
    }
}

impl From<RandomConstructor> for solidity::ContractConstructor {
    fn from(c: RandomConstructor) -> Self {
        c.0
    }
}

pub(crate) struct Random {
    contract: solidity::DeployedContract,
}

impl Random {
    pub fn random_256(&self, runner: &mut AuroraRunner, signer: &mut Signer) -> Option<U256> {
        let data = self
            .contract
            .abi
            .function("randomU256")
            .unwrap()
            .encode_input(&[])
            .unwrap();

        let tx = TransactionLegacy {
            nonce: signer.use_nonce().into(),
            gas_price: Default::default(),
            gas_limit: U256::from(DEFAULT_GAS),
            to: Some(self.contract.address),
            value: Default::default(),
            data,
        };

        let result = runner.submit_transaction(&signer.secret_key, tx).unwrap();
        let result = test_utils::unwrap_success(result);

        if result.len() == 32 {
            Some(U256::from(result.as_slice()))
        } else {
            None
        }
    }
}

impl From<solidity::DeployedContract> for Random {
    fn from(contract: solidity::DeployedContract) -> Self {
        Self { contract }
    }
}
