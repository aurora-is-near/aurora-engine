use near_sdk::borsh::BorshSerialize;
use near_sdk::test_utils::accounts;
use near_sdk_sim::{to_yocto, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};

use aurora_engine::parameters::NewCallArgs;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "release.wasm"
}

pub fn init() -> (UserAccount, UserAccount) {
    let master_account = near_sdk_sim::init_simulator(None);
    let contract_account =
        master_account.deploy(*EVM_WASM_BYTES, accounts(0).to_string(), to_yocto("1000"));
    contract_account
        .call(
            accounts(0).to_string(),
            "new",
            &NewCallArgs {
                chain_id: [0u8; 32],
                owner_id: master_account.account_id.clone(),
                bridge_prover_id: accounts(0).to_string(),
                upgrade_delay_blocks: 1,
            }
            .try_to_vec()
            .unwrap(),
            DEFAULT_GAS,
            STORAGE_AMOUNT,
        )
        .assert_success();
    (master_account, contract_account)
}

#[test]
fn test_contract_upgrade() {
    let (master_account, _contract_account) = init();
    master_account
        .call(
            accounts(0).to_string(),
            "stage_upgrade",
            &EVM_WASM_BYTES,
            DEFAULT_GAS,
            0,
        )
        .assert_success();
    master_account
        .call(
            accounts(0).to_string(),
            "deploy_upgrade",
            &[],
            DEFAULT_GAS,
            0,
        )
        .assert_success();
}
