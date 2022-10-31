use crate::test_utils::{self, standalone};
use aurora_engine::parameters::SubmitResult;

#[test]
fn test_account_id_precompiles() {
    let mut signer = test_utils::Signer::random();
    let mut runner = test_utils::deploy_evm();
    let mut standalone = standalone::StandaloneRunner::default();

    standalone.init_evm();
    runner.standalone_runner = Some(standalone);

    let constructor = test_utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "AccountIds.sol",
        "AccountIds",
    );

    // deploy contract
    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    // check current_account_id is correct
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_without_args("currentAccountId", nonce)
        })
        .unwrap();
    assert_eq!(unwrap_ethabi_string(&result), "aurora");

    // check predecessor_account_id is correct
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_without_args("predecessorAccountId", nonce)
        })
        .unwrap();
    assert_eq!(unwrap_ethabi_string(&result), "some-account.near");

    // confirm the precompile works in view calls too
    let tx = contract.call_method_without_args("predecessorAccountId", 0.into());
    let sender = test_utils::address_from_secret_key(&signer.secret_key);
    let result = runner
        .view_call(test_utils::as_view_call(tx, sender))
        .unwrap();
    assert!(result.is_ok());

    // double check the case where account_id is the full 64 bytes
    let account_id = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    assert_eq!(account_id.len(), 64);
    runner.standalone_runner.as_mut().unwrap().env.block_height += 1000;
    runner
        .standalone_runner
        .as_mut()
        .unwrap()
        .env
        .predecessor_account_id = account_id.parse().unwrap();
    let nonce = signer.use_nonce();
    let tx = contract.call_method_without_args("predecessorAccountId", nonce.into());
    let result = runner
        .standalone_runner
        .as_mut()
        .unwrap()
        .submit_transaction(&signer.secret_key, tx)
        .unwrap();
    assert_eq!(unwrap_ethabi_string(&result), account_id);
}

fn unwrap_ethabi_string(result: &SubmitResult) -> String {
    let bytes = test_utils::unwrap_success_slice(result);
    let mut tokens = ethabi::decode(&[ethabi::ParamType::String], bytes).unwrap();
    tokens.pop().unwrap().into_string().unwrap()
}
