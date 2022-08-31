use crate::prelude::Wei;
use crate::test_utils::{
    self,
    standard_precompiles::{PrecompilesConstructor, PrecompilesContract},
    AuroraRunner, ExecutionProfile, Signer,
};

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

fn precompile_execution_profile(method: &str) -> ExecutionProfile {
    let (mut runner, mut signer, contract) = initialize();
    let (_result, profile) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| contract.call_method(method, nonce))
        .unwrap();
    profile
}

#[test]
fn test_standard_precompiles() {
    let (mut runner, mut signer, contract) = initialize();

    let outcome = runner
        .submit_with_signer(&mut signer, |nonce| contract.call_method("test_all", nonce))
        .unwrap();

    test_utils::panic_on_fail(outcome.status);
}

#[test]
fn profile_ecrecover() {
    let profile = precompile_execution_profile("test_ecrecover");
    test_utils::assert_gas_bound(profile.all_gas(), 6);
}

#[test]
fn profile_sha256() {
    let profile = precompile_execution_profile("test_sha256");
    test_utils::assert_gas_bound(profile.all_gas(), 5);
}

#[test]
fn profile_ripemd160() {
    let profile = precompile_execution_profile("test_ripemd160");
    test_utils::assert_gas_bound(profile.all_gas(), 5);
}

#[test]
fn profile_identity() {
    let profile = precompile_execution_profile("test_identity");
    test_utils::assert_gas_bound(profile.all_gas(), 5);
}

#[test]
fn profile_modexp() {
    let profile = precompile_execution_profile("test_modexp");
    test_utils::assert_gas_bound(profile.all_gas(), 8);
}

#[test]
fn profile_ecadd() {
    let profile = precompile_execution_profile("test_ecadd");
    test_utils::assert_gas_bound(profile.all_gas(), 5);
}

#[test]
fn profile_ecmul() {
    let profile = precompile_execution_profile("test_ecmul");
    test_utils::assert_gas_bound(profile.all_gas(), 6);
}

#[test]
fn profile_ecpair() {
    let profile = precompile_execution_profile("test_ecpair");
    test_utils::assert_gas_bound(profile.all_gas(), 102);
}

#[test]
fn profile_blake2f() {
    let profile = precompile_execution_profile("test_blake2f");
    test_utils::assert_gas_bound(profile.all_gas(), 6);
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
