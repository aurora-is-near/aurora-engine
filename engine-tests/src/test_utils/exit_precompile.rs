use crate::prelude::{parameters::SubmitResult, transaction::LegacyEthTransaction, Address, U256};
use crate::test_utils::{self, solidity, AuroraRunner, Signer};

pub(crate) struct TesterConstructor(pub solidity::ContractConstructor);

const DEPLOY_CONTRACT_GAS: u64 = 1_000_000_000;

impl TesterConstructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_extended_json(
            "../etc/eth-contracts/artifacts/contracts/test/Tester.sol/Tester.json",
        ))
    }

    pub fn deploy(&self, nonce: u64, token: Address) -> LegacyEthTransaction {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
            .encode_input(self.0.code.clone(), &[ethabi::Token::Address(token)])
            .unwrap();

        LegacyEthTransaction {
            nonce: nonce.into(),
            gas_price: Default::default(),
            gas: U256::from(DEPLOY_CONTRACT_GAS),
            to: None,
            value: Default::default(),
            data,
        }
    }
}

pub(crate) struct Tester {
    pub contract: solidity::DeployedContract,
}

impl From<TesterConstructor> for solidity::ContractConstructor {
    fn from(c: TesterConstructor) -> Self {
        c.0
    }
}

impl From<solidity::DeployedContract> for Tester {
    fn from(contract: solidity::DeployedContract) -> Self {
        Self { contract }
    }
}

impl Tester {
    fn call_function(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        method: &str,
        params: &[ethabi::Token],
    ) -> Result<SubmitResult, Revert> {
        let data = self
            .contract
            .abi
            .function(method)
            .unwrap()
            .encode_input(params)
            .unwrap();

        let tx = LegacyEthTransaction {
            nonce: signer.use_nonce().into(),
            gas_price: Default::default(),
            gas: U256::from(DEPLOY_CONTRACT_GAS),
            to: Some(self.contract.address),
            value: Default::default(),
            data,
        };

        let result = runner.submit_transaction(&signer.secret_key, tx).unwrap();
        match result.status {
            aurora_engine::parameters::TransactionStatus::Succeed(_) => Ok(result),
            aurora_engine::parameters::TransactionStatus::Revert(bytes) => Err(Revert(bytes)),
            other => panic!("Unexpected status {:?}", other),
        }
    }

    pub fn hello_world(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        name: String,
    ) -> String {
        let output_type = &[ethabi::ParamType::String];
        let result = self
            .call_function(runner, signer, "helloWorld", &[ethabi::Token::String(name)])
            .unwrap();
        let output_bytes = test_utils::unwrap_success(result);
        let output = ethabi::decode(output_type, &output_bytes).unwrap();

        match &output[..] {
            [ethabi::Token::String(string)] => string.to_string(),
            _ => unreachable!(),
        }
    }

    pub fn withdraw(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        self.call_function(runner, signer, "withdraw", &[ethabi::Token::Bool(flag)])
    }

    pub fn withdraw_and_fail(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        self.call_function(
            runner,
            signer,
            "withdrawAndFail",
            &[ethabi::Token::Bool(flag)],
        )
    }

    pub fn try_withdraw_and_avoid_fail(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        self.call_function(
            runner,
            signer,
            "tryWithdrawAndAvoidFail",
            &[ethabi::Token::Bool(flag)],
        )
    }

    pub fn try_withdraw_and_avoid_fail_and_succeed(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        self.call_function(
            runner,
            signer,
            "tryWithdrawAndAvoidFailAndSucceed",
            &[ethabi::Token::Bool(flag)],
        )
    }
}

#[derive(Debug)]
pub(crate) struct Revert(Vec<u8>);
