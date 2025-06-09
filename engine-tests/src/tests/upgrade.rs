use std::{fs, path::Path};

use crate::utils::workspace::deploy_engine;

#[tokio::test]
async fn test_code_upgrade() {
    let aurora = deploy_engine().await;
    // do upgrade
    let result = aurora
        .upgrade(contract_bytes())
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    // call a new method
    let result = aurora
        .as_raw_contract()
        .view("some_new_fancy_function")
        .await
        .unwrap();

    let output: [u32; 7] = result.borsh().unwrap();
    assert_eq!(output, [3, 1, 4, 1, 5, 9, 2]);
}

#[tokio::test]
async fn test_code_upgrade_with_stage() {
    let aurora = deploy_engine().await;
    // do upgrade
    let result = aurora
        .stage_upgrade(contract_bytes())
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = aurora.deploy_upgrade().max_gas().transact().await.unwrap();
    assert!(result.is_success());

    // call a new method
    let result = aurora
        .as_raw_contract()
        .view("some_new_fancy_function")
        .await
        .unwrap();

    let output: [u32; 7] = result.borsh().unwrap();
    assert_eq!(output, [3, 1, 4, 1, 5, 9, 2]);
}

// TODO: Should be reworked with `upgrade_delay_blocks` more then one to check that
// we get the TOO EARLY error.
#[tokio::test]
async fn test_repeated_calls_to_upgrade_should_fail() {
    let aurora = deploy_engine().await;
    // First upgrade should succeed
    let result = aurora
        .stage_upgrade(contract_bytes())
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = aurora.deploy_upgrade().max_gas().transact().await.unwrap();
    assert!(result.is_success());

    // Second upgrade should fail since deployed code doesn't have method `stage_upgrade`.
    let result = aurora
        .stage_upgrade(contract_bytes())
        .max_gas()
        .transact()
        .await;
    assert!(result.is_err());
}

fn contract_bytes() -> Vec<u8> {
    let base_path = Path::new("../etc")
        .join("tests")
        .join("state-migration-test");
    let artifact_path = crate::utils::rust::compile(base_path);

    fs::read(artifact_path).unwrap()
}
