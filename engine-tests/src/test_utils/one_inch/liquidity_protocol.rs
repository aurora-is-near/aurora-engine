use crate::prelude::parameters::SubmitResult;
use crate::prelude::{Address, U256};
use crate::test_utils::{self, solidity, ExecutionProfile};
use std::path::PathBuf;
use std::sync::Once;

static DOWNLOAD_ONCE: Once = Once::new();
static COMPILE_ONCE: Once = Once::new();

pub(crate) struct Helper<'a> {
    pub runner: &'a mut test_utils::AuroraRunner,
    pub signer: &'a mut test_utils::Signer,
}

impl<'a> Helper<'a> {
    pub(crate) fn create_mooniswap_deployer(
        &mut self,
    ) -> (SubmitResult, ExecutionProfile, PoolDeployer) {
        let artifacts_path = download_and_compile_solidity_sources();
        let deployer_constructor =
            test_utils::solidity::ContractConstructor::compile_from_extended_json(
                artifacts_path.join("MooniswapDeployer.sol/MooniswapDeployer.json"),
            );
        let data = deployer_constructor.code;
        let abi = deployer_constructor.abi;

        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(self.signer, |nonce| {
                crate::prelude::transactions::legacy::TransactionLegacy {
                    nonce,
                    gas_price: Default::default(),
                    gas_limit: u64::MAX.into(),
                    to: None,
                    value: Default::default(),
                    data,
                }
            })
            .unwrap();

        let deployer_address =
            Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();
        let deployer = PoolDeployer(solidity::DeployedContract {
            abi,
            address: deployer_address,
        });

        (result, profile, deployer)
    }

    pub(crate) fn create_pool_factory(
        &mut self,
        pool_deployer: &PoolDeployer,
    ) -> (SubmitResult, ExecutionProfile, PoolFactory) {
        let artifacts_path = download_and_compile_solidity_sources();
        let constructor = test_utils::solidity::ContractConstructor::compile_from_extended_json(
            artifacts_path.join("MooniswapFactory.sol/MooniswapFactory.json"),
        );

        let signer_address = test_utils::address_from_secret_key(&self.signer.secret_key);
        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(self.signer, |nonce| {
                constructor.deploy_with_args(
                    nonce,
                    &[
                        ethabi::Token::Address(signer_address.raw()),
                        ethabi::Token::Address(pool_deployer.0.address.raw()),
                        ethabi::Token::Address(signer_address.raw()),
                    ],
                )
            })
            .unwrap();

        let address = Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();
        let pool_factory = PoolFactory(constructor.deployed_at(address));

        (result, profile, pool_factory)
    }

    pub(crate) fn create_pool(
        &mut self,
        pool_factory: &PoolFactory,
        token_a: Address,
        token_b: Address,
    ) -> (SubmitResult, ExecutionProfile, Pool) {
        let artifacts_path = download_and_compile_solidity_sources();
        let constructor = test_utils::solidity::ContractConstructor::compile_from_extended_json(
            artifacts_path.join("Mooniswap.sol/Mooniswap.json"),
        );

        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(self.signer, |nonce| {
                pool_factory.0.call_method_with_args(
                    "deploy",
                    &[
                        ethabi::Token::Address(token_a.raw()),
                        ethabi::Token::Address(token_b.raw()),
                    ],
                    nonce,
                )
            })
            .unwrap();

        let address =
            Address::try_from_slice(&test_utils::unwrap_success_slice(&result)[12..32]).unwrap();
        let pool = Pool(constructor.deployed_at(address));

        (result, profile, pool)
    }

    pub(crate) fn create_erc20(&mut self, name: &str, symbol: &str) -> test_utils::erc20::ERC20 {
        let constructor = test_utils::erc20::ERC20Constructor::load();
        let nonce = self.signer.use_nonce();
        test_utils::erc20::ERC20(self.runner.deploy_contract(
            &self.signer.secret_key,
            |c| c.deploy(name, symbol, nonce.into()),
            constructor,
        ))
    }

    pub(crate) fn mint_erc20_tokens(
        &mut self,
        token: &test_utils::erc20::ERC20,
        dest: Address,
    ) -> SubmitResult {
        let result = self
            .runner
            .submit_with_signer(self.signer, |nonce| {
                token.mint(dest, 1_000_000.into(), nonce)
            })
            .unwrap();
        assert!(result.status.is_ok());
        result
    }

    pub(crate) fn approve_erc20_tokens(
        &mut self,
        token: &test_utils::erc20::ERC20,
        dest: Address,
    ) -> SubmitResult {
        let result = self
            .runner
            .submit_with_signer(self.signer, |nonce| {
                token.approve(dest, 1_000_000.into(), nonce)
            })
            .unwrap();
        assert!(result.status.is_ok());
        result
    }

    pub(crate) fn pool_deposit(
        &mut self,
        pool: &Pool,
        args: DepositArgs,
    ) -> (SubmitResult, ExecutionProfile) {
        self.pool_call(
            pool,
            "deposit",
            &[
                ethabi::Token::FixedArray(vec![
                    ethabi::Token::Uint(args.max_token_a),
                    ethabi::Token::Uint(args.max_token_b),
                ]),
                ethabi::Token::FixedArray(vec![
                    ethabi::Token::Uint(args.min_token_a),
                    ethabi::Token::Uint(args.min_token_b),
                ]),
            ],
        )
    }

    pub(crate) fn pool_swap(
        &mut self,
        pool: &Pool,
        args: SwapArgs,
    ) -> (SubmitResult, ExecutionProfile) {
        self.pool_call(
            pool,
            "swap",
            &[
                ethabi::Token::Address(args.src_token.raw()),
                ethabi::Token::Address(args.dst_token.raw()),
                ethabi::Token::Uint(args.amount),
                ethabi::Token::Uint(args.min_amount),
                ethabi::Token::Address(args.referral.raw()),
            ],
        )
    }

    pub(crate) fn pool_withdraw(
        &mut self,
        pool: &Pool,
        args: WithdrawArgs,
    ) -> (SubmitResult, ExecutionProfile) {
        self.pool_call(
            pool,
            "withdraw",
            &[
                ethabi::Token::Uint(args.amount),
                ethabi::Token::Array(vec![
                    ethabi::Token::Uint(args.min_token_a),
                    ethabi::Token::Uint(args.min_token_b),
                ]),
            ],
        )
    }

    fn pool_call(
        &mut self,
        pool: &Pool,
        method_name: &str,
        args: &[ethabi::Token],
    ) -> (SubmitResult, ExecutionProfile) {
        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(self.signer, |nonce| {
                pool.0.call_method_with_args(method_name, args, nonce)
            })
            .unwrap();
        assert!(result.status.is_ok());
        (result, profile)
    }
}

pub(crate) struct PoolDeployer(solidity::DeployedContract);

pub(crate) struct PoolFactory(solidity::DeployedContract);

pub(crate) struct Pool(solidity::DeployedContract);

pub(crate) struct DepositArgs {
    pub min_token_a: U256,
    pub min_token_b: U256,
    pub max_token_a: U256,
    pub max_token_b: U256,
}

pub(crate) struct SwapArgs {
    pub src_token: Address,
    pub dst_token: Address,
    pub amount: U256,
    pub min_amount: U256,
    pub referral: Address,
}

pub(crate) struct WithdrawArgs {
    pub amount: U256,
    pub min_token_a: U256,
    pub min_token_b: U256,
}

impl Pool {
    pub fn address(&self) -> Address {
        self.0.address
    }
}

fn download_and_compile_solidity_sources() -> PathBuf {
    super::download_and_compile_solidity_sources(
        "liquidity-protocol",
        &DOWNLOAD_ONCE,
        &COMPILE_ONCE,
    )
}
