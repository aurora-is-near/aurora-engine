use crate::test_utils::{
    self,
    standard_precompiles::{PrecompilesConstructor, PrecompilesContract},
};
use crate::types::Wei;
use secp256k1::SecretKey;

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

#[test]
fn standard_precompiles() {
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        test_utils::address_from_secret_key(&source_account),
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );

    let constructor = PrecompilesConstructor::load();
    let contract = PrecompilesContract(runner.deploy_contract(
        &source_account,
        |c| c.deploy(INITIAL_NONCE.into()),
        constructor,
    ));

    let test_all_tx = contract.call_method("test_all", (INITIAL_NONCE + 1).into());
    let outcome = runner
        .submit_transaction(&source_account, test_all_tx)
        .unwrap();

    // status == false indicates failure
    if !outcome.status {
        panic!("{}", String::from_utf8_lossy(&outcome.result))
    }
}

#[test]
fn precompile_late_promise_create() {
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        test_utils::address_from_secret_key(&source_account),
        INITIAL_BALANCE.into(),
        INITIAL_NONCE.into(),
    );

    let constructor = PrecompilesConstructor::load();
    let contract = PrecompilesContract(runner.deploy_contract(
        &source_account,
        |c| c.deploy(INITIAL_NONCE.into()),
        constructor,
    ));

    let test_all_tx = contract.call_method("test_all", (INITIAL_NONCE + 1).into());
    let outcome = runner
        .submit_transaction(&source_account, test_all_tx)
        .unwrap();

    // status == false indicates failure
    if !outcome.status {
        panic!("{}", String::from_utf8_lossy(&outcome.result))
    }
}
