#![allow(dead_code)]
use crate::prelude::{Address, Wei, H160, H256, U256};
use crate::utils;
use aurora_engine_transactions::eip_2930;
use aurora_engine_transactions::eip_2930::{AccessTuple, Transaction2930};
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::parameters::engine::SubmitResult;
use evm::backend::MemoryAccount;
use libsecp256k1::SecretKey;
use std::collections::BTreeMap;
use std::iter;

/// State test dump data struct for fully reprodusing execution flow
/// with input & output and before & after state data.
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
    fn get_access_list(&self) -> Vec<AccessTuple> {
        let al = self.access_list.clone();
        al.iter()
            .map(|(address, key)| AccessTuple {
                address: *address,
                storage_keys: key.clone(),
            })
            .collect()
    }
}

/// Read State tests data from directory that contains json files
/// with specific test cases.
/// Return parsed Stete tests dump data
fn read_test_case() -> Vec<StateTestsDump> {
    use std::{fs, path::Path};

    fs::read_dir("src/tests/res/bls/")
        .expect("Read source test directory failed")
        .map(|entry| entry.unwrap().path())
        .filter(|entry| fs::metadata(entry).unwrap().is_file())
        .filter(|entry| {
            let file_name = entry.file_name().unwrap();
            Path::new(file_name).extension().unwrap().to_str() == Some("json")
        })
        .map(|entry| fs::read_to_string(entry).unwrap())
        .map(|data| serde_json::from_str(&data).unwrap())
        .collect::<Vec<_>>()
}

/// Get secret key from hash
fn get_secret_key(hash: H256) -> SecretKey {
    let mut secret_key = [0; 32];
    secret_key.copy_from_slice(hash.as_bytes());
    SecretKey::parse(&secret_key).expect("Unable to parse secret key")
}

#[test]
fn test_bls12_381_g1_add() {
    for test_case in read_test_case() {
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
        assert!(ussd_near_gas < 10);
        assert!(result.status.is_ok());
        assert_eq!(result.gas_used, test_case.used_gas);
    }
}
