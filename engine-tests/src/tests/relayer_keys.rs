use crate::prelude::U256;
use crate::test_utils::asserts::assert_execution_status_failure;
use crate::test_utils::{str_to_account_id, AuroraRunner};
use aurora_engine::parameters::NewCallArgs;
use aurora_engine_types::types::ZERO_YOCTO;
use borsh::BorshSerialize;
use near_crypto::PublicKey;
use near_primitives::account::AccessKeyPermission;
use near_sdk_sim::{ExecutionResult, UserAccount};
use std::str::FromStr;

const PUBLIC_KEY: &str = "ed25519:3gyjNWQWMZNrzqWLhwxzQqfeDFyd3KhXjzmwCqzVCRuF";
const PUBLIC_KEY_BUDGET: u128 = 1_000_000;

// there are multiple deploy_evm implementations but their API is not good enough
// TODO replace with near-workspaces
fn deploy_evm_with_relayer() -> AuroraAccount {
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
        bridge_prover_id: prover_account,
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
        owner: main_account,
        contract: contract_account,
    }
}

pub struct AuroraAccount {
    pub owner: UserAccount,
    pub contract: UserAccount,
}

fn add_relayer_key_call(user: &UserAccount, runner: &AuroraAccount) -> ExecutionResult {
    user.call(
        runner.contract.account_id.clone(),
        "add_relayer_key",
        format!("{{\"public_key\": \"{PUBLIC_KEY}\"}}").as_bytes(),
        near_sdk_sim::DEFAULT_GAS,
        PUBLIC_KEY_BUDGET,
    )
}

fn remove_relayer_key_call(user: &UserAccount, runner: &AuroraAccount) -> ExecutionResult {
    user.call(
        runner.contract.account_id.clone(),
        "remove_relayer_key",
        format!("{{\"public_key\": \"{PUBLIC_KEY}\"}}").as_bytes(),
        near_sdk_sim::DEFAULT_GAS,
        ZERO_YOCTO.as_u128(),
    )
}

#[test]
fn test_relayer_keys_mgmt_access() {
    let runner = deploy_evm_with_relayer();

    let result = add_relayer_key_call(&runner.contract, &runner);
    assert_execution_status_failure(
        result.outcome().clone().status,
        "ERR_NOT_ALLOWED",
        "Expected failure as public key does not exist",
    );
}

#[test]
fn test_relayer_keys_mgmt() {
    let runner = deploy_evm_with_relayer();

    add_relayer_key_call(&runner.owner, &runner).assert_success();

    let pk = PublicKey::from_str(PUBLIC_KEY).unwrap();
    let ak = runner
        .contract
        .borrow_runtime()
        .view_access_key(runner.contract.account_id.clone().as_str(), &pk)
        .unwrap();
    let fk = match ak.permission {
        AccessKeyPermission::FullAccess => panic!("Expected function access key"),
        AccessKeyPermission::FunctionCall(fk) => fk,
    };
    assert_eq!(fk.allowance.unwrap(), PUBLIC_KEY_BUDGET);
    assert_eq!(fk.method_names.join(","), "submit");

    remove_relayer_key_call(&runner.owner, &runner).assert_success();
    let ak = runner
        .contract
        .borrow_runtime()
        .view_access_key(runner.contract.account_id.clone().as_str(), &pk);

    assert_eq!(ak, None);
}
