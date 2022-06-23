use crate::test_utils;
use borsh::BorshSerialize;

#[test]
fn test_exploit_fix() {
    let (mut runner, mut signer, _) = crate::tests::sanity::initialize_transfer();

    let constructor = test_utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "echo.sol",
        "Echo",
    );

    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    let eth_custodian_address = if cfg!(feature = "mainnet-test") {
        "6bfad42cfc4efc96f529d786d643ff4a8b89fa52"
    } else if cfg!(feature = "testnet-test") {
        "84a82bb39c83989d5dc07e1310281923d2544dc2"
    } else {
        panic!("This test requires mainnet-test or testnet-test feature enabled.")
    };
    let target_address = "1111111122222222333333334444444455555555";
    let amount: u64 = 1_000_000;
    let amount_bytes = amount.to_le_bytes();
    let payload = hex::decode(format!(
        "000000{}{}{}",
        hex::encode(amount_bytes),
        target_address,
        eth_custodian_address
    ))
    .unwrap();

    let tx = contract.call_method_with_args("echo", &[ethabi::Token::Bytes(payload)], nonce.into());
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let view_call_args = test_utils::as_view_call(tx, sender);
    let input = view_call_args.try_to_vec().unwrap();

    let (_outcome, maybe_error) = runner.one_shot().call("view", "viewer", input);
    let error_message = format!("{:?}", maybe_error);
    assert!(error_message.contains("ERR_ILLEGAL_RETURN"));
}
