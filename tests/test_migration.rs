#![allow(dead_code)]

use near_sdk::borsh::BorshSerialize;
use near_sdk::test_utils::accounts;
use near_sdk_sim::{to_yocto, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};

use aurora_engine::parameters::{InitCallArgs, NewCallArgs};

const CONTRACT_ACC: &'static str = "eth_connector.root";
const PROVER_ACCOUNT: &'static str = "eth_connector.root";
const EVM_CUSTODIAN_ADDRESS: &'static str = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "release.wasm"
}

fn init() -> (UserAccount, UserAccount) {
    let master_account = near_sdk_sim::init_simulator(None);
    let contract = init_contract(&master_account, CONTRACT_ACC);
    (master_account, contract)
}

fn init_contract(master_account: &UserAccount, contract_name: &str) -> UserAccount {
    let contract_account = master_account.deploy(
        *EVM_WASM_BYTES,
        contract_name.to_string(),
        to_yocto("1000000"),
    );
    contract_account
        .call(
            contract_name.to_string(),
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
    contract_account
        .call(
            contract_name.to_string(),
            "new_eth_connector",
            &InitCallArgs {
                prover_account: PROVER_ACCOUNT.into(),
                eth_custodian_address: EVM_CUSTODIAN_ADDRESS.into(),
            }
            .try_to_vec()
            .unwrap(),
            DEFAULT_GAS,
            0,
        )
        .assert_success();
    contract_account
}

#[test]
fn test_json_parse() {
    let json_data = r#"[
        {
            "action": "Add",
            "data": [{
                "new_field": "test1",
                "old_field": "test2",
                "prefix": "1",
                "value": "val1"
            }]
        }
    ]"#;
    let (master_account, _contract) = init();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "migrate",
        &json_data.as_bytes(),
        DEFAULT_GAS,
        0,
    );
    println!("{:#?}", res.promise_results());
}
