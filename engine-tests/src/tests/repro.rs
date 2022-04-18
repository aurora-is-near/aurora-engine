//! A module containing tests which reproduce transactions sent to live networks.

use crate::test_utils::standalone;
use crate::test_utils::{AuroraRunner, ExecutionProfile};
use aurora_engine::parameters::SubmitResult;
use borsh::{BorshDeserialize, BorshSerialize};
use engine_standalone_storage::json_snapshot;

/// This test reproduces a transaction from testnet:
/// https://explorer.testnet.near.org/transactions/GdASJ3KESs8VegpFECTveCwLQp8fxw8yvsauNEmGb6pZ
/// It hit the NEAR gas limit even after the 2.4 engine release and limit increase to 300 Tgas.
/// The purpose of having it here is to be able to track it's performance as we continue to
/// optimize the Engine.
/// The test is somewhat inscrutable because the data was directly pulled from the Engine contract
/// on testnet, but according to the partner that submitted the transaction, the high level
/// description of what is happening is as follows:
/// "flashswap from uniswapv pool with call back to liquidate the user on compound and swap back the seized asset to payback the pool"
#[allow(non_snake_case)]
#[test]
fn repro_GdASJ3KESs() {
    // Note: this snapshot is pruned from the full Engine state on testnet at that block height.
    // The full snapshot is very large, and all that is necessary for this test are the keys used
    // in the transaction. This pruned snapshot contains precisely those keys, and no others.
    let snapshot = json_snapshot::types::JsonSnapshot::load_from_file(
        "src/tests/res/aurora_state_GdASJ3KESs.json",
    )
    .unwrap();

    let mut runner = AuroraRunner::default();
    runner.wasm_config.limit_config.max_gas_burnt = 3_000_000_000_000_000;
    runner.context.storage_usage = 1_000_000_000;
    runner.consume_json_snapshot(snapshot.clone());
    runner.context.block_index = 83596945;
    runner.context.block_timestamp = 1645717564644206730;

    let tx_hex = std::fs::read_to_string("src/tests/res/input_GdASJ3KESs.hex").unwrap();
    let tx_bytes = hex::decode(tx_hex.trim()).unwrap();

    let (outcome, error) = runner.call("submit", "relay.aurora", tx_bytes);
    let outcome = outcome.unwrap();
    let profile = ExecutionProfile::new(&outcome);
    if let Some(error) = error {
        panic!("{:?}", error);
    }
    let submit_result =
        SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(submit_result.gas_used, 706713);
    assert_eq!(239, profile.all_gas() / 1_000_000_000_000);

    // Also validate the SubmitResult in the standalone engine
    let mut standalone = standalone::StandaloneRunner::default();
    json_snapshot::initialize_engine_state(&mut standalone.storage, snapshot).unwrap();
    let standalone_result = standalone.submit_raw("submit", &runner.context).unwrap();
    assert_eq!(
        submit_result.try_to_vec().unwrap(),
        standalone_result.try_to_vec().unwrap()
    );
    standalone.close()
}

/// This test reproduces a transaction from mainnet:
/// https://explorer.mainnet.near.org/transactions/8ru7VEAEbyfZdbC1W2PYQv2cY3W92rbTToDEN4yTp8aZ
/// It hit the NEAR gas limit even after the 2.5.2 engine release and limit increase to 300 Tgas.
/// The purpose of having it here is to be able to track its performance as we continue to
/// optimize the Engine.
/// The test is somewhat inscrutable because the data was directly pulled from the Engine contract
/// on mainnet, but according to the partner that submitted the transaction, the transaction should
/// be doing something similar to this one on Ethereum itself:
/// https://etherscan.io/tx/0x6c1ccadf6553f4f8bdb475667a91f050b1dfb63ded09053354f1e6fd78ff63a6
#[allow(non_snake_case)]
#[test]
fn repro_8ru7VEA() {
    // Note: this snapshot is pruned from the full Engine state on mainnet at that block height.
    // The full snapshot is very large, and all that is necessary for this test are the keys used
    // in the transaction. This pruned snapshot contains precisely those keys, and no others.
    let snapshot = json_snapshot::types::JsonSnapshot::load_from_file(
        "src/tests/res/aurora_state_8ru7VEA.json",
    )
    .unwrap();

    let mut runner = AuroraRunner::default();
    runner.wasm_config.limit_config.max_gas_burnt = 3_000_000_000_000_000;
    runner.context.storage_usage = 1_000_000_000;
    runner.consume_json_snapshot(snapshot.clone());
    runner.context.block_index = 62625815;
    runner.context.block_timestamp = 1648829935343349589;

    let tx_hex = std::fs::read_to_string("src/tests/res/input_8ru7VEA.hex").unwrap();
    let tx_bytes = hex::decode(tx_hex.trim()).unwrap();

    let (outcome, error) = runner.call("submit", "relay.aurora", tx_bytes);
    let outcome = outcome.unwrap();
    let profile = ExecutionProfile::new(&outcome);
    if let Some(error) = error {
        panic!("{:?}", error);
    }
    let submit_result =
        SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(submit_result.gas_used, 1732181);
    assert_eq!(411, profile.all_gas() / 1_000_000_000_000);

    // Also validate the SubmitResult in the standalone engine
    let mut standalone = standalone::StandaloneRunner::default();
    json_snapshot::initialize_engine_state(&mut standalone.storage, snapshot).unwrap();
    let standalone_result = standalone.submit_raw("submit", &runner.context).unwrap();
    assert_eq!(
        submit_result.try_to_vec().unwrap(),
        standalone_result.try_to_vec().unwrap()
    );
    standalone.close()
}
