use crate::prelude::{parameters::SubmitResult, vec, Address, Wei, H256, U256};
use crate::test_utils::{origin, AuroraRunner, Signer};

use crate::test_utils;
use crate::test_utils::exit_precompile::{Tester, TesterConstructor, DEST_ACCOUNT, DEST_ADDRESS};

fn setup_test() -> (AuroraRunner, Signer, Address, Tester) {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token("tt.testnet");
    let mut signer = test_utils::Signer::random();
    runner.create_address(
        test_utils::address_from_secret_key(&signer.secret_key),
        Wei::from_eth(1.into()).unwrap(),
        U256::zero(),
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
    let (mut runner, mut signer, token, tester) = setup_test();

    let test_data = vec![
        (true, "call_contract tt.testnet.ft_transfer"),
        (false, "call_contract tt.testnet.withdraw"),
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
        let exit_events = parse_exit_events(withdraw_result, &schema);

        // One exit event
        assert_eq!(exit_events.len(), 1);

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
            let address = Address::try_from_slice(&address).unwrap();
            ethabi::LogParam {
                name: "dest".to_string(),
                value: ethabi::Token::Address(address.raw()),
            }
        };
        let expected_event = vec![
            ethabi::LogParam {
                name: "sender".to_string(),
                value: ethabi::Token::Address(token.raw()),
            },
            ethabi::LogParam {
                name: "erc20_address".to_string(),
                value: ethabi::Token::Address(token.raw()),
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
        (true, "call_contract tt.testnet.ft_transfer"),
        (false, "call_contract tt.testnet.withdraw"),
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
        (true, "call_contract tt.testnet.ft_transfer"),
        (false, "call_contract tt.testnet.withdraw"),
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
        (true, "call_contract tt.testnet.ft_transfer"),
        (false, "call_contract tt.testnet.withdraw"),
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

#[test]
fn withdraw_eth() {
    let (mut runner, mut signer, _token, tester) = setup_test();
    let amount = Wei::new_u64(10);

    // exit to NEAR
    let result = tester
        .withdraw_eth(&mut runner, &mut signer, true, amount)
        .unwrap();
    let dest = aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(
        DEST_ACCOUNT.to_string(),
    )]));
    let schema = aurora_engine_precompiles::native::events::exit_to_near_schema();
    let mut expected_event = vec![
        ethabi::LogParam {
            name: "sender".to_string(),
            value: ethabi::Token::Address(tester.contract.address.raw()),
        },
        ethabi::LogParam {
            name: "erc20_address".to_string(),
            value: ethabi::Token::Address(
                aurora_engine_precompiles::native::events::ETH_ADDRESS.raw(),
            ),
        },
        ethabi::LogParam {
            name: "dest".to_string(),
            value: ethabi::Token::FixedBytes(dest.as_bytes().to_vec()),
        },
        ethabi::LogParam {
            name: "amount".to_string(),
            value: ethabi::Token::Uint(amount.raw()),
        },
    ];
    let exit_events = parse_exit_events(result, &schema);

    assert!(exit_events.len() == 1);
    assert_eq!(&expected_event, &exit_events[0].params);

    // exit to ethereum
    let amount = Wei::new_u64(42);
    let result = tester
        .withdraw_eth(&mut runner, &mut signer, false, amount)
        .unwrap();
    expected_event[2] = ethabi::LogParam {
        name: "dest".to_string(),
        value: ethabi::Token::Address(DEST_ADDRESS.raw()),
    };
    expected_event[3] = ethabi::LogParam {
        name: "amount".to_string(),
        value: ethabi::Token::Uint(amount.raw()),
    };
    let schema = aurora_engine_precompiles::native::events::exit_to_eth_schema();
    let exit_events = parse_exit_events(result, &schema);

    assert!(exit_events.len() == 1);
    assert_eq!(&expected_event, &exit_events[0].params);
}

fn parse_exit_events(result: SubmitResult, schema: &ethabi::Event) -> Vec<ethabi::Log> {
    let signature = schema.signature();
    result
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
        .collect()
}
