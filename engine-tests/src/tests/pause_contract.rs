use crate::test_utils;

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
fn test_pause_contract() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();

    // get owner to check that contract is running (by default)
    let result = runner.call("get_owner", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // get owner to check that contract is paused
    let result = runner.call("get_owner", &aurora_account_id, vec![]);
    assert!(result.is_err());
}

#[test]
fn test_resume_contract() {
    let mut runner = test_utils::deploy_evm();
    let aurora_account_id = runner.aurora_account_id.clone();

    // get owner to check that contract is running (by default)
    let result = runner.call("get_owner", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // pause contract
    let result = runner.call("pause_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // get owner to check that contract is paused
    let result = runner.call("get_owner", &aurora_account_id, vec![]);
    assert!(result.is_err());

    // resume contract
    let result = runner.call("resume_contract", &aurora_account_id, vec![]);
    assert!(result.is_ok());

    // get owner to check that contract is running again
    let result = runner.call("get_owner", &aurora_account_id, vec![]);
    assert!(result.is_ok());
}