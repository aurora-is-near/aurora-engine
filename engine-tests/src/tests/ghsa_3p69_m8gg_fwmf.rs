use crate::utils;

#[test]
fn test_exploit_fix() {
    utils::load_library();

    let (mut runner, mut signer, _) = crate::tests::sanity::initialize_transfer();

    let constructor = utils::solidity::ContractConstructor::compile_from_source(
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

    let eth_custodian_address = "6bfad42cfc4efc96f529d786d643ff4a8b89fa52";
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
    let sender = utils::address_from_secret_key(&signer.secret_key);
    let view_call_args = utils::as_view_call(tx, sender);
    let input = borsh::to_vec(&view_call_args).unwrap();
    let error = runner.one_shot().call("view", "viewer", input).unwrap_err();

    assert!(
        error.kind.as_bytes().starts_with(b"ERR_ILLEGAL_RETURN"),
        "{error:?}"
    );
}
