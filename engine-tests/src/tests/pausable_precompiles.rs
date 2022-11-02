use crate::prelude::{Address, U256};
use crate::test_utils::exit_precompile::{Tester, TesterConstructor};
use crate::test_utils::{
    self, origin, AuroraRunner, Signer, PAUSED_PRECOMPILES, PAUSE_PRECOMPILES, RESUME_PRECOMPILES,
};
use aurora_engine::parameters::{PausePrecompilesCallArgs, TransactionStatus};
use aurora_engine_types::types::Wei;
use borsh::BorshSerialize;
use near_vm_errors::{FunctionCallError, HostError};
use near_vm_runner::VMError;

const EXIT_TO_ETHEREUM_FLAG: u32 = 0b10;
const CALLED_ACCOUNT_ID: &str = "aurora";

#[test]
fn test_paused_precompile_is_shown_when_viewing() {
    let mut runner = test_utils::deploy_evm();

    let call_args = PausePrecompilesCallArgs {
        paused_mask: EXIT_TO_ETHEREUM_FLAG,
    };

    let mut input: Vec<u8> = Vec::new();
    call_args.serialize(&mut input).unwrap();

    let _ = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input.clone());
    let (outcome, error) = runner.call(PAUSED_PRECOMPILES, CALLED_ACCOUNT_ID, Vec::new());

    assert!(error.is_none(), "{:?}", error);

    let output: Vec<u8> = outcome
        .as_ref()
        .unwrap()
        .return_data
        .clone()
        .as_value()
        .unwrap();

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

    let mut input: Vec<u8> = Vec::new();
    call_args.serialize(&mut input).unwrap();

    let _ = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input.clone());
    let is_to_near = false;
    let result = tester.withdraw(&mut runner, &mut signer, is_to_near);

    assert!(result.is_err(), "{:?}", result);

    let error = result.unwrap_err();
    match &error {
        VMError::FunctionCallError(fn_error) => match fn_error {
            FunctionCallError::HostError(err) => match err {
                HostError::GuestPanic { panic_msg } => assert_eq!(panic_msg, "ERR_PAUSED"),
                other => panic!("Unexpected host error {:?}", other),
            },
            other => panic!("Unexpected function call error {:?}", other),
        },
        other => panic!("Unexpected VM error {:?}", other),
    };
}

#[test]
fn test_executing_paused_and_then_resumed_precompile_succeeds() {
    let (mut runner, mut signer, _, tester) = setup_test();

    let call_args = PausePrecompilesCallArgs {
        paused_mask: EXIT_TO_ETHEREUM_FLAG,
    };

    let mut input: Vec<u8> = Vec::new();
    call_args.serialize(&mut input).unwrap();

    let _ = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input.clone());
    let _ = runner.call(RESUME_PRECOMPILES, CALLED_ACCOUNT_ID, input);
    let is_to_near = false;
    let result = tester
        .withdraw(&mut runner, &mut signer, is_to_near)
        .unwrap();

    let number = match result.status {
        TransactionStatus::Succeed(number) => U256::from(number.as_slice()),
        _ => panic!("Unexpected status {:?}", result),
    };

    assert_eq!(number, U256::zero());
}

#[test]
fn test_resuming_precompile_does_not_throw_error() {
    let mut runner = test_utils::deploy_evm();

    let call_args = PausePrecompilesCallArgs { paused_mask: 0b1 };

    let mut input: Vec<u8> = Vec::new();
    call_args.serialize(&mut input).unwrap();

    let (_, error) = runner.call(RESUME_PRECOMPILES, CALLED_ACCOUNT_ID, input);

    assert!(error.is_none(), "{:?}", error);
}

#[test]
fn test_pausing_precompile_does_not_throw_error() {
    let mut runner = test_utils::deploy_evm();

    let call_args = PausePrecompilesCallArgs { paused_mask: 0b1 };

    let mut input: Vec<u8> = Vec::new();
    call_args.serialize(&mut input).unwrap();

    let (_, error) = runner.call(PAUSE_PRECOMPILES, CALLED_ACCOUNT_ID, input);

    assert!(error.is_none(), "{:?}", error);
}

fn setup_test() -> (AuroraRunner, Signer, Address, Tester) {
    const INITIAL_NONCE: u64 = 0;

    let mut runner = test_utils::deploy_evm();
    let token = runner.deploy_erc20_token("tt.testnet");
    let mut signer = Signer::random();
    runner.create_address(
        test_utils::address_from_secret_key(&signer.secret_key),
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

    runner.mint(token, tester.contract.address, 1_000_000_000, origin());

    (runner, signer, token, tester)
}
