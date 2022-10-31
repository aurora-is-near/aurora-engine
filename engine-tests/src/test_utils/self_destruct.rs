use crate::prelude::{
    parameters::CallArgs, parameters::FunctionCallArgsV2, transactions::legacy::TransactionLegacy,
    Address, WeiU256, U256,
};
use crate::test_utils::{self, solidity, AuroraRunner, Signer};
use borsh::BorshSerialize;

pub(crate) struct SelfDestructFactoryConstructor(pub solidity::ContractConstructor);

const DEFAULT_GAS: u64 = 1_000_000_000;

impl SelfDestructFactoryConstructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_extended_json(
            "../etc/eth-contracts/artifacts/contracts/test/StateTest.sol/SelfDestructFactory.json",
        ))
    }

    pub fn deploy(&self, nonce: u64) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
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

pub(crate) struct SelfDestructFactory {
    contract: solidity::DeployedContract,
}

impl From<SelfDestructFactoryConstructor> for solidity::ContractConstructor {
    fn from(c: SelfDestructFactoryConstructor) -> Self {
        c.0
    }
}

impl From<solidity::DeployedContract> for SelfDestructFactory {
    fn from(contract: solidity::DeployedContract) -> Self {
        Self { contract }
    }
}

impl SelfDestructFactory {
    pub fn deploy(&self, runner: &mut AuroraRunner, signer: &mut Signer) -> Address {
        let data = self
            .contract
            .abi
            .function("deploy")
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

        Address::try_from_slice(&result[12..]).unwrap()
    }
}

pub(crate) struct SelfDestructConstructor(pub solidity::ContractConstructor);

impl SelfDestructConstructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_extended_json(
            "../etc/eth-contracts/artifacts/contracts/test/StateTest.sol/SelfDestruct.json",
        ))
    }
}

pub(crate) struct SelfDestruct {
    contract: solidity::DeployedContract,
}

impl SelfDestruct {
    pub fn counter(&self, runner: &mut AuroraRunner, signer: &mut Signer) -> Option<u128> {
        let data = self
            .contract
            .abi
            .function("counter")
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
            Some(u128::from_be_bytes(result[16..32].try_into().unwrap()))
        } else {
            None
        }
    }

    pub fn increase(&self, runner: &mut AuroraRunner, signer: &mut Signer) {
        let data = self
            .contract
            .abi
            .function("increase")
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

        runner.submit_transaction(&signer.secret_key, tx).unwrap();
    }

    pub fn finish_using_submit(&self, runner: &mut AuroraRunner, signer: &mut Signer) {
        let data = self
            .contract
            .abi
            .function("finish")
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

        runner.submit_transaction(&signer.secret_key, tx).unwrap();
    }

    pub fn finish(&self, runner: &mut AuroraRunner) {
        let data = self
            .contract
            .abi
            .function("finish")
            .unwrap()
            .encode_input(&[])
            .unwrap();

        let input = CallArgs::V2(FunctionCallArgsV2 {
            contract: self.contract.address,
            value: WeiU256::default(),
            input: data.to_vec(),
        })
        .try_to_vec()
        .unwrap();

        runner.call("call", "anyone", input);
    }
}

impl From<solidity::DeployedContract> for SelfDestruct {
    fn from(contract: solidity::DeployedContract) -> Self {
        Self { contract }
    }
}
