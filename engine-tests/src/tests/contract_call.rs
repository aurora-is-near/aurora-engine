use crate::prelude::{vec, Address, H256};
use crate::test_utils::{origin, AuroraRunner, Signer};

use crate::test_utils;
use crate::test_utils::exit_precompile::{Tester, TesterConstructor};

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
    let (mut runner, mut signer, _token, tester) = setup_test();

    let name = "AuroraG".to_string();
    let expected = format!("Hello {}!", name);

    let result = tester.hello_world(&mut runner, &mut signer, name);
    assert_eq!(expected, result);
}

#[test]
fn withdraw() {
    let (mut runner, mut signer, _token, tester) = setup_test();

    let test_data = vec![
        (true, "Call contract: tt.testnet.ft_transfer"),
        (false, "Call contract: tt.testnet.withdraw"),
    ];

    for (is_to_near, expected) in test_data {
        let withdraw_result = tester
            .withdraw(&mut runner, &mut signer, is_to_near)
            .unwrap();

        // parse exit events
        let schema = if is_to_near {
            aurora_engine_precompiles::native::events::exit_to_near_schema()
        } else {
            aurora_engine_precompiles::native::events::exit_to_eth_schema()
        };
        let signature = schema.signature();
        let exit_events: Vec<ethabi::Log> = withdraw_result
            .logs
            .into_iter()
            .filter_map(|log| {
                if log.topics.first().unwrap() != &signature.0 {
                    return None;
                }
                Some(
                    schema
                        .parse_log(ethabi::RawLog {
                            topics: log.topics.into_iter().map(H256).collect(),
                            data: log.data,
                        })
                        .unwrap(),
                )
            })
            .collect();

        // One exit event
        assert!(exit_events.len() == 1);

        let dest = if is_to_near {
            // transferred to "target.aurora" (defined in Tester.sol)
            let dest = "target.aurora";
            // need to hash it since it is an indexed value in the log
            let dest = aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(
                dest.to_string(),
            )]));
            ethabi::LogParam {
                name: "dest".to_string(),
                value: ethabi::Token::FixedBytes(dest.as_bytes().to_vec()),
            }
        } else {
            // transferred to 0xE0f5206BBD039e7b0592d8918820024e2a7437b9 (defined in Tester.sol)
            let address = hex::decode("E0f5206BBD039e7b0592d8918820024e2a7437b9").unwrap();
            let address = Address::from_slice(&address);
            ethabi::LogParam {
                name: "dest".to_string(),
                value: ethabi::Token::Address(address),
            }
        };
        let expected_event = vec![
            ethabi::LogParam {
                name: "is_erc20".to_string(),
                value: ethabi::Token::Bool(true),
            },
            dest,
            ethabi::LogParam {
                name: "amount".to_string(),
                value: ethabi::Token::Uint(1.into()),
            },
        ];
        assert_eq!(&expected_event, &exit_events[0].params);

        // One promise is scheduled
        assert!(runner.previous_logs.contains(&expected.to_string()));
    }
}

#[test]
fn withdraw_and_fail() {
    let (mut runner, mut signer, _token, tester) = setup_test();

    let test_data = vec![
        (true, "Call contract: tt.testnet.ft_transfer"),
        (false, "Call contract: tt.testnet.withdraw"),
    ];

    for (flag, not_expected) in test_data {
        assert!(tester
            .withdraw_and_fail(&mut runner, &mut signer, flag)
            .is_err());

        // No promise is scheduled
        assert!(!runner.previous_logs.contains(&not_expected.to_string()));
    }
}

#[test]
fn try_withdraw_and_avoid_fail() {
    let (mut runner, mut signer, _token, tester) = setup_test();

    let test_data = vec![
        (true, "Call contract: tt.testnet.ft_transfer"),
        (false, "Call contract: tt.testnet.withdraw"),
    ];

    for (flag, not_expected) in test_data {
        assert!(tester
            .try_withdraw_and_avoid_fail(&mut runner, &mut signer, flag)
            .is_ok());

        // No promise is scheduled
        assert!(!runner.previous_logs.contains(&not_expected.to_string()));
    }
}

#[test]
fn try_withdraw_and_avoid_fail_and_succeed() {
    let (mut runner, mut signer, _token, tester) = setup_test();

    let test_data = vec![
        (true, "Call contract: tt.testnet.ft_transfer"),
        (false, "Call contract: tt.testnet.withdraw"),
    ];

    for (flag, expected) in test_data {
        println!("{}", flag);
        assert!(tester
            .try_withdraw_and_avoid_fail_and_succeed(&mut runner, &mut signer, flag)
            .is_ok());
        // One promise is scheduled
        println!("{:?} {:?}", runner.previous_logs, expected.to_string());
        assert!(runner.previous_logs.contains(&expected.to_string()));
    }
}
