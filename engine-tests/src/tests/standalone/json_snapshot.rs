use crate::test_utils::standalone;
use aurora_engine_types::{Address, U256};
use engine_standalone_storage::json_snapshot;

const NONCE_PREFIX: [u8; 2] = [0x07, 0x01];
const BALANCE_PREFIX: [u8; 2] = [0x07, 0x02];
const CODE_PREFIX: [u8; 2] = [0x07, 0x03];

#[test]
fn test_consume_snapshot() {
    let snapshot = json_snapshot::types::JsonSnapshot::load_from_file(
        "src/tests/res/contract.aurora.block51077328.json",
    )
    .unwrap();
    let mut runner = standalone::StandaloneRunner::default();
    json_snapshot::initialize_engine_state(&mut runner.storage, snapshot.clone()).unwrap();

    // check accounts to see they were written properly
    runner.env.block_height = snapshot.result.block_height + 1;
    for entry in snapshot.result.values {
        let key = base64::decode(entry.key).unwrap();
        let value = base64::decode(entry.value).unwrap();
        if key.as_slice().starts_with(&NONCE_PREFIX) {
            let address = address_from_key(&key);
            let nonce = U256::from_big_endian(&value);
            assert_eq!(nonce, runner.get_nonce(&address))
        } else if key.as_slice().starts_with(&BALANCE_PREFIX) {
            let address = address_from_key(&key);
            let balance = U256::from_big_endian(&value);
            assert_eq!(balance, runner.get_balance(&address).raw())
        } else if key.as_slice().starts_with(&CODE_PREFIX) {
            let address = address_from_key(&key);
            assert_eq!(value, runner.get_code(&address))
        }
    }

    runner.close();
}

fn address_from_key(key: &[u8]) -> Address {
    let mut result = [0u8; 20];
    result.copy_from_slice(&key[2..22]);
    Address(result)
}
