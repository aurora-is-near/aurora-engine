use near_sdk::borsh::BorshSerialize;
use near_sdk::serde_json::json;
use near_sdk::test_utils::accounts;
use near_sdk_sim::{to_yocto, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};

use aurora_engine::parameters::NewCallArgs;

const CONTRACT_ACC: &'static str = "eth_connector.root";

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "release.wasm"
}

fn init() -> (UserAccount, UserAccount) {
    let master_account = near_sdk_sim::init_simulator(None);
    let contract_account =
        master_account.deploy(*EVM_WASM_BYTES, CONTRACT_ACC.to_string(), to_yocto("1000"));
    contract_account
        .call(
            CONTRACT_ACC.to_string(),
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
    master_account
        .call(
            CONTRACT_ACC.to_string(),
            "new_eth_connector",
            json!({
                "prover_account": "root",
                "eth_custodian_address": "88657f6D4c4bbDB193C2b0B78DD74cD38479f819",
            })
            .to_string()
            .as_bytes(),
            DEFAULT_GAS,
            0,
        )
        .assert_success();
    (master_account, contract_account)
}

#[test]
fn test_withdraw_eth() {
    /*
    let sender = validate_eth_address("891B2749238B27fF58e951088e55b04de71Dc374".into());
    let eth_recipient = validate_eth_address("891B2749238B27fF58e951088e55b04de71Dc374".into());
    let custodian_address = validate_eth_address("88657f6D4c4bbDB193C2b0B78DD74cD38479f819".into());
    let amount = U256::from(7654321);
    let eip712_signature = "9b97c6fd1428f77ce4dc680415e87b1379bebfdbbefeb2c87e891d3e9b771ed509bfd0910da0c673a72105d44331762d8dba6e700ea3e0395410a1458c79daea1c";
    */
    let (master_account, _contract_account) = init();
    master_account
        .call(
            CONTRACT_ACC.to_string(),
            "withdraw_eth",
            json!({
                "sender": "891B2749238B27fF58e951088e55b04de71Dc374", 
                "eth_recipient": "891B2749238B27fF58e951088e55b04de71Dc374", 
                "amount": "7654321", 
                "eip712_signature": "9b97c6fd1428f77ce4dc680415e87b1379bebfdbbefeb2c87e891d3e9b771ed509bfd0910da0c673a72105d44331762d8dba6e700ea3e0395410a1458c79daea1c"
            })
            .to_string()
            .as_bytes(),
            DEFAULT_GAS,
            0,
        )
        .assert_success();
}
