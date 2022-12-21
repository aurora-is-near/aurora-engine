use crate::prelude::U256;
use crate::test_utils::{self, str_to_account_id, AuroraRunner};
use aurora_engine::parameters::{InitCallArgs, NewCallArgs};
use borsh::BorshSerialize;
use near_sdk_sim::{ExecutionResult, UserAccount};
use std::fs;
use std::path::Path;

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

pub fn deploy_evm() -> AuroraAccount {
    let aurora_runner = AuroraRunner::default();
    let main_account = near_sdk_sim::init_simulator(None);
    let sim_aurora_account = format!(
        "{}.{}",
        aurora_runner.aurora_account_id,
        main_account.account_id()
    );
    let contract_account = main_account.deploy(
        aurora_runner.code.code(),
        sim_aurora_account.parse().unwrap(),
        5 * near_sdk_sim::STORAGE_AMOUNT,
    );
    let prover_account = str_to_account_id("prover.near");
    let new_args = NewCallArgs {
        chain_id: crate::prelude::u256_to_arr(&U256::from(aurora_runner.chain_id)),
        owner_id: str_to_account_id(main_account.account_id.as_str()),
        bridge_prover_id: prover_account.clone(),
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
    let init_args = InitCallArgs {
        prover_account,
        eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
        metadata: Default::default(),
    };
    contract_account
        .call(
            contract_account.account_id.clone(),
            "new_eth_connector",
            &init_args.try_to_vec().unwrap(),
            near_sdk_sim::DEFAULT_GAS,
            0,
        )
        .assert_success();
    AuroraAccount {
        user: main_account,
        contract: contract_account,
    }
}

pub struct AuroraAccount {
    pub user: UserAccount,
    pub contract: UserAccount,
}

impl AuroraAccount {
    pub fn call(&self, method: &str, args: &[u8]) -> ExecutionResult {
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
    let base_path = Path::new("../etc")
        .join("tests")
        .join("state-migration-test");
    let output_path = base_path
        .join("target/wasm32-unknown-unknown/release/aurora_engine_state_migration_test.wasm");
    test_utils::rust::compile(base_path);
    fs::read(output_path).unwrap()
}
