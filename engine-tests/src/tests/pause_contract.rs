use crate::test_utils;
use aurora_engine::parameters::SetUpgradeDelayBlocksArgs;
use borsh::BorshSerialize;

#[test]
fn test_pause_contract_require_owner() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();

    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("pause_contract", "new_owner.near", vec![]);
    assert!(result.is_err());
}

#[test]
fn test_resume_contract_require_owner() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();

    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    let result = runner.call("resume_contract", "new_owner.near", vec![]);
    assert!(result.is_err());
}

#[test]
fn test_pause_contract_get_method() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();

    // contract is running by default, gets should work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is paused, gets should still work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_pause_contract_set_method() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();
    let set_input = (SetUpgradeDelayBlocksArgs {upgrade_delay_blocks: 2}).try_to_vec().unwrap();

    // contract is running by default, sets should work
    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set_input.clone());
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is paused, sets should NOT work
    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set_input);
    assert!(result.is_err());
}

#[test]
fn test_resume_contract_get_method() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();

    // contract is running by default, gets should work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is paused, gets should still work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // resume contract
    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is running again, gets should work
    let result = runner.call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_resume_contract_set_method() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();
    let set_input = (SetUpgradeDelayBlocksArgs {upgrade_delay_blocks: 2}).try_to_vec().unwrap();

    // contract is running by default, sets should work
    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set_input.clone());
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is paused, sets should NOT work
    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set_input.clone());
    assert!(result.is_err());

    // resume contract
    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // contract is running again, sets should work
    let result = runner.call("set_upgrade_delay_blocks", &aurora_account_id, set_input);
    assert!(result.is_ok());
}