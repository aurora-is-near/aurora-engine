use crate::prelude::{
    parameters::SubmitResult, transactions::legacy::TransactionLegacy, Address, Wei, U256,
};
use crate::test_utils::{self, solidity, AuroraRunner, Signer};
use near_vm_errors::VMError;

pub(crate) struct TesterConstructor(pub solidity::ContractConstructor);

const DEPLOY_CONTRACT_GAS: u64 = 1_000_000_000;
pub const DEST_ACCOUNT: &str = "target.aurora";
pub const DEST_ADDRESS: Address =
    aurora_engine_precompiles::make_address(0xe0f5206b, 0xbd039e7b0592d8918820024e2a7437b9);

impl TesterConstructor {
    #[cfg(feature = "error_refund")]
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_extended_json(
            "../etc/eth-contracts/artifacts/contracts/test/TesterV2.sol/TesterV2.json",
        ))
    }
    #[cfg(not(feature = "error_refund"))]
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_extended_json(
            "../etc/eth-contracts/artifacts/contracts/test/Tester.sol/Tester.json",
        ))
    }

    pub fn deploy(&self, nonce: u64, token: Address) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
            .encode_input(self.0.code.clone(), &[ethabi::Token::Address(token.raw())])
            .unwrap();

        TransactionLegacy {
            nonce: nonce.into(),
            gas_price: Default::default(),
            gas_limit: U256::from(DEPLOY_CONTRACT_GAS),
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
        value: Wei,
        params: &[ethabi::Token],
    ) -> Result<SubmitResult, VMError> {
        let data = self
            .contract
            .abi
            .function(method)
            .unwrap()
            .encode_input(params)
            .unwrap();

        let tx = TransactionLegacy {
            nonce: signer.use_nonce().into(),
            gas_price: Default::default(),
            gas_limit: U256::from(DEPLOY_CONTRACT_GAS),
            to: Some(self.contract.address),
            value,
            data,
        };

        runner.submit_transaction(&signer.secret_key, tx)
    }

    fn submit_result_to_success_or_revert(result: SubmitResult) -> Result<SubmitResult, Revert> {
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
            .call_function(
                runner,
                signer,
                "helloWorld",
                Wei::zero(),
                &[ethabi::Token::String(name)],
            )
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
    ) -> Result<SubmitResult, VMError> {
        self.call_function(
            runner,
            signer,
            "withdraw",
            Wei::zero(),
            &[ethabi::Token::Bool(flag)],
        )
    }

    pub fn withdraw_and_fail(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        Self::submit_result_to_success_or_revert(
            self.call_function(
                runner,
                signer,
                "withdrawAndFail",
                Wei::zero(),
                &[ethabi::Token::Bool(flag)],
            )
            .unwrap(),
        )
    }

    pub fn try_withdraw_and_avoid_fail(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        Self::submit_result_to_success_or_revert(
            self.call_function(
                runner,
                signer,
                "tryWithdrawAndAvoidFail",
                Wei::zero(),
                &[ethabi::Token::Bool(flag)],
            )
            .unwrap(),
        )
    }

    pub fn try_withdraw_and_avoid_fail_and_succeed(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        flag: bool,
    ) -> Result<SubmitResult, Revert> {
        Self::submit_result_to_success_or_revert(
            self.call_function(
                runner,
                signer,
                "tryWithdrawAndAvoidFailAndSucceed",
                Wei::zero(),
                &[ethabi::Token::Bool(flag)],
            )
            .unwrap(),
        )
    }

    pub fn withdraw_eth(
        &self,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
        is_to_near: bool,
        amount: Wei,
    ) -> Result<SubmitResult, Revert> {
        Self::submit_result_to_success_or_revert(if is_to_near {
            self.call_function(
                runner,
                signer,
                "withdrawEthToNear",
                amount,
                &[ethabi::Token::Bytes(DEST_ACCOUNT.as_bytes().to_vec())],
            )
            .unwrap()
        } else {
            self.call_function(
                runner,
                signer,
                "withdrawEthToEthereum",
                amount,
                &[ethabi::Token::Address(DEST_ADDRESS.raw())],
            )
            .unwrap()
        })
    }
}

#[derive(Debug)]
pub(crate) struct Revert(Vec<u8>);
