mod sim_tests {
    use crate::prelude::{Wei, U256};
    use crate::test_utils;
    use crate::tests::state_migration::{deploy_evm, AuroraAccount};
    use aurora_engine::parameters::SubmitResult;
    use aurora_engine_types::types::Address;
    use near_sdk_sim::UserAccount;

    const ASYNC_AURORA_PATH: &str =
        "../../etc/async-aurora/target/wasm32-unknown-unknown/release/async_aurora.wasm";
    const RECEIVER_PATH: &str = "src/tests/res/async_aurora_test.wasm";
    const RECEIVER_ACCOUNT: &str = "receiver_contract";
    const CALLER_GAS: &str = "20000000000000";

    struct TestContext {
        signer: test_utils::Signer,
        async_aurora: near_sdk_sim::UserAccount,
        caller: Address,
        receiver: near_sdk_sim::UserAccount,
        aurora: AuroraAccount,
        chain_id: u64,
    }

    fn deploy_receiver_contract(account_id: &str, aurora: &AuroraAccount) -> UserAccount {
        let contract_bytes = std::fs::read(RECEIVER_PATH).unwrap();

        let contract_account = aurora.user.deploy(
            &contract_bytes,
            account_id.parse().unwrap(),
            5 * near_sdk_sim::STORAGE_AMOUNT,
        );

        contract_account
    }

    fn get_current_receiver_value(account: &near_sdk_sim::UserAccount) -> i128 {
        account
            .call(
                account.account_id(),
                "get_value",
                &[],
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .unwrap_json_value()
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn build_input(str_selector: &str, inputs: &[ethabi::Token]) -> Vec<u8> {
        use sha3::Digest;
        let sel = sha3::Keccak256::digest(str_selector.as_bytes()).to_vec()[..4].to_vec();
        let inputs = ethabi::encode(inputs);
        [sel.as_slice(), inputs.as_slice()].concat().to_vec()
    }

    fn test_common() -> TestContext {
        // 1. Deploy Aurora
        let aurora = deploy_evm();
        let chain_id = test_utils::AuroraRunner::default().chain_id;
        let signer = test_utils::Signer::random();

        // 2. Deploy receiver
        let receiver = deploy_receiver_contract(RECEIVER_ACCOUNT, &aurora);

        assert_eq!(get_current_receiver_value(&receiver), 0);

        // 3. Deploy caller

        let constructor = test_utils::solidity::ContractConstructor::compile_from_source(
            "src/tests/res",
            "target/solidity_build",
            "TestAsync.sol",
            "TestAsync",
        );

        let submit_result: SubmitResult =
            aurora.call("deploy_code", &constructor.code).unwrap_borsh();
        let caller = Address::try_from_slice(&test_utils::unwrap_success(submit_result)).unwrap();

        let async_aurora = aurora.user.deploy(
            &std::fs::read(ASYNC_AURORA_PATH).unwrap(),
            aurora.contract.account_id(),
            near_sdk_sim::STORAGE_AMOUNT,
        );

        TestContext {
            signer,
            async_aurora,
            caller,
            receiver,
            aurora,
            chain_id,
        }
    }

    fn submit(context: &TestContext, input: Vec<u8>) {
        let tx = test_utils::create_eth_transaction(
            Some(context.caller),
            Wei::new_u64(0),
            input,
            Some(context.chain_id),
            &context.signer.secret_key,
        );
        context
            .aurora
            .user
            .call(
                context.async_aurora.account_id(),
                "submit",
                &rlp::encode(&tx),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    fn caller_simple_call(context: &TestContext, method: String, arg: i128) {
        let input = build_input(
            "simpleCall(string,string,int128,uin256)",
            &[
                ethabi::Token::String(context.receiver.account_id().to_string()),
                ethabi::Token::String(method),
                ethabi::Token::Int(U256::from(arg).into()),
                ethabi::Token::String(CALLER_GAS.to_string()),
            ],
        );

        submit(context, input);
    }

    fn caller_then_call(context: &TestContext, method1: String, method2: String, arg: i128) {
        let input = build_input(
            "thenCall(string,string,string,int128,uin256)",
            &[
                ethabi::Token::String(context.receiver.account_id().to_string()),
                ethabi::Token::String(method1),
                ethabi::Token::String(method2),
                ethabi::Token::Int(U256::from(arg).into()),
                ethabi::Token::String(CALLER_GAS.to_string()),
            ],
        );

        submit(context, input);
    }

    fn caller_and_then_and_call(
        context: &TestContext,
        method1: String,
        method2: String,
        method3: String,
        method4: String,
        arg: i128,
    ) {
        let input = build_input(
            "andThenAndCall(string,string,string,string,string,int128,uin256)",
            &[
                ethabi::Token::String(context.receiver.account_id().to_string()),
                ethabi::Token::String(method1),
                ethabi::Token::String(method2),
                ethabi::Token::String(method3),
                ethabi::Token::String(method4),
                ethabi::Token::Int(U256::from(arg).into()),
                ethabi::Token::String(CALLER_GAS.to_string()),
            ],
        );

        submit(context, input);
    }

    #[test]
    fn test_aurora_async_simple() {
        let context = test_common();
        caller_simple_call(&context, "add".to_string(), 10);
        assert_eq!(get_current_receiver_value(&context.receiver), 10);
        caller_simple_call(&context, "sub".to_string(), 10);
        assert_eq!(get_current_receiver_value(&context.receiver), 0);

        caller_then_call(&context, "add".to_string(), "mul".to_string(), 5);
        assert_eq!(get_current_receiver_value(&context.receiver), 25);
        caller_then_call(&context, "sub".to_string(), "mul".to_string(), 5);
        assert_eq!(get_current_receiver_value(&context.receiver), 100);
        caller_then_call(&context, "sub".to_string(), "sub".to_string(), 50);
        assert_eq!(get_current_receiver_value(&context.receiver), 0);

        caller_and_then_and_call(
            &context,
            "add".to_string(),
            "mul".to_string(),
            "add".to_string(),
            "add".to_string(),
            5,
        );
        assert_eq!(get_current_receiver_value(&context.receiver), 35);
    }
}
