use crate::prelude::parameters::SubmitResult;
use crate::prelude::{Wei, U256};
use crate::test_utils::one_inch::liquidity_protocol;
use crate::test_utils::{self, assert_gas_bound};
use borsh::BorshDeserialize;
use libsecp256k1::SecretKey;
use near_vm_logic::VMOutcome;
use std::sync::Once;

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000_000);
const INITIAL_NONCE: u64 = 0;

static DOWNLOAD_ONCE: Once = Once::new();
static COMPILE_ONCE: Once = Once::new();

#[test]
fn test_1inch_liquidity_protocol() {
    let (mut runner, mut source_account) = initialize();
    let mut helper = liquidity_protocol::Helper {
        runner: &mut runner,
        signer: &mut source_account,
    };

    let (result, profile, deployer_address) = helper.create_mooniswap_deployer();
    assert!(result.gas_used >= 5_100_000); // more than 5.1M EVM gas used
    assert_gas_bound(profile.all_gas(), 10); // less than 10 NEAR Tgas used

    let (result, profile, pool_factory) = helper.create_pool_factory(&deployer_address);
    assert!(result.gas_used >= 2_800_000); // more than 2.8M EVM gas used
    assert_gas_bound(profile.all_gas(), 9); // less than 9 NEAR Tgas used

    // create some ERC-20 tokens to have a liquidity pool for
    let signer_address = test_utils::address_from_secret_key(&helper.signer.secret_key);
    let token_a = helper.create_erc20("TokenA", "AAA");
    let token_b = helper.create_erc20("TokenB", "BBB");
    helper.mint_erc20_tokens(&token_a, signer_address);
    helper.mint_erc20_tokens(&token_b, signer_address);

    let (result, profile, pool) =
        helper.create_pool(&pool_factory, token_a.0.address, token_b.0.address);
    assert!(result.gas_used >= 4_500_000); // more than 4.5M EVM gas used
    assert_gas_bound(profile.all_gas(), 20);

    // Approve giving ERC-20 tokens to the pool
    helper.approve_erc20_tokens(&token_a, pool.address());
    helper.approve_erc20_tokens(&token_b, pool.address());

    // I don't understand why this is needed but for some reason the 1inch
    // contract divides by zero unless I mess with the time.
    helper.runner.context.block_timestamp += 10_000_000 * 1_000_000_000;
    let (result, profile) = helper.pool_deposit(
        &pool,
        liquidity_protocol::DepositArgs {
            min_token_a: U256::zero(),
            min_token_b: U256::zero(),
            max_token_a: 10_000.into(),
            max_token_b: 10_000.into(),
        },
    );
    assert!(result.gas_used >= 302_000); // more than 302k EVM gas used
    assert_gas_bound(profile.all_gas(), 24);

    // Same here
    helper.runner.context.block_timestamp += 10_000_000 * 1_000_000_000;
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
    assert!(result.gas_used >= 210_000); // more than 210k EVM gas used
    assert_gas_bound(profile.all_gas(), 25);

    let (result, profile) = helper.pool_withdraw(
        &pool,
        liquidity_protocol::WithdrawArgs {
            amount: 100.into(),
            min_token_a: U256::one(),
            min_token_b: U256::one(),
        },
    );
    assert!(result.gas_used >= 150_000); // more than 150k EVM gas used
    assert_gas_bound(profile.all_gas(), 21);
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

    // more than 3.5 million Ethereum gas used
    assert!(result.gas_used > 3_500_000);
    // less than 10 NEAR Tgas used
    assert_gas_bound(profile.all_gas(), 10);
    // at least 45% of which is from wasm execution
    let wasm_fraction = 100 * profile.wasm_gas() / profile.all_gas();
    assert!(
        50 <= wasm_fraction && wasm_fraction <= 60,
        "{}% is not between 50% and 60%",
        wasm_fraction
    );
}

fn deploy_1_inch_limit_order_contract(
    runner: &mut test_utils::AuroraRunner,
    signer: &mut test_utils::Signer,
) -> VMOutcome {
    let artifacts_path = test_utils::one_inch::download_and_compile_solidity_sources(
        "limit-order-protocol",
        &DOWNLOAD_ONCE,
        &COMPILE_ONCE,
    );
    let contract_path = artifacts_path.join("LimitOrderProtocol.sol/LimitOrderProtocol.json");
    let constructor =
        test_utils::solidity::ContractConstructor::compile_from_extended_json(contract_path);

    let nonce = signer.use_nonce();
    let deploy_tx = crate::prelude::transactions::legacy::TransactionLegacy {
        nonce: nonce.into(),
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
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
