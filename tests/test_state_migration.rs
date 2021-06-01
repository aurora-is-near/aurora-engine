use aurora_engine::parameters::NewCallArgs;
use aurora_engine::prelude::U256;
use aurora_engine::types;
use borsh::BorshSerialize;
use near_sdk_sim::{ExecutionResult, UserAccount};
use std::fs;
use std::path::Path;
use std::process::Command;

// TODO: it would be nice to include this under src/tests but right now this is not possible.
// The issue is a linker error (arising from multiple dependencies on near-vm-logic I think).

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "release.wasm"
}

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

fn deploy_evm() -> AuroraAccount {
    let aurora_config = AuroraConfig::default();
    let main_account = near_sdk_sim::init_simulator(None);
    let contract_account = main_account.deploy(
        &aurora_config.code,
        aurora_config.account_id.clone(),
        5 * near_sdk_sim::STORAGE_AMOUNT,
    );
    let new_args = NewCallArgs {
        chain_id: types::u256_to_arr(&U256::from(aurora_config.chain_id)),
        owner_id: main_account.account_id.clone(),
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

struct AuroraConfig {
    code: Vec<u8>,
    chain_id: u64,
    account_id: String,
}

impl Default for AuroraConfig {
    fn default() -> Self {
        Self {
            code: EVM_WASM_BYTES.to_vec(),
            chain_id: 1313161556, // NEAR betanet
            account_id: "aurora".to_string(),
        }
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
