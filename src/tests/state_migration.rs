use crate::parameters::NewCallArgs;
use crate::prelude::U256;
use crate::test_utils::{self, AuroraRunner};
use crate::types;
use crate::types::Wei;
use borsh::BorshSerialize;
use near_sdk_sim::{ExecutionResult, UserAccount};
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_state_migration() {
    let aurora = deploy_evm();

    // do upgrade
    let upgraded_contract_bytes = contract_bytes();
    aurora
        .call("stage_upgrade", &upgraded_contract_bytes)
        .assert_success();
    aurora.call("deploy_upgrade", &[]).assert_success();

    // upgraded contract as some_new_fancy_function
    let result = aurora.call("some_new_fancy_function", &[]);
    result.assert_success();
    let some_numbers: [u32; 7] = result.unwrap_borsh();
    assert_eq!(some_numbers, [3, 1, 4, 1, 5, 9, 2]);
}

// This test has nothing to do with migration. I'm just putting it here for convenience
// because it does require near-sdk-sim and the state migration test already had the
// `deploy_evm()` function to set everything up.
#[test]
fn test_state_revert() {
    let aurora = deploy_evm();
    let mut signer = test_utils::Signer::random();
    let address = test_utils::address_from_secret_key(&signer.secret_key);

    // create account
    let args = crate::parameters::WithdrawCallArgs {
        recipient_address: address.0,
        amount: 1_000_000,
    };
    aurora
        .call("mint_account", &args.try_to_vec().unwrap())
        .assert_success();

    // confirm nonce is zero
    let x = aurora.call("get_nonce", &address.0);
    let observed_nonce = match &x.outcome().status {
        near_sdk_sim::transaction::ExecutionStatus::SuccessValue(b) => U256::from_big_endian(&b),
        _ => panic!("?"),
    };
    assert_eq!(observed_nonce, U256::zero());

    // try operation that fails (transfer more eth than we have)
    let nonce = signer.use_nonce();
    let tx = test_utils::transfer(
        crate::prelude::Address([0; 20]),
        Wei::new_u64(2_000_000),
        nonce.into(),
    );
    let signed_tx = test_utils::sign_transaction(
        tx,
        Some(AuroraRunner::default().chain_id),
        &signer.secret_key,
    );
    let x = aurora.call("submit", rlp::encode(&signed_tx).as_ref());
    println!("{:?}", x);

    // check nonce again; it should have incremented because the transaction was valid
    // (even though its execution failed)
    let x = aurora.call("get_nonce", &address.0);
    let observed_nonce = match &x.outcome().status {
        near_sdk_sim::transaction::ExecutionStatus::SuccessValue(b) => U256::from_big_endian(&b),
        _ => panic!("?"),
    };
    assert_eq!(observed_nonce, U256::one());
}

fn deploy_evm() -> AuroraAccount {
    let aurora_runner = AuroraRunner::default();
    let main_account = near_sdk_sim::init_simulator(None);
    let contract_account = main_account.deploy(
        &aurora_runner.code.code,
        aurora_runner.aurora_account_id.parse().unwrap(),
        5 * near_sdk_sim::STORAGE_AMOUNT,
    );
    let new_args = NewCallArgs {
        chain_id: types::u256_to_arr(&U256::from(aurora_runner.chain_id)),
        owner_id: main_account.account_id.clone().into(),
        bridge_prover_id: "prover.near".to_string(),
        upgrade_delay_blocks: 1,
    };
    main_account
        .call(
            contract_account.account_id.clone(),
            "new",
            &new_args.try_to_vec().unwrap(),
            near_sdk_sim::DEFAULT_GAS,
            0,
        )
        .assert_success();
    AuroraAccount {
        user: main_account,
        contract: contract_account,
    }
}

struct AuroraAccount {
    user: UserAccount,
    contract: UserAccount,
}

impl AuroraAccount {
    fn call(&self, method: &str, args: &[u8]) -> ExecutionResult {
        self.user.call(
            self.contract.account_id.clone(),
            method,
            args,
            near_sdk_sim::DEFAULT_GAS,
            0,
        )
    }
}

fn contract_bytes() -> Vec<u8> {
    let base_path = Path::new("etc").join("state-migration-test");
    let output_path = base_path
        .join("target/wasm32-unknown-unknown/release/aurora_engine_state_migration_test.wasm");
    compile(base_path);
    fs::read(output_path).unwrap()
}

fn compile<P: AsRef<Path>>(source_path: P) {
    let output = Command::new("cargo")
        .current_dir(source_path)
        .args(&["build", "--target", "wasm32-unknown-unknown", "--release"])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!("{}", String::from_utf8(output.stderr).unwrap());
    }
}
