use crate::prelude::{Address, U256};
use crate::utils::solidity::exit_precompile::{Tester, TesterConstructor};
use crate::utils::{
    self, AuroraRunner, Signer, DEFAULT_AURORA_ACCOUNT_ID, PAUSED_PRECOMPILES, PAUSE_PRECOMPILES,
    RESUME_PRECOMPILES,
};
use aurora_engine::engine::EngineErrorKind;
use aurora_engine::parameters::{PausePrecompilesCallArgs, TransactionStatus};
use aurora_engine_types::borsh::BorshSerialize;
use aurora_engine_types::types::Wei;

const EXIT_TO_ETHEREUM_FLAG: u32 = 0b10;
const CALLED_ACCOUNT_ID: &str = "aurora";

#[test]
fn test_paused_precompile_is_shown_when_viewing() {
    let mut runner = utils::deploy_runner();

    let call_args = PausePrecompilesCallArgs {
        paused_mask: EXIT_TO_ETHEREUM_FLAG,
    };
    let input = call_args.try_to_vec().unwrap();

    let _res = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input);
    let result = runner
        .one_shot()
        .call(PAUSED_PRECOMPILES, CALLED_ACCOUNT_ID, Vec::new())
        .unwrap();
    let output = result.return_data.as_value().unwrap();
    let actual_paused_precompiles = u32::from_le_bytes(output.as_slice().try_into().unwrap());
    let expected_paused_precompiles = EXIT_TO_ETHEREUM_FLAG;

    assert_eq!(expected_paused_precompiles, actual_paused_precompiles);
}

#[test]
fn test_executing_paused_precompile_throws_error() {
    let (mut runner, mut signer, _, tester) = setup_test();

    let call_args = PausePrecompilesCallArgs {
        paused_mask: EXIT_TO_ETHEREUM_FLAG,
    };
    let input = call_args.try_to_vec().unwrap();

    let _res = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input);
    let is_to_near = false;
    let error = tester
        .withdraw(&mut runner, &mut signer, is_to_near)
        .unwrap_err();

    assert!(matches!(
        error.kind,
        EngineErrorKind::EvmFatal(evm::ExitFatal::Other(e)) if e == "ERR_PAUSED"
    ));
}

#[test]
fn test_executing_paused_and_then_resumed_precompile_succeeds() {
    let (mut runner, mut signer, _, tester) = setup_test();

    let call_args = PausePrecompilesCallArgs {
        paused_mask: EXIT_TO_ETHEREUM_FLAG,
    };
    let input = call_args.try_to_vec().unwrap();

    let _res = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input.clone());
    let _res = runner.call(RESUME_PRECOMPILES, CALLED_ACCOUNT_ID, input);
    let is_to_near = false;
    let result = tester
        .withdraw(&mut runner, &mut signer, is_to_near)
        .unwrap();

    let number = match result.status {
        TransactionStatus::Succeed(number) => U256::from(number.as_slice()),
        _ => panic!("Unexpected status {result:?}"),
    };

    assert_eq!(number, U256::zero());
}

#[test]
fn test_resuming_precompile_does_not_throw_error() {
    let mut runner = utils::deploy_runner();

    let call_args = PausePrecompilesCallArgs { paused_mask: 0b1 };
    let input = call_args.try_to_vec().unwrap();
    let result = runner.call(RESUME_PRECOMPILES, CALLED_ACCOUNT_ID, input);

    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn test_pausing_precompile_does_not_throw_error() {
    let mut runner = utils::deploy_runner();
    let call_args = PausePrecompilesCallArgs { paused_mask: 0b1 };
    let input = call_args.try_to_vec().unwrap();
    let result = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input);

    assert!(result.is_ok(), "{result:?}");
}

fn setup_test() -> (AuroraRunner, Signer, Address, Tester) {
    const INITIAL_NONCE: u64 = 0;

    let mut runner = utils::deploy_runner();
    let token = runner.deploy_erc20_token("tt.testnet");
    let mut signer = Signer::random();
    runner.create_address(
        utils::address_from_secret_key(&signer.secret_key),
        Wei::from_eth(1.into()).unwrap(),
        INITIAL_NONCE.into(),
    );

    let tester_ctr = TesterConstructor::load();
    let nonce = signer.use_nonce();

    let tester: Tester = runner
        .deploy_contract(
            &signer.secret_key,
            |ctr| ctr.deploy(nonce, token),
            tester_ctr,
        )
        .into();

    runner
        .mint(
            token,
            tester.contract.address,
            1_000_000_000,
            DEFAULT_AURORA_ACCOUNT_ID,
        )
        .unwrap();

    (runner, signer, token, tester)
}
