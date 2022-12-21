use crate::prelude::Wei;
use crate::test_utils::{self, ExecutionProfile};

use aurora_engine::parameters::SubmitResult;
use eth_json_test::test_types::transaction_test::{
    TransactionTest, TtResult, TtResultErr, TtResultOk,
};
use libsecp256k1::SecretKey;
use near_vm_runner::VMError;
use rustc_hex::FromHex;

const INITIAL_BALANCE: u64 = 1_000_000;
const INITIAL_NONCE: u64 = 0;

fn hexstr_to_bytes(value: &str) -> Vec<u8> {
    let v = match value.len() {
        0 => vec![],
        2 if value.starts_with("0x") => vec![],
        _ if value.starts_with("0x") && value.len() % 2 == 1 => {
            let v = "0".to_owned() + &value[2..];
            FromHex::from_hex(v.as_str()).unwrap_or_default()
        }
        _ if value.starts_with("0x") => FromHex::from_hex(&value[2..]).unwrap_or_default(),
        _ => FromHex::from_hex(value).unwrap_or_default(),
    };

    v
}

fn initialize_runner() -> test_utils::AuroraRunner {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );
    runner
}

fn run(path: String, name: String) {
    let mut runner = initialize_runner();

    // Bring up the test json file
    let tt_json = TransactionTest::new(path, name);
    let tx_bytes_str = &tt_json.txbytes;
    let txbytes: Vec<u8> = hexstr_to_bytes(tx_bytes_str);

    // Do transaction with tx bytes as data
    let outcome: Result<(SubmitResult, ExecutionProfile), VMError> =
        runner.submit_transaction_raw(txbytes);
    match tt_json.result("London".to_string()) {
        TtResult::TtResultOk {
            hash,
            intrinsic_gas,
            sender,
        } => {
            let _ok = TtResultOk {
                hash,
                intrinsic_gas,
                sender,
            };
            // TODO: Check outcome with parsed transaction property
            assert!(outcome.is_ok())
        }
        TtResult::TtResultErr {
            exception,
            intrinsic_gas,
        } => {
            let _err = TtResultErr {
                exception,
                intrinsic_gas,
            };
            assert!(outcome.is_err())
            // TODO: Add exceptions to engine and test reasons on transaction parser
            // assert_eq!(outcome.to_string(), exception);
        }
    };
}

// Test individual tests in root directory
// cargo test --package aurora-engine-tests --features mainnet-test --lib -- tests::transaction_tests::test_address_less_than_20 --exact --nocapture
#[test]
fn test_address_less_than_20() {
    run(
        "etc/eth-json-test/res/tests/TransactionTests/ttAddress/AddressLessThan20.json".to_string(),
        "AddressLessThan20".to_string(),
    )
}

// Passing tests
#[test]
fn test_address_less_than_20_prefixed() {
    run(
        "etc/eth-json-test/res/tests/TransactionTests/ttAddress/AddressLessThan20Prefixed0.json"
            .to_string(),
        "AddressLessThan20Prefixed0".to_string(),
    );
}
