//! A module containing tests which reproduce transactions sent to live networks.

use crate::utils::{standalone, AuroraRunner, ExecutionProfile};
use aurora_engine::parameters::SubmitResult;
use aurora_engine_types::borsh::BorshDeserialize;
use engine_standalone_storage::json_snapshot;

/// This test reproduces a transaction from testnet:
/// `https://explorer.testnet.near.org/transactions/GdASJ3KESs8VegpFECTveCwLQp8fxw8yvsauNEmGb6pZ`
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
    repro_common(&ReproContext {
        snapshot_path: "src/tests/res/aurora_state_GdASJ3KESs.json",
        block_height: 83_596_945,
        block_timestamp: 1_645_717_564_644_206_730,
        input_path: "src/tests/res/input_GdASJ3KESs.hex",
        evm_gas_used: 706_713,
        near_gas_used: 114,
    });
}

/// This test reproduces a transaction from mainnet:
/// `https://explorer.mainnet.near.org/transactions/8ru7VEAEbyfZdbC1W2PYQv2cY3W92rbTToDEN4yTp8aZ`
/// It hit the NEAR gas limit even after the 2.5.2 engine release and limit increase to 300 Tgas.
/// The purpose of having it here is to be able to track its performance as we continue to
/// optimize the Engine.
/// The test is somewhat inscrutable because the data was directly pulled from the Engine contract
/// on mainnet, but according to the partner that submitted the transaction, the transaction should
/// be doing something similar to this one on Ethereum itself:
/// `https://etherscan.io/tx/0x6c1ccadf6553f4f8bdb475667a91f050b1dfb63ded09053354f1e6fd78ff63a6`
#[allow(non_snake_case)]
#[test]
fn repro_8ru7VEA() {
    // Note: this snapshot is pruned from the full Engine state on mainnet at that block height.
    // The full snapshot is very large, and all that is necessary for this test are the keys used
    // in the transaction. This pruned snapshot contains precisely those keys, and no others.
    repro_common(&ReproContext {
        snapshot_path: "src/tests/res/aurora_state_8ru7VEA.json",
        block_height: 62_625_815,
        block_timestamp: 1_648_829_935_343_349_589,
        input_path: "src/tests/res/input_8ru7VEA.hex",
        evm_gas_used: 1_732_181,
        near_gas_used: 205,
    });
}

/// This test reproduces a transaction from mainnet:
/// `https://explorer.mainnet.near.org/transactions/FRcorNvFojoxBrdiVMTy9gRD3H8EYXXKau4feevMZmFV`
/// It hit the gas limit at the time of its execution (engine v2.5.2 after 300 Tgas limit increase).
/// The transaction performs some complex defi interaction (description from the user):
/// 1. It sells 30% BSTN to NEAR, and mint `cNEAR`
/// 2. It sells 35% BSTN to NEAR, and make NEAR-BSTN LP token
/// 3. Deposit LP token created from step2 to Trisolaris farm
#[allow(non_snake_case)]
#[test]
fn repro_FRcorNv() {
    repro_common(&ReproContext {
        snapshot_path: "src/tests/res/aurora_state_FRcorNv.json",
        block_height: 64_328_524,
        block_timestamp: 1_650_960_438_774_745_116,
        input_path: "src/tests/res/input_FRcorNv.hex",
        evm_gas_used: 1_239_721,
        near_gas_used: 167,
    });
}

/// This test reproduces a transaction from mainnet:
/// `https://explorer.mainnet.near.org/transactions/5bEgfRQ5TSJfN9XCqYkMr9cgBLToM7JmS1bNzKpDXJhT`
/// It hit the gas limit at the time of its execution (engine v2.5.2 after 300 Tgas limit increase).
/// The transaction is a "claim xp rewards action" from the game `CryptoBlades`.
#[allow(non_snake_case)]
#[test]
fn repro_5bEgfRQ() {
    repro_common(&ReproContext {
        snapshot_path: "src/tests/res/aurora_state_5bEgfRQ.json",
        block_height: 64_417_403,
        block_timestamp: 1_651_073_772_931_594_646,
        input_path: "src/tests/res/input_5bEgfRQ.hex",
        evm_gas_used: 6_414_105,
        near_gas_used: 649,
    });
}

/// This test reproduces a transaction from mainnet:
/// `https://explorer.mainnet.near.org/transactions/D98vwmi44hAYs8KtX5aLne1zEkj3MUss42e5SkG2a4SC`
/// It hit the gas limit at the time of its execution (engine v2.5.2 after 300 Tgas limit increase).
/// The transaction is a harvest action for some sort of defi contract. See the report here:
/// `https://github.com/aurora-is-near/aurora-relayer/issues/60#issuecomment-1118549256`
#[allow(non_snake_case)]
#[test]
fn repro_D98vwmi() {
    repro_common(&ReproContext {
        snapshot_path: "src/tests/res/aurora_state_D98vwmi.json",
        block_height: 64_945_381,
        block_timestamp: 1_651_753_443_421_003_245,
        input_path: "src/tests/res/input_D98vwmi.hex",
        evm_gas_used: 1_035_348,
        near_gas_used: 168,
    });
}

/// This test reproduces a transaction from testnet:
/// `https://explorer.testnet.near.org/transactions/Emufid2pv2UpxrZae4NyowF2N2ZHvYEPq16LsQc7Uoc6`
/// It hit the gas limit at the time of its execution (engine v2.7.0).
/// The transaction is some kind of multi-step token swap. The user says it should be similar
/// to this transaction on Polygon:
/// `https://mumbai.polygonscan.com/tx/0xd9ab182692c74a873f0c444854ed1045edbb32a252b561677042276143a024b7`
#[allow(non_snake_case)]
#[test]
fn repro_Emufid2() {
    repro_common(&ReproContext {
        snapshot_path: "src/tests/res/aurora_state_Emufid2.json",
        block_height: 99_197_180,
        block_timestamp: 1_662_118_048_636_713_538,
        input_path: "src/tests/res/input_Emufid2.hex",
        evm_gas_used: 1_156_364,
        near_gas_used: 293,
    });
}

fn repro_common(context: &ReproContext) {
    let ReproContext {
        snapshot_path,
        block_height,
        block_timestamp,
        input_path,
        evm_gas_used,
        near_gas_used,
    } = context;

    let snapshot = json_snapshot::types::JsonSnapshot::load_from_file(snapshot_path).unwrap();

    let mut runner = AuroraRunner {
        standalone_runner: None, // Turn off standalone here, validated separately below
        ..Default::default()
    };
    runner.wasm_config.limit_config.max_gas_burnt = 3_000_000_000_000_000;
    runner.context.storage_usage = 1_000_000_000;
    runner.consume_json_snapshot(snapshot.clone());
    runner.context.block_height = *block_height;
    runner.context.block_timestamp = *block_timestamp;

    let tx_hex = std::fs::read_to_string(input_path).unwrap();
    let tx_bytes = hex::decode(tx_hex.trim()).unwrap();

    // Make a random call that touches the Engine state to force the lazy migration
    runner
        .call("get_chain_id", "relay.aurora", Vec::new())
        .unwrap();
    // Run benchmark post-migration
    let outcome = runner.call("submit", "relay.aurora", tx_bytes).unwrap();
    let profile = ExecutionProfile::new(&outcome);
    let submit_result =
        SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(submit_result.gas_used, *evm_gas_used);
    assert_eq!(*near_gas_used, profile.all_gas() / 1_000_000_000_000);

    // Also validate the SubmitResult in the standalone engine
    let mut standalone = standalone::StandaloneRunner::default();
    json_snapshot::initialize_engine_state(&standalone.storage, snapshot).unwrap();
    let standalone_result = standalone
        .submit_raw("submit", &runner.context, &[], None)
        .unwrap();
    assert_eq!(
        borsh::to_vec(&submit_result).unwrap(),
        borsh::to_vec(&standalone_result).unwrap()
    );
    standalone.close();
}

struct ReproContext<'a> {
    snapshot_path: &'a str,
    block_height: u64,
    block_timestamp: u64,
    input_path: &'a str,
    evm_gas_used: u64,
    near_gas_used: u64,
}
