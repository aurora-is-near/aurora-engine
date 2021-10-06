use crate::prelude::Wei;
use crate::test_utils::{
    self,
    standard_precompiles::{PrecompilesConstructor, PrecompilesContract},
    AuroraRunner, Signer,
};

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

#[test]
fn standard_precompiles() {
    let (mut runner, mut signer, contract) = initialize();

    let outcome = runner
        .submit_with_signer(&mut signer, |nonce| contract.call_method("test_all", nonce))
        .unwrap();

    test_utils::panic_on_fail(outcome.status);
}

#[test]
#[ignore]
fn ecpair() {
    let (mut runner, mut signer, contract) = initialize();

    // TODO(#46): This should fit into 200 Tgas; we should not need to increase the limit like this.
    runner.wasm_config.limit_config.max_gas_burnt = u64::MAX;
    let (_result, profile) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            contract.call_method("test_ecpair", nonce)
        })
        .unwrap();

    // Some day this number should be less than 200 Tgas.
    println!("{:?}", profile.all_gas());
    assert!(profile.all_gas() < 200_000_000_000_000);
}

fn initialize() -> (AuroraRunner, Signer, PrecompilesContract) {
    let mut runner = test_utils::deploy_evm();
    let mut signer = Signer::random();
    signer.nonce = INITIAL_NONCE;
    runner.create_address(
        test_utils::address_from_secret_key(&signer.secret_key),
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    );

    let constructor = PrecompilesConstructor::load();
    let nonce = signer.use_nonce();
    let contract = PrecompilesContract(runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy(nonce.into()),
        constructor,
    ));

    (runner, signer, contract)
}
