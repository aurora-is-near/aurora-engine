use crate::prelude::transactions::eip_1559::{self, SignedTransaction1559, Transaction1559};
use crate::prelude::transactions::eip_2930::AccessTuple;
use crate::prelude::transactions::EthTransactionKind;
use crate::prelude::Wei;
use crate::prelude::{H256, U256};
use crate::test_utils;
use aurora_engine::parameters::SubmitResult;
use borsh::BorshDeserialize;
use std::convert::TryFrom;
use std::iter;

const SECRET_KEY: &str = "45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8";
const INITIAL_NONCE: u64 = 1;
const INITIAL_BALANCE: Wei = Wei::new_u64(0x0de0b6b3a7640000);

const CONTRACT_ADDRESS: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const CONTRACT_NONCE: u64 = 1;
const CONTRACT_CODE: &str = "3a6000554860015500";
const CONTRACT_BALANCE: Wei = Wei::new_u64(0x0de0b6b3a7640000);

const EXAMPLE_TX_HEX: &str = "02f8c101010a8207d0833d090094cccccccccccccccccccccccccccccccccccccccc8000f85bf85994ccccccccccccccccccccccccccccccccccccccccf842a00000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000180a0d671815898b8dd34321adbba4cb6a57baa7017323c26946f3719b00e70c755c2a03528b9efe3be57ea65a933d1e6bbf3b7d0c78830138883c1201e0c641fee6464";

// Test taken from https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stExample/eip1559.json
// TODO(#170): generally support Ethereum tests
#[test]
fn test_eip_1559_tx_encoding_decoding() {
    let secret_key = exmaple_signer().secret_key;
    let transaction = example_transaction();

    let signed_tx = test_utils::sign_eip_1559_transaction(transaction, &secret_key);
    let bytes = encode_tx(&signed_tx);
    let expected_bytes = hex::decode(EXAMPLE_TX_HEX).unwrap();

    assert_eq!(bytes, expected_bytes);

    let decoded_tx = match EthTransactionKind::try_from(expected_bytes.as_slice()) {
        Ok(EthTransactionKind::Eip1559(tx)) => tx,
        Ok(_) => panic!("Unexpected transaction type"),
        Err(_) => panic!("Transaction parsing failed"),
    };

    assert_eq!(signed_tx, decoded_tx);

    assert_eq!(
        signed_tx.sender().unwrap(),
        test_utils::address_from_secret_key(&secret_key)
    )
}

// Test inspired by https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stExample/eip1559.json
// but modified slightly because our BASEFEE is always 0.
#[test]
fn test_eip_1559_example() {
    let mut runner = test_utils::deploy_evm();
    let mut signer = exmaple_signer();
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
    let contract_address = test_utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    // Check initial state
    assert_eq!(runner.get_balance(signer_address), INITIAL_BALANCE);
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);

    let mut transaction = example_transaction();
    transaction.chain_id = runner.chain_id;
    signer.use_nonce();
    let signed_tx = test_utils::sign_eip_1559_transaction(transaction, &signer.secret_key);

    let sender = "relay.aurora";
    let (maybe_outcome, maybe_err) = runner.call(test_utils::SUBMIT, sender, encode_tx(&signed_tx));
    assert!(maybe_err.is_none());
    let result =
        SubmitResult::try_from_slice(&maybe_outcome.unwrap().return_data.as_value().unwrap())
            .unwrap();
    assert_eq!(result.gas_used, 0xb8d2);

    // Check post state:
    // signer spent some ETH on gas fees and incremented nonce for submitting transaction
    assert_eq!(
        runner.get_balance(signer_address),
        Wei::new_u64(0x0de0b6b3a75cc7cc)
    );
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    // Contract balance, code, nonce all unchanged, but storage was written
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);
    assert_eq!(
        runner.get_storage(contract_address, H256::zero()),
        h256_from_hex("000000000000000000000000000000000000000000000000000000000000000a")
    );
    assert_eq!(runner.get_storage(contract_address, one()), H256::zero());
    // Gas fees were awarded to the address derived from sending account
    let coinbase = aurora_engine_sdk::types::near_account_to_evm_address(sender.as_bytes());
    assert_eq!(runner.get_balance(coinbase), Wei::new_u64(0x73834));
}

fn encode_tx(signed_tx: &SignedTransaction1559) -> Vec<u8> {
    iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(signed_tx).into_iter())
        .collect()
}

fn exmaple_signer() -> test_utils::Signer {
    let secret_key =
        libsecp256k1::SecretKey::parse_slice(&hex::decode(SECRET_KEY).unwrap()).unwrap();

    test_utils::Signer {
        nonce: INITIAL_NONCE,
        secret_key,
    }
}

fn example_transaction() -> Transaction1559 {
    Transaction1559 {
        chain_id: 1,
        nonce: U256::from(INITIAL_NONCE),
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: Some(test_utils::address_from_hex(CONTRACT_ADDRESS)),
        value: Wei::zero(),
        data: vec![0],
        access_list: vec![AccessTuple {
            address: test_utils::address_from_hex(CONTRACT_ADDRESS).raw(),
            storage_keys: vec![H256::zero(), one()],
        }],
    }
}

fn h256_from_hex(hex: &str) -> H256 {
    let bytes = hex::decode(hex).unwrap();
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    H256(result)
}

fn one() -> H256 {
    let mut x = [0u8; 32];
    x[31] = 1;
    H256(x)
}
