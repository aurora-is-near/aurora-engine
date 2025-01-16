use crate::prelude::transactions::eip_1559::{self, SignedTransaction1559, Transaction1559};
use crate::prelude::transactions::eip_2930::AccessTuple;
use crate::prelude::transactions::EthTransactionKind;
use crate::prelude::Wei;
use crate::prelude::{H256, U256};
use crate::utils;
use aurora_engine::parameters::SubmitResult;
use aurora_engine_transactions::eip_2930::Transaction2930;
use aurora_engine_transactions::eip_7702::{AuthorizationTuple, Transaction7702};
use aurora_engine_transactions::{eip_2930, eip_7702};
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::types::Address;
use aurora_engine_types::H160;
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

/*
  PUSH20 0xd16292912e440956828f1a847ca6efc8412f45b6 [0x73d16292912e440956828f1a847ca6efc8412f45b6]
  EXTCODESIZE [0x3B]
  PUSH1 0x00 [0x6000]
  SSTORE [0x55]
  STOP [0x00]
*/
const CONTRACT_CODE_EIP7702: &str = "73d16292912e440956828f1a847ca6efc8412f45b63B60005500";

// Test taken from https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stExample/eip1559.json
// TODO(#170): generally support Ethereum tests
#[test]
fn test_eip_1559_tx_encoding_decoding() {
    let secret_key = example_signer().secret_key;
    let transaction = example_transaction();

    let signed_tx = utils::sign_eip_1559_transaction(transaction, &secret_key);
    let bytes = encode_tx(&signed_tx);
    let expected_bytes = hex::decode(EXAMPLE_TX_HEX).unwrap();

    assert_eq!(bytes, expected_bytes);

    let decoded_tx = match EthTransactionKind::try_from(expected_bytes.as_slice()) {
        Ok(EthTransactionKind::Eip1559(tx)) => tx,
        Ok(_) => panic!("Unexpected transaction type"),
        Err(e) => panic!("Transaction parsing failed: {e:?}"),
    };

    assert_eq!(signed_tx, decoded_tx);

    assert_eq!(
        signed_tx.sender().unwrap(),
        utils::address_from_secret_key(&secret_key)
    );
}

// Test inspired by https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stExample/eip1559.json
// but modified slightly because our BASEFEE is always 0.
#[test]
fn test_eip_1559_example() {
    let mut runner = utils::deploy_runner();
    let mut signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
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
    let signed_tx = utils::sign_eip_1559_transaction(transaction, &signer.secret_key);

    let sender = "relay.aurora";
    let outcome = runner
        .call(utils::SUBMIT, sender, encode_tx(&signed_tx))
        .unwrap();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
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

// Test taken from https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stExample/accessListExample.json
// TODO(#170): generally support Ethereum tests
#[test]
fn test_access_list_tx_encoding_decoding() {
    let secret_key = libsecp256k1::SecretKey::parse_slice(
        &hex::decode("45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8").unwrap(),
    )
    .unwrap();
    let transaction = Transaction2930 {
        chain_id: 1,
        nonce: U256::zero(),
        gas_price: U256::from(0x0a),
        gas_limit: U256::from(0x061a80),
        to: Some(utils::address_from_hex(
            "0x095e7baea6a6c7c4c2dfeb977efac326af552d87",
        )),
        value: Wei::new_u64(0x0186a0),
        data: vec![0],
        access_list: vec![
            AccessTuple {
                address: utils::address_from_hex("0x095e7baea6a6c7c4c2dfeb977efac326af552d87")
                    .raw(),
                storage_keys: vec![H256::zero(), one()],
            },
            AccessTuple {
                address: utils::address_from_hex("0x195e7baea6a6c7c4c2dfeb977efac326af552d87")
                    .raw(),
                storage_keys: vec![H256::zero()],
            },
        ],
    };

    let signed_tx = utils::sign_access_list_transaction(transaction, &secret_key);
    let bytes: Vec<u8> = iter::once(eip_2930::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx))
        .collect();
    let expected_bytes = hex::decode("01f8f901800a83061a8094095e7baea6a6c7c4c2dfeb977efac326af552d87830186a000f893f85994095e7baea6a6c7c4c2dfeb977efac326af552d87f842a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001f794195e7baea6a6c7c4c2dfeb977efac326af552d87e1a0000000000000000000000000000000000000000000000000000000000000000080a011c97e0bb8a356fe4f49b37863d059c6fe8cd3214a6ac06a8387a2f6f0b75f60a0212368a1097da30806edfd13d9c35662e1baee939235eb25de867980bd0eda26").unwrap();

    assert_eq!(bytes, expected_bytes);

    let decoded_tx = match EthTransactionKind::try_from(expected_bytes.as_slice()) {
        Ok(EthTransactionKind::Eip2930(tx)) => tx,
        Ok(_) => panic!("Unexpected transaction type"),
        Err(e) => panic!("Transaction parsing failed: {e:?}"),
    };

    assert_eq!(signed_tx, decoded_tx);

    assert_eq!(
        signed_tx.sender().unwrap(),
        utils::address_from_secret_key(&secret_key)
    );
}

#[test]
fn test_eip_7702_tx_encoding_decoding() {
    let secret_key = example_signer().secret_key;
    let transaction = eip7702_transaction(0);

    let signed_tx = utils::sign_eip_7702_transaction(transaction, &secret_key);
    let tx_bytes: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx))
        .collect();

    let decoded_tx = match EthTransactionKind::try_from(tx_bytes.as_slice()) {
        Ok(EthTransactionKind::Eip7702(tx)) => tx,
        Ok(_) => panic!("Unexpected transaction type"),
        Err(e) => panic!("Transaction parsing failed: {e:?}"),
    };

    assert_eq!(signed_tx, decoded_tx);
    assert_eq!(
        signed_tx.sender().unwrap(),
        utils::address_from_secret_key(&secret_key)
    );
}

#[test]
fn test_eip_7702_success() {
    // 0xa52a8a2229e3c512d6ed27b6e6e7d39958ca9fb3,
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE_EIP7702).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    let mut transaction = eip7702_transaction(0);
    transaction.chain_id = runner.chain_id;
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx))
        .collect();

    let sender = "relay.aurora";
    let outcome = runner.call(utils::SUBMIT, sender, tx_bytes).unwrap();
    // Unwrapping execution results validates outcome
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    let delegated_designator = Address::decode("d16292912e440956828f1a847ca6efc8412f45b6").unwrap();

    assert_eq!(result.gas_used, 68206);
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);
    // `EXTCODESIZE` should return size of `EF01` = 2 for delegated designator
    assert_eq!(
        runner.get_storage(contract_address, H256::zero()),
        H256::from(H160::from_low_u64_be(2))
    );

    // Authority address should increase Nonce
    assert_eq!(runner.get_nonce(delegated_designator), 1.into());
    // Get delegated designator address
    assert_eq!(
        hex::encode(runner.get_code(delegated_designator)),
        "ef0100cccccccccccccccccccccccccccccccccccccccc"
    );
}

#[test]
fn test_eip_7702_wrong_auth_nonce() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    // Contract for auth address: 0xa52a8a2229e3c512d6ed27b6e6e7d39958ca9fb3
    let contract_code =
        hex::decode("73a52a8a2229e3c512d6ed27b6e6e7d39958ca9fb33B60005500").unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    let mut transaction = eip7702_transaction(10);
    transaction.chain_id = runner.chain_id;
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx))
        .collect();

    let sender = "relay.aurora";
    let outcome = runner.call(utils::SUBMIT, sender, tx_bytes).unwrap();
    // Unwrapping execution results validates outcome
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    let delegated_designator = Address::decode("a52a8a2229e3c512d6ed27b6e6e7d39958ca9fb3").unwrap();

    assert_eq!(result.gas_used, 50806);
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);
    // `EXTCODESIZE` should return zero, as `authorization_list` failed validation
    assert_eq!(
        runner.get_storage(contract_address, H256::zero()),
        H256::zero()
    );

    // Authority address should increase Nonce
    assert_eq!(runner.get_nonce(delegated_designator), 0.into());
    // Get delegated designator address: in that particular case it should be empty
    assert!(runner.get_code(delegated_designator).is_empty());
}

fn encode_tx(signed_tx: &SignedTransaction1559) -> Vec<u8> {
    iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(signed_tx))
        .collect()
}

fn example_signer() -> utils::Signer {
    let secret_key =
        libsecp256k1::SecretKey::parse_slice(&hex::decode(SECRET_KEY).unwrap()).unwrap();

    utils::Signer {
        nonce: INITIAL_NONCE,
        secret_key,
    }
}

fn eip7702_transaction(auth_chain_id: u64) -> Transaction7702 {
    Transaction7702 {
        chain_id: 1,
        nonce: INITIAL_NONCE.into(),
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: utils::address_from_hex(CONTRACT_ADDRESS),
        value: Wei::zero(),
        data: vec![],
        access_list: vec![],
        authorization_list: vec![AuthorizationTuple {
            chain_id: auth_chain_id.into(),
            address: utils::address_from_hex(CONTRACT_ADDRESS).raw(),
            nonce: 0,
            parity: 1.into(),
            r: 2.into(),
            s: 3.into(),
        }],
    }
}

fn example_transaction() -> Transaction1559 {
    Transaction1559 {
        chain_id: 1,
        nonce: U256::from(INITIAL_NONCE),
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: Some(utils::address_from_hex(CONTRACT_ADDRESS)),
        value: Wei::zero(),
        data: vec![0],
        access_list: vec![AccessTuple {
            address: utils::address_from_hex(CONTRACT_ADDRESS).raw(),
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

const fn one() -> H256 {
    let mut x = [0u8; 32];
    x[31] = 1;
    H256(x)
}
