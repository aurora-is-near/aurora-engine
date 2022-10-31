use crate::test_utils::{self, standalone};
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H160, U256};
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

#[test]
fn test_produce_snapshot() {
    let snapshot = json_snapshot::types::JsonSnapshot::load_from_file(
        "src/tests/res/contract.aurora.block51077328.json",
    )
    .unwrap();
    let mut runner = standalone::StandaloneRunner {
        chain_id: 1313161554,
        ..Default::default()
    };
    runner
        .storage
        .set_engine_account_id(&"aurora".parse().unwrap())
        .unwrap();
    json_snapshot::initialize_engine_state(&mut runner.storage, snapshot.clone()).unwrap();

    // add a couple more transactions that write some extra keys
    runner.env.block_height = snapshot.result.block_height + 1;
    let sk = libsecp256k1::SecretKey::parse(&[0x77; 32]).unwrap();
    let mut signer = test_utils::Signer::new(sk);
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
    let dest1 = Address::from_array([0x11; 20]);
    let dest2 = Address::from_array([0x22; 20]);
    let initial_balance = Wei::from_eth(U256::one()).unwrap();
    let transfer_amount = Wei::new_u64(100_000);
    runner.mint_account(signer_address, initial_balance, U256::zero(), None);

    runner
        .transfer_with_signer(&mut signer, transfer_amount, dest1)
        .unwrap();
    runner
        .transfer_with_signer(&mut signer, transfer_amount, dest2)
        .unwrap();

    // Take snapshot from before these transactions new are included
    let mut computed_snapshot = runner
        .storage
        .get_snapshot(snapshot.result.block_height)
        .unwrap();

    // Computed snapshot should exactly the same keys from initial snapshot
    for entry in snapshot.result.values.iter() {
        let key = base64::decode(&entry.key).unwrap();
        let value = base64::decode(&entry.value).unwrap();
        assert_eq!(computed_snapshot.remove(&key).unwrap(), value);
    }
    assert!(computed_snapshot.is_empty());

    // Take snapshot of current state
    let computed_snapshot = runner
        .storage
        .get_snapshot(runner.env.block_height)
        .unwrap();

    // New snapshot should still contain all keys from initial snapshot
    for entry in snapshot.result.values {
        let key = base64::decode(entry.key).unwrap();
        // skip the eth-connector keys; they were changed by minting the new account
        if key[0..3] == [7, 6, 1] {
            continue;
        }
        let value = base64::decode(entry.value).unwrap();
        assert_eq!(computed_snapshot.get(&key).unwrap(), &value);
    }

    // New snapshot should contain the keys from the new transactions as well
    let addr_info = [
        signer_address.as_bytes(),
        dest1.as_bytes(),
        dest2.as_bytes(),
    ]
    .into_iter()
    .zip([
        initial_balance - transfer_amount - transfer_amount,
        transfer_amount,
        transfer_amount,
    ])
    .zip([signer.nonce, 0, 0]);
    for ((address, balance), nonce) in addr_info {
        let balance_key = [&BALANCE_PREFIX, address].concat();
        let nonce_key = [&NONCE_PREFIX, address].concat();
        let balance_value = balance.to_bytes().to_vec();
        let nonce_value = {
            let mut buf = vec![0; 32];
            U256::from(nonce).to_big_endian(&mut buf);
            buf
        };
        assert_eq!(computed_snapshot.get(&balance_key).unwrap(), &balance_value);
        if nonce != 0 {
            assert_eq!(computed_snapshot.get(&nonce_key).unwrap(), &nonce_value);
        }
    }

    runner.close();
}

fn address_from_key(key: &[u8]) -> Address {
    let mut result = [0u8; 20];
    result.copy_from_slice(&key[2..22]);
    Address::new(H160(result))
}
