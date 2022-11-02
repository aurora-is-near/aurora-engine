use aurora_engine_sdk::env::Env;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H256, U256};
use engine_standalone_tracing::{sputnik, types::TransactionTrace};
use serde::Deserialize;
use std::path::Path;

use crate::test_utils::{self, standalone};

/// This test replays two transactions from Ethereum mainnet (listed below) and checks we obtain
/// the same gas usage and transaction trace as reported by etherscan.
/// Transactions:
/// * https://etherscan.io/tx/0x79f7f8f9b3ad98f29a3df5cbed1556397089701c3ce007c2844605849dfb0ad4
/// * https://etherscan.io/tx/0x33db52b0e7fa03cd84e8c99fea90a1962b4f8d0e63c8bbe4c11373a233dc4f0e
/// Traces:
/// * https://etherscan.io/vmtrace?txhash=0x79f7f8f9b3ad98f29a3df5cbed1556397089701c3ce007c2844605849dfb0ad4
/// * https://etherscan.io/vmtrace?txhash=0x33db52b0e7fa03cd84e8c99fea90a1962b4f8d0e63c8bbe4c11373a233dc4f0e
#[test]
fn test_evm_tracing_with_storage() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
    let sender_address = Address::decode("304ee8ae14eceb3a544dff53a27eb1bb1aaa471f").unwrap();
    let weth_address = Address::decode("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();

    // Initialize EVM
    runner.init_evm_with_chain_id(1);
    runner.mint_account(signer_address, Wei::zero(), signer.nonce.into(), None);

    // Deploy WETH contract
    let weth_constructor = test_utils::weth::WethConstructor::load();
    let deploy_tx = weth_constructor.deploy(signer.use_nonce().into());
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address =
        Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();

    // Move it over to the same address as it exists on mainnet
    let mut diff = engine_standalone_storage::Diff::default();
    for (key, value) in runner.get_current_state().iter() {
        if key.len() >= 22 && &key[2..22] == contract_address.as_bytes() {
            let mut new_key = key.clone();
            new_key[2..22].copy_from_slice(weth_address.as_bytes());
            match value {
                engine_standalone_storage::diff::DiffValue::Modified(bytes) => {
                    diff.modify(new_key, bytes.clone())
                }
                engine_standalone_storage::diff::DiffValue::Deleted => diff.delete(new_key),
            }
        }
    }
    runner.env.block_height += 1;
    let block_height = runner.env.block_height;
    let block_hash = test_utils::standalone::mocks::compute_block_hash(block_height);
    let block_metadata = engine_standalone_storage::BlockMetadata {
        timestamp: runner.env.block_timestamp(),
        random_seed: runner.env.random_seed(),
    };
    runner
        .storage
        .set_block_data(block_hash, block_height, block_metadata)
        .unwrap();
    let tx = engine_standalone_storage::sync::TransactionIncludedOutcome {
        hash: H256::zero(),
        info: engine_standalone_storage::sync::types::TransactionMessage {
            block_hash,
            near_receipt_id: H256::zero(),
            position: 0,
            succeeded: true,
            signer: "system".parse().unwrap(),
            caller: "system".parse().unwrap(),
            attached_near: 0,
            transaction: engine_standalone_storage::sync::types::TransactionKind::Unknown,
            promise_data: Vec::new(),
        },
        diff,
        maybe_result: Ok(None),
    };
    test_utils::standalone::storage::commit(&mut runner.storage, &tx);

    // Replay transaction depositing some ETH to get WETH (for the first time)
    // tx: https://etherscan.io/tx/0x79f7f8f9b3ad98f29a3df5cbed1556397089701c3ce007c2844605849dfb0ad4
    let tx_nonce = U256::from(2);
    let tx_bytes = hex::decode(MAINNET_TX_79F7F8F9).unwrap();
    runner.mint_account(
        sender_address,
        Wei::from_eth(2.into()).unwrap(),
        tx_nonce,
        None,
    );
    let mut listener = sputnik::TransactionTraceBuilder::default();
    let result = sputnik::traced_call(&mut listener, || {
        runner.submit_raw_transaction_bytes(&tx_bytes).unwrap()
    });
    assert!(result.status.is_ok());
    assert_eq!(result.gas_used, 45_038);

    // Check trace
    check_transaction_trace(
        listener.finish(),
        "src/tests/res/79f7f8f9b3ad98f29a3df5cbed1556397089701c3ce007c2844605849dfb0ad4_trace.json",
    );

    // Replay transaction depositing some ETH to get WETH (for the second time)
    // tx: https://etherscan.io/tx/0x33db52b0e7fa03cd84e8c99fea90a1962b4f8d0e63c8bbe4c11373a233dc4f0e
    let tx_nonce = U256::from(14);
    let tx_bytes = hex::decode(MAINNET_TX_33DB52B0).unwrap();
    runner.mint_account(
        sender_address,
        Wei::from_eth(2.into()).unwrap(),
        tx_nonce,
        None,
    );
    let mut listener = sputnik::TransactionTraceBuilder::default();
    let result = sputnik::traced_call(&mut listener, || {
        runner.submit_raw_transaction_bytes(&tx_bytes).unwrap()
    });
    assert!(result.status.is_ok());
    assert_eq!(result.gas_used, 27_938);

    // Check trace
    check_transaction_trace(
        listener.finish(),
        "src/tests/res/33db52b0e7fa03cd84e8c99fea90a1962b4f8d0e63c8bbe4c11373a233dc4f0e_trace.json",
    );
}

/// Test based on expected trace of
/// https://rinkeby.etherscan.io/tx/0xfc9359e49278b7ba99f59edac0e3de49956e46e530a53c15aa71226b7aa92c6f
/// (geth example found at https://gist.github.com/karalabe/c91f95ac57f5e57f8b950ec65ecc697f).
#[test]
fn test_evm_tracing() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();

    // Initialize EVM
    runner.init_evm();

    // Deploy contract
    let deploy_tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: None,
        value: Wei::zero(),
        data: hex::decode(CONTRACT_CODE).unwrap(),
    };
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address =
        Address::try_from_slice(test_utils::unwrap_success_slice(&result)).unwrap();

    // Interact with contract (and trace execution)
    let tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: 90_000.into(),
        to: Some(contract_address),
        value: Wei::zero(),
        data: hex::decode(CONTRACT_INPUT).unwrap(),
    };
    let mut listener = sputnik::TransactionTraceBuilder::default();
    let result = sputnik::traced_call(&mut listener, || {
        runner.submit_transaction(&signer.secret_key, tx).unwrap()
    });
    assert!(result.status.is_ok());

    // Check trace
    let trace = listener.finish();
    let positions: Vec<u8> = trace
        .logs()
        .0
        .iter()
        .map(|l| l.program_counter.into_u32() as u8)
        .collect();
    assert_eq!(positions.as_slice(), &EXPECTED_POSITIONS);

    let costs: Vec<u32> = trace
        .logs()
        .0
        .iter()
        .map(|l| l.gas_cost.as_u64() as u32)
        .collect();
    assert_eq!(costs.as_slice(), &EXPECTED_COSTS);

    let op_codes: Vec<u8> = trace.logs().0.iter().map(|l| l.opcode.0).collect();
    assert_eq!(op_codes.as_slice(), &EXPECTED_OP_CODES);
}

const MAINNET_TX_79F7F8F9: &str = "02f87701028459682f00851fb8b1884182afee94c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2880c7d713b49da000084d0e30db0c080a0b1bf69eab31f6d5482f0f8a48f8fcda916db162e0b874d523293c29246e30ed4a03b79f1f9ccbc4fd6beb9809343eadfe1ddafbc0c7b8673aff2cad5bf3345c227";
const MAINNET_TX_33DB52B0: &str = "02f877010e845d57122a85135bb40f4c826d2294c02aaa39b223fe8d0a0e5c4f27ead9083c756cc28805ebc9f935949db384d0e30db0c001a0956288989306881d6e400d6b40cf06d1210a87d71e8dc4179a3e1a37890ae318a06cbbffed3e749cf9c56de8f8db6ec3df62dbebe2e0b007d020de0b27c05db064";
const CONTRACT_CODE: &str = "60606040525b60008054600160a060020a03191633600160a060020a0316179055346001555b5b61011e806100356000396000f3006060604052361560465763ffffffff7c010000000000000000000000000000000000000000000000000000000060003504166383197ef08114604a5780638da5cb5b14605c575b5b5b005b3415605457600080fd5b60466095565b005b3415606657600080fd5b606c60d6565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b6000543373ffffffffffffffffffffffffffffffffffffffff9081169116141560d35760005473ffffffffffffffffffffffffffffffffffffffff16ff5b5b565b60005473ffffffffffffffffffffffffffffffffffffffff16815600a165627a7a7230582080eeb07bf95bf0cca20d03576cbb3a25de3bd0d1275c173d370dcc90ce23158d0029";
const CONTRACT_INPUT: &str = "2df07fbaabbe40e3244445af30759352e348ec8bebd4dd75467a9f29ec55d98d6cf6c418de0e922b1c55be39587364b88224451e7901d10a4a2ee2eeab3cccf51c";
const EXPECTED_POSITIONS: [u8; 27] = [
    0, 2, 4, 5, 6, 7, 9, 10, 15, 45, 47, 48, 49, 50, 55, 56, 57, 59, 60, 61, 66, 67, 69, 70, 71,
    72, 73,
];
const EXPECTED_COSTS: [u32; 27] = [
    3, 3, 12, 2, 3, 3, 10, 3, 3, 3, 3, 5, 3, 3, 3, 3, 3, 10, 3, 3, 3, 3, 10, 1, 1, 1, 0,
];
const EXPECTED_OP_CODES: [u8; 27] = [
    96, 96, 82, 54, 21, 96, 87, 99, 124, 96, 53, 4, 22, 99, 129, 20, 96, 87, 128, 99, 20, 96, 87,
    91, 91, 91, 0,
];

fn check_transaction_trace<P: AsRef<Path>>(trace: TransactionTrace, expected_trace_path: P) {
    let expected_trace: Vec<EtherscanTraceStep> = {
        let file = std::fs::File::open(expected_trace_path).unwrap();
        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader).unwrap()
    };

    assert_eq!(trace.logs().0.len(), expected_trace.len());
    for (log, step) in trace.logs().0.iter().zip(expected_trace.into_iter()) {
        assert_eq!(
            log.program_counter.0, step.pc,
            "Program counters should match"
        );
        assert_eq!(log.depth.into_u32(), step.depth, "Depths should match");
        assert_eq!(log.opcode.as_u8(), step.op, "opcodes should match");
        assert_eq!(
            log.gas_cost.as_u64(),
            step.gas_cost,
            "gas costs should match"
        );
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
struct EtherscanTraceStep {
    pub step: u32,
    pub pc: u32,
    pub op: u8,
    pub gas: u64,
    pub gas_cost: u64,
    pub depth: u32,
    pub opcode_name: String,
}
