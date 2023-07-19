use crate::utils;
use aurora_engine::parameters::SetUpgradeDelayBlocksArgs;
use aurora_engine_types::borsh::BorshSerialize;

#[test]
fn test_pause_contract_require_owner() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("pause_contract", "new_owner.near", vec![]);
    assert!(result.is_err());
}

#[test]
fn test_resume_contract_require_owner() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("resume_contract", "new_owner.near", vec![]);
    assert!(result.is_err());
}

#[test]
fn test_pause_contract_require_running() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_err());
}

#[test]
fn test_resume_contract_require_paused() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_err());

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_pause_contract() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();
    let set = SetUpgradeDelayBlocksArgs {
        upgrade_delay_blocks: 2,
    }
    .try_to_vec()
    .unwrap();

    // contract is running by default, gets and sets should work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set.clone());
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is paused, gets should still work but sets should fail
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set);
    assert!(result.is_err());
}

#[test]
fn test_resume_contract() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();
    let set = SetUpgradeDelayBlocksArgs {
        upgrade_delay_blocks: 2,
    }
    .try_to_vec()
    .unwrap();

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // resume contract
    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is running again, gets and sets should work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set);
    assert!(result.is_ok());
}
