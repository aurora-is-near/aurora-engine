use crate::test_utils::{origin, AuroraRunner, Signer};

use crate::prelude::U256;
use crate::test_utils;
use crate::test_utils::exit_precompile::{Tester, TesterConstructor};
use crate::test_utils::solidity;
use crate::transaction::EthTransaction;
use ethabi::Address;
use near_crypto::SecretKey;
use std::path::{Path, PathBuf};

fn setup_test() -> (AuroraRunner, Signer, [u8; 20], Tester) {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token(&"tt.testnet".to_string());
    let mut signer = test_utils::Signer::random();

    let tester_ctr = TesterConstructor::load();
    let nonce = signer.use_nonce();

    let tester: Tester = runner
        .deploy_contract(
            &signer.secret_key,
            |ctr| ctr.deploy(nonce, token.into()),
            tester_ctr,
        )
        .into();

    runner.mint(
        token,
        tester.contract.address.into(),
        1_000_000_000,
        origin(),
    );

    (runner, signer, token, tester)
}

#[test]
fn hello_world_solidity() {
    let (mut runner, mut signer, token, tester) = setup_test();

    let name = "AuroraG".to_string();
    let expected = format!("Hello {}!", name);

    let result = tester.hello_world(&mut runner, &mut signer, name).unwrap();
    assert_eq!(expected, result);
}

#[test]
fn withdraw() {
    let (mut runner, mut signer, token, tester) = setup_test();

    let test_data = vec![
        (true, "tt.testnet.ft_transfer"),
        (false, "tt.testnet.withdraw"),
    ];

    for (flag, expected) in test_data {
        assert!(tester.withdraw(&mut runner, &mut signer, flag).is_ok());
        // One promise is scheduled
        assert_eq!(runner.previous_logs, vec![expected.to_string()]);
    }
}

#[test]
fn withdraw_and_fail() {
    let (mut runner, mut signer, token, tester) = setup_test();

    for flag in vec![true, false] {
        assert!(tester
            .withdraw_and_fail(&mut runner, &mut signer, flag)
            .is_err());

        // No promise is scheduled
        assert!(runner.previous_logs.is_empty());
    }
}

#[test]
fn try_withdraw_and_avoid_fail() {
    let (mut runner, mut signer, token, tester) = setup_test();

    for flag in vec![true, false] {
        assert!(tester
            .try_withdraw_and_avoid_fail(&mut runner, &mut signer, flag)
            .is_ok());

        // No promise is scheduled
        assert!(runner.previous_logs.is_empty());
    }
}

#[test]
fn try_withdraw_and_avoid_fail_and_succeed() {
    let (mut runner, mut signer, token, tester) = setup_test();

    let test_data = vec![
        (true, "tt.testnet.ft_transfer"),
        (false, "tt.testnet.withdraw"),
    ];

    for (flag, expected) in test_data {
        assert!(tester
            .try_withdraw_and_avoid_fail_and_succeed(&mut runner, &mut signer, flag)
            .is_ok());
        // One promise is scheduled
        assert_eq!(runner.previous_logs, vec![expected.to_string()]);
    }
}
