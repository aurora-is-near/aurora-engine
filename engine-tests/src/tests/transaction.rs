use aurora_engine::parameters::SubmitResult;

use aurora_engine_transactions::eip_2930;
use aurora_engine_transactions::eip_2930::{
    ACCESS_LIST_LENGTH, ACCESS_LIST_STORAGE_KEY_LENGTH, SignedTransaction2930, Transaction2930,
};
use aurora_engine_types::H160;
use aurora_engine_types::borsh::BorshDeserialize;
use std::convert::TryFrom;
use std::iter;

use crate::prelude::Wei;
use crate::prelude::transactions::EthTransactionKind;
use crate::prelude::transactions::eip_1559::{self, SignedTransaction1559, Transaction1559};
use crate::prelude::transactions::eip_2930::AccessTuple;
use crate::prelude::{H256, U256};
use crate::utils;

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

/// Quantization step for NEAR-gas asserts: 0.1 Tgas = 100 Ggas.
const ACCESS_NEAR_GAS_STEP: u64 = 100_000_000_000;

/// Floor-round NEAR gas to nearest 0.1 Tgas step.
const fn access_round_near_gas(gas: u64) -> u64 {
    gas / ACCESS_NEAR_GAS_STEP * ACCESS_NEAR_GAS_STEP
}

/// Convert raw NEAR gas to the nearest 0.1 Tgas step, for more stable assertions.
const fn near_ggas(gas: u64) -> u64 {
    gas * ACCESS_NEAR_GAS_STEP
}

/// EVM `gas_limit` scaled to fit the intrinsic cost of a max-size `access_list`:
///   `gas_transaction_call (21_000) + ACCESS_LIST_LENGTH × gas_access_list_address (2_400)`
/// plus a 1 M headroom for EVM execution + refund.
///
/// This scales automatically when `ACCESS_LIST_LENGTH` is changed in
/// `engine-transactions` — no test-side tweak needed.
const ACCESS_MAX_LIST_EVM_GAS_LIMIT: u64 = 21_000 + (ACCESS_LIST_LENGTH as u64) * 2_400 + 1_000_000;

const RELAY_ACCOUNT: &str = "relay.aurora";

/// Build `n` AccessTuple entries with unique addresses and zero storage-keys.
/// Storage-keys are left empty — this suite only exercises the list-length cap,
/// not the per-tuple storage-keys length.
fn make_access_list(n: usize) -> Vec<AccessTuple> {
    make_access_list_with_keys(n, 0)
}

/// Build `n` AccessTuple entries, each carrying `keys_per` storage keys.
/// Keys are unique H256 values derived from the tuple index (avoids collisions
/// that might get deduped inside EVM warm-slot tracking).
fn make_access_list_with_keys(n: usize, keys_per: usize) -> Vec<AccessTuple> {
    (0..n)
        .map(|i| AccessTuple {
            address: H160::from_low_u64_be(i as u64 + 1),
            storage_keys: (0..keys_per)
                .map(|k| {
                    let mut b = [0u8; 32];
                    b[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                    b[8..16].copy_from_slice(&(k as u64).to_be_bytes());
                    H256(b)
                })
                .collect(),
        })
        .collect()
}

/// Build an EIP-1559 tx with a pre-built access list.
fn eip1559_tx_with_access_list(
    chain_id: u64,
    nonce: U256,
    gas_limit: U256,
    access_list: Vec<AccessTuple>,
) -> Transaction1559 {
    Transaction1559 {
        chain_id,
        nonce,
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        gas_limit,
        to: Some(utils::address_from_hex(CONTRACT_ADDRESS)),
        value: Wei::zero(),
        data: vec![],
        access_list,
    }
}

/// Build an EIP-2930 tx with a pre-built access list.
fn eip2930_tx_with_access_list(
    chain_id: u64,
    nonce: U256,
    gas_limit: U256,
    access_list: Vec<AccessTuple>,
) -> Transaction2930 {
    Transaction2930 {
        chain_id,
        nonce,
        gas_price: U256::from(0x07d0),
        gas_limit,
        to: Some(utils::address_from_hex(CONTRACT_ADDRESS)),
        value: Wei::zero(),
        data: vec![],
        access_list,
    }
}

/// Serialise a signed EIP-1559 tx to the byte-stream the contract expects.
fn encode_signed_1559(signed: &SignedTransaction1559) -> Vec<u8> {
    iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(signed))
        .collect()
}

/// Serialise a signed EIP-2930 tx to the byte-stream the contract expects.
fn encode_signed_2930(signed: &SignedTransaction2930) -> Vec<u8> {
    iter::once(eip_2930::TYPE_BYTE)
        .chain(rlp::encode(signed))
        .collect()
}

/// Length cap — happy path. `access_list.len() == ACCESS_LIST_LENGTH` tx
/// must be accepted and executed. Captures the baseline NEAR gas for EIP-1559.
#[test]
fn test_eip_1559_access_list_max_length_succeeds() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let near_gas_used = outcome.used_gas.as_gas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert!(
        result.status.is_ok(),
        "tx must succeed at max access list length; status = {:?}",
        result.status
    );
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());

    assert_eq!(result.gas_used, 1_965_310);
    assert_eq!(access_round_near_gas(near_gas_used), near_ggas(160)); // 16.0 Tgas
}

/// Length cap — list size overruns the constant.
/// Rejected inside `SignedTransaction1559::decode` via `take(MAX+1).count()`,
/// BEFORE per-item `as_list()` decoding. Surfaced as `ERR_TX_RLP_DECODE`.
#[test]
fn test_eip_1559_access_list_exceeds_limit_rejected() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH + 1),
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_TX_RLP_DECODE");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(19)); // 1.9 Tgas
}

/// Length cap — max access list + wrong tx-level chain_id → rejected early.
#[test]
fn test_eip_1559_max_access_list_wrong_tx_chain_id_early_exit() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip1559_tx_with_access_list(
        runner.chain_id.wrapping_add(1),
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_INVALID_CHAIN_ID");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(71)); // 7.1 Tgas
}

/// Length cap — max access list + wrong tx-level nonce → rejected early.
#[test]
fn test_eip_1559_max_access_list_wrong_tx_nonce_early_exit() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        U256::from(9999u64),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert!(err.kind.as_bytes().starts_with(b"ERR_INCORRECT_NONCE"));
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(72)); // 7.2 Tgas
}

/// Length cap — max access list + sender cannot afford `gas_limit * max_fee_per_gas`.
#[test]
fn test_eip_1559_max_access_list_insufficient_balance_early_exit() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, Wei::new_u64(100), signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_OUT_OF_FUND");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(73)); // 7.3 Tgas
}

/// Length cap — happy path for EIP-2930. Same contract as EIP-1559.
#[test]
fn test_eip_2930_access_list_max_length_succeeds() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let near_gas_used = outcome.used_gas.as_gas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert!(result.status.is_ok());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());

    assert_eq!(result.gas_used, 1_965_310);
    assert_eq!(access_round_near_gas(near_gas_used), near_ggas(160)); // 16.0 Tgas
}

/// Length cap — list size overruns the constant (EIP-2930).
#[test]
fn test_eip_2930_access_list_exceeds_limit_rejected() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH + 1),
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_TX_RLP_DECODE");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(19)); // 1.9 Tgas
}

/// Length cap — max access list + wrong tx-level chain_id (EIP-2930).
#[test]
fn test_eip_2930_max_access_list_wrong_tx_chain_id_early_exit() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip2930_tx_with_access_list(
        runner.chain_id.wrapping_add(1),
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_INVALID_CHAIN_ID");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(71)); // 7.1 Tgas
}

/// Length cap — max access list + wrong tx-level nonce (EIP-2930).
#[test]
fn test_eip_2930_max_access_list_wrong_tx_nonce_early_exit() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        U256::from(9999u64),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert!(err.kind.as_bytes().starts_with(b"ERR_INCORRECT_NONCE"));
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(72)); // 7.2 Tgas
}

/// Length cap — max access list + insufficient balance (EIP-2930).
#[test]
fn test_eip_2930_max_access_list_insufficient_balance_early_exit() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, Wei::new_u64(100), signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        ACCESS_MAX_LIST_EVM_GAS_LIMIT.into(),
        make_access_list(ACCESS_LIST_LENGTH),
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_OUT_OF_FUND");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(73)); // 7.3 Tgas
}

/// Storage-keys cap — happy path at the upper bound.
/// 1 AccessTuple carrying `ACCESS_LIST_STORAGE_KEY_LENGTH` storage keys.
/// Exercises the per-tuple cap from the "minimum tuples × max keys" direction.
/// Validates that such a tx is accepted and executed; captures NEAR + EVM gas.
#[test]
fn test_eip_1559_single_tuple_max_storage_keys_succeeds() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    // 1 tuple × ACCESS_LIST_STORAGE_KEY_LENGTH storage keys.
    let access_list = make_access_list_with_keys(1, ACCESS_LIST_STORAGE_KEY_LENGTH);
    // intrinsic: 21_000 + 1 × 2_400 + 20 × 1_900 = 61_400, plus EVM exec.
    let evm_gas_limit: u64 =
        21_000 + 2_400 + (ACCESS_LIST_STORAGE_KEY_LENGTH as u64) * 1_900 + 1_000_000;

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        evm_gas_limit.into(),
        access_list,
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let near_gas_used = outcome.used_gas.as_gas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert!(result.status.is_ok());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(result.gas_used, 85_710);
    assert_eq!(access_round_near_gas(near_gas_used), near_ggas(41)); // 4.1 Tgas
}

/// Combined worst case — `ACCESS_LIST_LENGTH × ACCESS_LIST_STORAGE_KEY_LENGTH` slots.
///
/// At the current caps (800 × 20 = 16_000 slots) this payload **does NOT fit**
/// the NEAR 300 Tgas per-tx cap — the engine's decoder-level caps enforce only
/// an upper bound on ACCEPTED payload, they are NOT a guarantee that every
/// combination within the caps fits the NEAR gas budget. This test documents
/// and asserts that behaviour: tx passes decoding, but wasm panics with
/// `HostError(GasLimitExceeded)` inside `runner.call`.
#[test]
fn test_eip_1559_access_list_combined_max_success() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let access_list =
        make_access_list_with_keys(ACCESS_LIST_LENGTH, ACCESS_LIST_STORAGE_KEY_LENGTH);
    let evm_gas_limit: u64 = 21_000
        + (ACCESS_LIST_LENGTH as u64) * 2_400
        + (ACCESS_LIST_LENGTH as u64) * (ACCESS_LIST_STORAGE_KEY_LENGTH as u64) * 1_900
        + 1_000_000;

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        evm_gas_limit.into(),
        access_list,
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);
    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let near_gas_used = outcome.used_gas.as_gas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert!(result.status.is_ok());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(result.gas_used, 85_710);
    assert_eq!(access_round_near_gas(near_gas_used), near_ggas(41)); // 4.1 Tgas
}

/// Storage-keys cap — rejection at decoder level.
/// 1 AccessTuple with `(ACCESS_LIST_STORAGE_KEY_LENGTH + 1)` storage keys.
/// `AccessTuple::decode` returns `DecoderError::Custom("ERR_STORAGE_KEYS_TOO_LARGE")`,
/// surfaced as `ERR_TX_RLP_DECODE` through the error chain. NEAR gas stays minimal.
#[test]
fn test_eip_1559_storage_keys_exceeds_limit_rejected() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    // 1 tuple × (MAX_K + 1) keys — over the per-tuple cap.
    let access_list = make_access_list_with_keys(1, ACCESS_LIST_STORAGE_KEY_LENGTH + 1);
    let evm_gas_limit: u64 =
        21_000 + 2_400 + (ACCESS_LIST_STORAGE_KEY_LENGTH as u64 + 1) * 1_900 + 1_000_000;

    let tx = eip1559_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        evm_gas_limit.into(),
        access_list,
    );
    let signed_tx = utils::sign_eip_1559_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_TX_RLP_DECODE");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(14)); // 1.4 Tgas
}

/// EIP-2930 mirror of [`test_eip_1559_single_tuple_max_storage_keys_succeeds`].
#[test]
fn test_eip_2930_single_tuple_max_storage_keys_succeeds() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let access_list = make_access_list_with_keys(1, ACCESS_LIST_STORAGE_KEY_LENGTH);
    let evm_gas_limit: u64 =
        21_000 + 2_400 + (ACCESS_LIST_STORAGE_KEY_LENGTH as u64) * 1_900 + 1_000_000;

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        evm_gas_limit.into(),
        access_list,
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let near_gas_used = outcome.used_gas.as_gas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert!(result.status.is_ok());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(result.gas_used, 85_710);
    assert_eq!(access_round_near_gas(near_gas_used), near_ggas(41)); // 4.1 Tgas
}

/// EIP-2930 mirror of [`test_eip_1559_combined_max_exceeds_near_gas_cap`].
#[test]
fn test_eip_2930_access_list_combined_max_success() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(CONTRACT_CODE).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let access_list =
        make_access_list_with_keys(ACCESS_LIST_LENGTH, ACCESS_LIST_STORAGE_KEY_LENGTH);
    let evm_gas_limit: u64 = 21_000
        + (ACCESS_LIST_LENGTH as u64) * 2_400
        + (ACCESS_LIST_LENGTH as u64) * (ACCESS_LIST_STORAGE_KEY_LENGTH as u64) * 1_900
        + 1_000_000;

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        evm_gas_limit.into(),
        access_list,
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let near_gas_used = outcome.used_gas.as_gas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert!(result.status.is_ok());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(result.gas_used, 85_710);
    assert_eq!(access_round_near_gas(near_gas_used), near_ggas(41)); // 4.1 Tgas≠
}

/// EIP-2930 mirror of [`test_eip_1559_storage_keys_exceeds_limit_rejected`].
#[test]
fn test_eip_2930_storage_keys_exceeds_limit_rejected() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        hex::decode(CONTRACT_CODE).unwrap(),
    );

    let access_list = make_access_list_with_keys(1, ACCESS_LIST_STORAGE_KEY_LENGTH + 1);
    let evm_gas_limit: u64 =
        21_000 + 2_400 + (ACCESS_LIST_STORAGE_KEY_LENGTH as u64 + 1) * 1_900 + 1_000_000;

    let tx = eip2930_tx_with_access_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        evm_gas_limit.into(),
        access_list,
    );
    let signed_tx = utils::sign_access_list_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_2930(&signed_tx);

    let err = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes)
        .unwrap_err();

    assert_eq!(err.kind.as_bytes(), b"ERR_TX_RLP_DECODE");
    assert_eq!(runner.get_nonce(signer_address), signer.nonce.into());
    assert_eq!(access_round_near_gas(err.gas_used), near_ggas(14)); // 1.4 Tgas
}
