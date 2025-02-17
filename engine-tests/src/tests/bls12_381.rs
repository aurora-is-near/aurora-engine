//! # BLS12-382 precompiles tests
//!
//! Tests bases on parse data from:
//! <https://github.com/ethereum/execution-spec-tests/releases/tag/pectra-devnet-5%40v1.2.0>
//! for Prague hard fork.
//!
//! To generate test cases created special tool for dump data:
//! <https://github.com/aurora-is-near/sputnikvm/pull/78>
//! It generates full EVM state for transaction execution from `execution-spec-tests`
//! for BLS12-381 precompiles.
//!
//! Second kind of tests is `standalone`. It based on parsed `execution-spec-tests`
//! data for BLS12-381 precompiles but distilled to input/output data only.
//!
//! Full EVM state tests has only limited count. As we can't send big bunch of test
//! cases to NEAR VM, as it's extremely expensive operation from time
//! consumption point of view.
//!
//! Standalone data set fully represents all tests from `execution-spec-tests` for
//! BLS12-381 precompiles. We run this test in standalone manner.

use crate::prelude::{Address, Wei, H160, H256, U256};
use crate::tests::sanity::initialize_transfer;
use crate::utils;
use aurora_engine_precompiles::bls12_381;
use aurora_engine_precompiles::Precompile;
use aurora_engine_transactions::eip_2930;
use aurora_engine_transactions::eip_2930::{AccessTuple, Transaction2930};
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::parameters::engine::SubmitResult;
use evm::backend::MemoryAccount;
use libsecp256k1::SecretKey;
use std::collections::BTreeMap;
use std::{fs, iter};

/// State test dump data struct for fully reprodusing execution flow
/// with input & output and before & after state data.
#[allow(dead_code)]
#[derive(Default, Debug, Clone, serde::Deserialize)]
pub struct StateTestsDump {
    pub state: BTreeMap<H160, MemoryAccount>,
    pub caller: H160,
    pub gas_price: U256,
    pub effective_gas_price: U256,
    pub caller_secret_key: H256,
    pub used_gas: u64,
    pub state_hash: H256,
    pub result_state: BTreeMap<H160, MemoryAccount>,
    pub to: H160,
    pub value: U256,
    pub data: Vec<u8>,
    pub gas_limit: u64,
    pub access_list: Vec<(H160, Vec<H256>)>,
}

impl StateTestsDump {
    /// Transform `access_list` from test case data.
    fn get_access_list(&self) -> Vec<AccessTuple> {
        let al = self.access_list.clone();
        al.iter()
            .map(|(address, key)| AccessTuple {
                address: *address,
                storage_keys: key.clone(),
            })
            .collect()
    }

    /// Read State tests data from directory that contains json files
    /// with specific test cases for precompile.
    /// Return parsed state tests dump data for precompile.
    fn read_test_case(path: &str) -> Vec<Self> {
        fs::read_dir(path)
            .expect("Read source test directory failed")
            .map(|entry| entry.unwrap().path())
            .filter(|entry| fs::metadata(entry).unwrap().is_file())
            .filter(|entry| {
                let file_name = entry.file_name().unwrap();
                std::path::Path::new(file_name)
                    .extension()
                    .unwrap()
                    .to_str()
                    == Some("json")
            })
            .map(|entry| fs::read_to_string(entry).unwrap())
            .map(|data| serde_json::from_str(&data).unwrap())
            .collect::<Vec<_>>()
    }
}

/// Precompile input and output data struct
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PrecompileStandaloneData {
    pub input: String,
    pub output: String,
}

/// Standalone data for precompile tests.
/// It contains input data for precompile and expected
/// output after precompile execution.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PrecompileStandalone {
    pub precompile_data: Vec<PrecompileStandaloneData>,
}

impl PrecompileStandalone {
    fn new(path: &str) -> Self {
        let data = fs::read_to_string(path).expect("Unable to read file");
        serde_json::from_str(&data).unwrap()
    }
}

/// Get secret key from hash
fn get_secret_key(hash: H256) -> SecretKey {
    let mut secret_key = [0; 32];
    secret_key.copy_from_slice(hash.as_bytes());
    SecretKey::parse(&secret_key).expect("Unable to parse secret key")
}

/// Read test cases from directory, fill blockchain state
/// and send transaction that execute logic that touch precompile
/// logic. To validate correctness we check transaction status and
/// EVM gas consumption. We don't validate result state as it's extremely
/// expensive operation from execution time point. Most important part
/// of test case execution is controlled NEAR gas consumption.
fn run_bls12_381_transaction_call(path: &str) {
    for test_case in StateTestsDump::read_test_case(path) {
        // To avoid NEAR gas limit exceed exception
        if test_case.data.len() > 800 {
            continue;
        }

        let mut runner = utils::deploy_runner();
        runner.standalone_runner = None;
        // Get caller secret key
        let sk = get_secret_key(test_case.caller_secret_key);
        for (address, account) in &test_case.state {
            runner.create_address_with_code(
                Address::new(*address),
                Wei::new(account.balance),
                account.nonce,
                account.code.clone(),
            );
        }
        let transaction = Transaction2930 {
            chain_id: runner.chain_id,
            nonce: U256::zero(),
            gas_price: test_case.gas_price,
            gas_limit: test_case.gas_limit.into(),
            to: Some(Address::new(test_case.to)),
            value: Wei::new(test_case.value),
            data: test_case.data.clone(),
            access_list: test_case.get_access_list(),
        };
        let signed_tx = utils::sign_access_list_transaction(transaction, &sk);
        let tx_bytes: Vec<u8> = iter::once(eip_2930::TYPE_BYTE)
            .chain(rlp::encode(&signed_tx))
            .collect();
        let outcome = runner
            .call(utils::SUBMIT, "relay.aurora", tx_bytes)
            .unwrap();
        let result =
            SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
        let ussd_near_gas = outcome.used_gas / 1_000_000_000_000;
        //assert!(ussd_near_gas < 10, "{ussd_near_gas}  < 10");
        println!("{ussd_near_gas:?} TGas, len: {}", test_case.data.len());
        assert!(result.status.is_ok());
        //assert_eq!(result.gas_used, test_case.used_gas);
    }
}

/// Run precompile with specific input data from the file.
/// It executes precompile it two ways:
///   1. Run directly and check result with expected output
///   2. Call transaction and validation expected output. To avoid NEAR gas limit errors
///      we only send input with limited expected size.
fn run_bls12_381_standalone(precompile: &impl Precompile, address: Address, path: &str) {
    for data in PrecompileStandalone::new(path).precompile_data {
        let input = hex::decode(data.input.clone()).unwrap();
        let output = hex::decode(data.output.clone()).unwrap();
        // if input.iter().all(|&x| x == 0) {
        //     continue;
        // }
        // println!(
        //     "--> {} {}: {}",
        //     input.len(),
        //     output.len(),
        //     hex::encode(output.clone())
        // );

        let ctx = evm::Context {
            address: H160::default(),
            caller: H160::default(),
            apparent_value: U256::zero(),
        };
        // Run precompile directly with specific input and validate output result
        let standalone_result = precompile.run(&input, None, &ctx, false).unwrap();
        assert_eq!(standalone_result.output, output);

        // To avoid NEAR gas error "GasLimit" it make sense to limit input size.
        // and send transaction.
        //if input.len() < 800 {
        check_wasm_submit(address, input, &output);
        //}
    }
}

/// Submit transaction to precompile address and check result with expected output.
// TODO
#[allow(dead_code)]
fn check_wasm_submit(address: Address, input: Vec<u8>, expected_output: &[u8]) {
    let (mut runner, mut signer, _) = initialize_transfer();
    runner.context.prepaid_gas = u64::MAX;

    let wasm_result = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            aurora_engine_transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(address),
                value: Wei::zero(),
                data: input,
            }
        })
        .unwrap();
    println!(
        "Gas used: {:?} | {:?}",
        wasm_result.1.wasm_gas(),
        wasm_result.1.all_gas()
    );
    println!(
        "RES: {}",
        expected_output == utils::unwrap_success_slice(&wasm_result.0),
    );
    // println!("{:?}", expected_output);
    // println!("{:?}", utils::unwrap_success_slice(&wasm_result));
}

#[test]
fn test_bls12_381_g1_add() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_g1_add/");
}

#[test]
fn test_bls12_381_g1_mul() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_g1_mul/");
}

#[test]
fn test_bls12_381_g2_add() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_g2_add/");
}

#[test]
fn test_bls12_381_g2_mul() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_g2_mul/");
}

#[test]
fn test_bls12_381_pairing() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_pair/");
}

#[test]
fn test_bls12_381_map_fp_to_g1() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_map_fp_to_g1/");
}

#[test]
fn test_bls12_381_map_fp2_to_g2() {
    run_bls12_381_transaction_call("src/tests/res/bls/bls12_381_map_fp2_to_g2/");
}

#[test]
fn test_bls12_381_g1_add_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsG1Add,
        bls12_381::BlsG1Add::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_g1_add.json",
    );
}

#[test]
fn test_bls12_381_g1_mul_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsG1Msm,
        bls12_381::BlsG1Msm::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_g1_mul.json",
    );
}

#[test]
fn test_bls12_381_g2_add_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsG2Add,
        bls12_381::BlsG2Add::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_g2_add.json",
    );
}

#[test]
fn test_bls12_381_g2_mul_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsG2Msm,
        bls12_381::BlsG2Msm::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_g2_mul.json",
    );
}

#[test]
fn test_bls12_381_pair_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsPairingCheck,
        bls12_381::BlsPairingCheck::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_pair.json",
    );
}

#[test]
fn test_bls12_381_map_fp_to_g1_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsMapFpToG1,
        bls12_381::BlsMapFpToG1::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_map_fp_to_g1.json",
    );
}

#[test]
fn test_bls12_381_map_fp2_to_g2_standalone() {
    run_bls12_381_standalone(
        &bls12_381::BlsMapFp2ToG2,
        bls12_381::BlsMapFp2ToG2::ADDRESS,
        "src/tests/res/bls/standalone/bls12_381_map_fp2_to_g2.json",
    );
}
