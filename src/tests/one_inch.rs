use crate::parameters::SubmitResult;
use crate::prelude::U256;
use crate::test_utils;
use crate::types::Wei;
use borsh::BorshDeserialize;
use near_vm_logic::VMOutcome;
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000_000);
const INITIAL_NONCE: u64 = 0;

static DOWNLOAD_ONCE: Once = Once::new();
static COMPILE_ONCE: Once = Once::new();

#[test]
fn test_tmp() {
    let (mut runner, mut source_account) = initialize();
    let mut helper = liquidity_protocol::Helper {
        runner: &mut runner,
        signer: &mut source_account,
    };

    let (result, profile, deployer_address) = helper.create_mooniswap_deployer();
    println!("ETH_GAS {}", result.gas_used);
    println!("NEAR_GAS {}", profile.all_gas());

    let (result, profile, pool_factory) = helper.create_pool_factory(&deployer_address);
    println!("ETH_GAS {}", result.gas_used);
    println!("NEAR_GAS {}", profile.all_gas());

    let signer_address = test_utils::address_from_secret_key(&helper.signer.secret_key);
    let token_a = helper.create_erc20("TokenA", "AAA");
    let token_b = helper.create_erc20("TokenB", "BBB");
    helper.mint_erc20_tokens(&token_a, signer_address);
    helper.mint_erc20_tokens(&token_b, signer_address);

    let (result, profile, pool) =
        helper.create_pool(&pool_factory, token_a.0.address, token_b.0.address);
    println!("ETH_GAS {}", result.gas_used);
    println!("NEAR_GAS {}", profile.all_gas());

    helper.approve_erc20_tokens(&token_a, pool.address());
    helper.approve_erc20_tokens(&token_b, pool.address());

    let (result, profile) = helper.pool_deposit(
        &pool,
        liquidity_protocol::DepositArgs {
            min_token_a: U256::zero(),
            min_token_b: U256::zero(),
            max_token_a: 10_000.into(),
            max_token_b: 10_000.into(),
        },
    );
    println!("ETH_GAS {}", result.gas_used);
    println!("NEAR_GAS {}", profile.all_gas());

    let (result, profile) = helper.pool_swap(
        &pool,
        liquidity_protocol::SwapArgs {
            src_token: token_a.0.address,
            dst_token: token_b.0.address,
            amount: 1000.into(),
            min_amount: U256::one(),
            referral: signer_address,
        },
    );
    println!("ETH_GAS {}", result.gas_used);
    println!("NEAR_GAS {}", profile.all_gas());

    let (result, profile) = helper.pool_withdraw(
        &pool,
        liquidity_protocol::WithdrawArgs {
            amount: 100.into(),
            min_token_a: U256::one(),
            min_token_b: U256::one(),
        },
    );
    println!("ETH_GAS {}", result.gas_used);
    println!("NEAR_GAS {}", profile.all_gas());
}

#[test]
fn test_1_inch_limit_order_deploy() {
    // set up Aurora runner and accounts
    let (mut runner, mut source_account) = initialize();

    let outcome = deploy_1_inch_limit_order_contract(&mut runner, &mut source_account);
    let profile = test_utils::ExecutionProfile::new(&outcome);
    let result: SubmitResult =
        SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    // more than 4 million Ethereum gas used
    assert!(result.gas_used > 4_000_000);
    // less than 42 NEAR Tgas used
    assert!(profile.all_gas() < 42_000_000_000_000);
    // at least 70% of which is from wasm execution
    assert!(100 * profile.wasm_gas() / profile.all_gas() > 70);
}

fn deploy_1_inch_limit_order_contract(
    runner: &mut test_utils::AuroraRunner,
    signer: &mut test_utils::Signer,
) -> VMOutcome {
    let contract_path = download_and_compile_solidity_sources();
    let constructor =
        test_utils::solidity::ContractConstructor::compile_from_extended_json(contract_path);

    let nonce = signer.use_nonce();
    let deploy_tx = crate::transaction::LegacyEthTransaction {
        nonce: nonce.into(),
        gas_price: Default::default(),
        gas: u64::MAX.into(),
        to: None,
        value: Default::default(),
        data: constructor.code,
    };
    let tx = test_utils::sign_transaction(deploy_tx, Some(runner.chain_id), &signer.secret_key);

    let (outcome, error) = runner.call(
        test_utils::SUBMIT,
        "any_account.near",
        rlp::encode(&tx).to_vec(),
    );
    assert!(error.is_none());
    outcome.unwrap()
}

fn download_and_compile_solidity_sources() -> PathBuf {
    let sources_dir = Path::new("target").join("limit-order-protocol");
    if !sources_dir.exists() {
        // Contracts not already present, so download them (but only once, even
        // if multiple tests running in parallel saw `contracts_dir` does not exist).
        DOWNLOAD_ONCE.call_once(|| {
            let url = "https://github.com/1inch/limit-order-protocol";
            git2::Repository::clone(url, &sources_dir).unwrap();
        });
    }

    COMPILE_ONCE.call_once(|| {
        // install packages
        let status = Command::new("/usr/bin/env")
            .current_dir(&sources_dir)
            .args(["yarn", "install"])
            .status()
            .unwrap();
        assert!(status.success());

        let hardhat = |command: &str| {
            let status = Command::new("/usr/bin/env")
                .current_dir(&sources_dir)
                .args(["node_modules/hardhat/internal/cli/cli.js", command])
                .status()
                .unwrap();
            assert!(status.success());
        };

        // clean and compile
        hardhat("clean");
        hardhat("compile");
    });

    sources_dir.join("artifacts/contracts/LimitOrderProtocol.sol/LimitOrderProtocol.json")
}

fn initialize() -> (test_utils::AuroraRunner, test_utils::Signer) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let mut signer = test_utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    (runner, signer)
}

mod liquidity_protocol {
    use crate::parameters::SubmitResult;
    use crate::prelude::{Address, U256};
    use crate::test_utils::{self, solidity, ExecutionProfile};
    use std::path::{Path, PathBuf};
    use std::process::Command;
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
                    crate::transaction::LegacyEthTransaction {
                        nonce,
                        gas_price: Default::default(),
                        gas: u64::MAX.into(),
                        to: None,
                        value: Default::default(),
                        data,
                    }
                })
                .unwrap();

            let deployer_address = Address::from_slice(test_utils::unwrap_success_slice(&result));
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
                            ethabi::Token::Address(signer_address),
                            ethabi::Token::Address(pool_deployer.0.address),
                            ethabi::Token::Address(signer_address),
                        ],
                    )
                })
                .unwrap();

            let address = Address::from_slice(test_utils::unwrap_success_slice(&result));
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
                            ethabi::Token::Address(token_a),
                            ethabi::Token::Address(token_b),
                        ],
                        nonce,
                    )
                })
                .unwrap();

            let address = Address::from_slice(&test_utils::unwrap_success_slice(&result)[12..32]);
            let pool = Pool(constructor.deployed_at(address));

            (result, profile, pool)
        }

        pub(crate) fn create_erc20(
            &mut self,
            name: &str,
            symbol: &str,
        ) -> test_utils::erc20::ERC20 {
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
                    ethabi::Token::Address(args.src_token),
                    ethabi::Token::Address(args.dst_token),
                    ethabi::Token::Uint(args.amount),
                    ethabi::Token::Uint(args.min_amount),
                    ethabi::Token::Address(args.referral),
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
        let sources_dir = Path::new("target").join("liquidity-protocol");
        if !sources_dir.exists() {
            // Contracts not already present, so download them (but only once, even
            // if multiple tests running in parallel saw `contracts_dir` does not exist).
            DOWNLOAD_ONCE.call_once(|| {
                let url = "https://github.com/1inch/liquidity-protocol";
                git2::Repository::clone(url, &sources_dir).unwrap();
            });
        }

        COMPILE_ONCE.call_once(|| {
            // install packages
            let status = Command::new("/usr/bin/env")
                .current_dir(&sources_dir)
                .args(["yarn", "install"])
                .status()
                .unwrap();
            assert!(status.success());

            let hardhat = |command: &str| {
                let status = Command::new("/usr/bin/env")
                    .current_dir(&sources_dir)
                    .args(["node_modules/hardhat/internal/cli/cli.js", command])
                    .status()
                    .unwrap();
                assert!(status.success());
            };

            // clean and compile
            hardhat("clean");
            hardhat("compile");
        });

        sources_dir.join("artifacts/contracts")
    }
}
