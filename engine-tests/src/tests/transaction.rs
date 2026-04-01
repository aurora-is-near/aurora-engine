use aurora_engine::parameters::SubmitResult;
use aurora_engine_transactions::eip_2930::Transaction2930;
use aurora_engine_transactions::eip_7702::Transaction7702;
use aurora_engine_transactions::{eip_2930, eip_7702};
use aurora_engine_types::H160;
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::types::Address;
use std::convert::TryFrom;
use std::iter;

use crate::prelude::Wei;
use crate::prelude::transactions::EthTransactionKind;
use crate::prelude::transactions::eip_1559::{self, SignedTransaction1559, Transaction1559};
use crate::prelude::transactions::eip_2930::AccessTuple;
use crate::prelude::{H256, U256};
use crate::utils;
use crate::utils::{sign_eip_1559_transaction, sign_eip7702_authorization};

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
    let mut runner = utils::deploy_runner();

    // Signer data
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    // ── Authority: gets delegation, then sends a tx ──
    let authority_sk = example_authority_signer();
    let authority_address = utils::address_from_secret_key(&authority_sk.secret_key);

    // Contract data
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = sample_code_for_contract_eip7702(authority_address);

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

    let relay = "relay.aurora";
    let outcome = runner.call(utils::SUBMIT, relay, tx_bytes).unwrap();
    // Unwrapping execution results validates outcome
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(result.gas_used, 68206);
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);
    // `EXTCODESIZE` should return size of `EF0100`+20 = 23 for delegated designator
    assert_eq!(
        runner.get_storage(contract_address, H256::zero()),
        H256::from(H160::from_low_u64_be(23))
    );

    // Authority address should increase Nonce
    assert_eq!(runner.get_nonce(authority_address), 1.into());
    // Get delegated designator address
    assert_eq!(
        hex::encode(runner.get_code(authority_address)),
        "ef0100cccccccccccccccccccccccccccccccccccccccc"
    );
}

/// Test: account with EIP-7702 delegated code can send transactions
///
/// Step 1: EIP-7702 tx installs delegation on authority address
/// Step 2: EIP-1559 tx sent FROM that authority address succeeds
/// Step 3 — Sponsored tx: signer calls authority, triggering delegated code in authority's context
/// Step 4: revoke authority delegation
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
#[test]
fn test_eip_7702_delegated_sender_can_transact() {
    let mut runner = utils::deploy_runner();

    // ── Signer: sends the EIP-7702 tx ──
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    // ── Authority: gets delegation, then sends a tx ──
    let authority_sk = example_authority_signer();
    let authority_address = utils::address_from_secret_key(&authority_sk.secret_key);

    // ── Contract: delegation target ──
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    // Build contract code: PUSH20 <authority> | EXTCODESIZE | PUSH1 0 | SSTORE | STOP
    let contract_code = sample_code_for_contract_eip7702(authority_address);

    // ── Recipient for the second (transfer) tx ──
    let recipient = utils::address_from_hex("0x1111111111111111111111111111111111111111");

    // ── Fund accounts ──
    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    // Authority MUST start with nonce 0 — EIP-7702 authorization verifies this.
    // Needs balance to cover the value transfer in Step 2.
    runner.create_address(authority_address, INITIAL_BALANCE, 0.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    // ══════════════════════════════════════════════════════════════
    // Step 1: EIP-7702 tx — install delegation on authority
    // ══════════════════════════════════════════════════════════════
    let mut transaction = eip7702_transaction(0);
    transaction.chain_id = runner.chain_id;
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx))
        .collect();

    let relay = "relay.aurora";
    let outcome = runner.call(utils::SUBMIT, relay, tx_bytes).unwrap();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);
    // `EXTCODESIZE` should return size of `EF0100`+20 = 23 for delegated designator
    assert_eq!(
        runner.get_storage(contract_address, H256::zero()),
        H256::from(H160::from_low_u64_be(23))
    );

    // ── Verify delegation was installed ──
    let expected_delegation_code = format!("ef0100{}", hex::encode(contract_address.as_bytes()));
    assert_eq!(
        hex::encode(runner.get_code(authority_address)),
        expected_delegation_code,
        "authority must have EF0100 delegation designator"
    );
    assert_eq!(
        runner.get_nonce(authority_address),
        1.into(),
        "authority nonce must be 1 after EIP-7702 setup"
    );

    // ══════════════════════════════════════════════════════════════
    // Step 2: EIP-1559 tx FROM authority (which now has delegated code)
    // ══════════════════════════════════════════════════════════════
    let transfer_value = Wei::new_u64(1_000);

    let eip1559_tx = Transaction1559 {
        chain_id: runner.chain_id,
        nonce: 1.into(), // authority nonce is 1 after Step 1
        max_priority_fee_per_gas: U256::zero(),
        max_fee_per_gas: U256::zero(),
        gas_limit: 21_000.into(),
        to: Some(recipient),
        value: transfer_value,
        data: Vec::new(),
        access_list: Vec::new(),
    };

    let signed_tx_2 = utils::sign_eip_1559_transaction(eip1559_tx, &authority_sk.secret_key);
    let tx_bytes_2: Vec<u8> = iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx_2))
        .collect();

    let outcome_2 = runner.call(utils::SUBMIT, relay, tx_bytes_2).unwrap();
    let result_2 =
        SubmitResult::try_from_slice(&outcome_2.return_data.as_value().unwrap()).unwrap();

    // ── The tx MUST succeed: EIP-3607 must not reject EF0100 senders ──
    assert!(
        result_2.status.is_ok(),
        "tx from account with EIP-7702 delegation must succeed (EIP-3607 exception)"
    );

    // Nonce: 1 → 2
    assert_eq!(
        runner.get_nonce(authority_address),
        2.into(),
        "authority nonce must increment to 2"
    );
    // Delegation code must persist — sending a tx does not clear it
    assert_eq!(
        hex::encode(runner.get_code(authority_address)),
        expected_delegation_code,
        "delegation designator must survive after authority sends a tx"
    );
    assert_eq!(
        runner.get_balance(authority_address),
        INITIAL_BALANCE - transfer_value
    );
    // Storage slot should be empty
    assert_eq!(
        runner.get_storage(authority_address, H256::zero()),
        H256::from(H160::from_low_u64_be(0))
    );

    // Recipient received the value
    assert_eq!(
        runner.get_balance(recipient),
        transfer_value,
        "recipient must receive the transferred value"
    );

    // ══════════════════════════════════════════════════════════════
    // Step 3: EIP-1559 tx FROM signer - sponsored transaction
    // ══════════════════════════════════════════════════════════════
    let eip1559_tx2 = Transaction1559 {
        chain_id: runner.chain_id,
        nonce: 2.into(),
        max_priority_fee_per_gas: U256::zero(),
        max_fee_per_gas: U256::zero(),
        gas_limit: 0x3d0900.into(),
        to: Some(authority_address),
        value: Wei::new_u64(0),
        data: Vec::new(),
        access_list: Vec::new(),
    };

    let signed_tx_3 = utils::sign_eip_1559_transaction(eip1559_tx2, &signer.secret_key);
    let tx_bytes_3: Vec<u8> = iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx_3))
        .collect();

    let outcome_3 = runner.call(utils::SUBMIT, relay, tx_bytes_3).unwrap();
    let result_3 =
        SubmitResult::try_from_slice(&outcome_3.return_data.as_value().unwrap()).unwrap();
    assert!(result_3.status.is_ok());

    assert_eq!(runner.get_nonce(signer_address), 3.into());

    assert_eq!(
        runner.get_nonce(authority_address),
        2.into(),
        "authority nonce must remain 2 (sponsored tx does not increment authority nonce)"
    );
    // Delegation code must persist — sending a tx does not clear it
    assert_eq!(
        hex::encode(runner.get_code(authority_address)),
        expected_delegation_code,
        "delegation designator must survive after authority sends a tx"
    );
    assert_eq!(
        runner.get_balance(authority_address),
        INITIAL_BALANCE - transfer_value
    );
    // As sponsored transaction executed - storage slot now changed
    assert_eq!(
        runner.get_storage(authority_address, H256::zero()),
        H256::from(H160::from_low_u64_be(23))
    );

    // ══════════════════════════════════════════════════════════════
    // Step 4: revoke authority delegation
    // ══════════════════════════════════════════════════════════════
    let auth_revoke = sign_eip7702_authorization(
        0,
        Address::zero(),
        2, // Current nonce from authority EOA
        &authority_sk.secret_key,
    );

    let revoke_tx = Transaction7702 {
        chain_id: runner.chain_id,
        nonce: 3.into(), // signer nonce
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: contract_address,
        value: Wei::zero(),
        data: vec![],
        access_list: vec![],
        authorization_list: vec![auth_revoke],
    };

    let signed_tx_4 = utils::sign_eip_7702_transaction(revoke_tx, &authority_sk.secret_key);
    let tx_bytes_4: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx_4))
        .collect();

    let outcome_4 = runner.call(utils::SUBMIT, relay, tx_bytes_4).unwrap();
    let result_4 =
        SubmitResult::try_from_slice(&outcome_4.return_data.as_value().unwrap()).unwrap();
    assert!(result_4.status.is_ok());

    assert_eq!(runner.get_nonce(signer_address), 4.into());

    // Delegation revoked — code should be empty
    assert!(
        runner.get_code(authority_address).is_empty(),
        "authority code must be empty after revocation"
    );
    // Nonce increments: 2 → 3
    assert_eq!(
        runner.get_nonce(authority_address),
        3.into(),
        "authority nonce must be 3 after revocation"
    );
    // Balance unchanged
    assert_eq!(
        runner.get_balance(authority_address),
        INITIAL_BALANCE - transfer_value
    );
    // Storage slot unchanged
    assert_eq!(
        runner.get_storage(authority_address, H256::zero()),
        H256::from(H160::from_low_u64_be(23))
    );
}

/// Test: authority EOA revokes its own delegation.
///
/// Step 1: signer installs delegation on authority (EIP-7702)
/// Step 2: authority sends EIP-7702 tx to revoke its own delegation (address=0)
#[test]
fn test_eip_7702_authority_self_revoke() {
    let mut runner = utils::deploy_runner();

    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let authority_sk = example_authority_signer();
    let authority_address = utils::address_from_secret_key(&authority_sk.secret_key);

    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = sample_code_for_contract_eip7702(authority_address);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address(authority_address, INITIAL_BALANCE, 0.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    // ══════════════════════════════════════════════════════════════
    // Step 1: signer installs delegation on authority
    // ══════════════════════════════════════════════════════════════
    let mut transaction = eip7702_transaction(0);
    transaction.chain_id = runner.chain_id;
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx))
        .collect();
    let relay = "relay.aurora";
    let outcome = runner.call(utils::SUBMIT, relay, tx_bytes).unwrap();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    // Transfer to authority_address
    let tx_transfer = Transaction1559 {
        chain_id: runner.chain_id,
        nonce: U256::from(2),
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: Some(authority_address),
        value: Wei::new_u64(10),
        data: vec![],
        access_list: vec![],
    };
    let signed_tx_transfer = sign_eip_1559_transaction(tx_transfer, &signer.secret_key);
    let tx_bytes: Vec<u8> = iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx_transfer))
        .collect();
    let outcome = runner.call(utils::SUBMIT, relay, tx_bytes).unwrap();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    let expected_delegation_code = format!("ef0100{}", hex::encode(contract_address.as_bytes()));
    assert_eq!(
        hex::encode(runner.get_code(authority_address)),
        expected_delegation_code,
    );
    assert_eq!(runner.get_nonce(authority_address), 1.into());
    assert_eq!(runner.get_nonce(signer_address), 3.into());
    assert_eq!(
        runner.get_balance(authority_address),
        INITIAL_BALANCE + Wei::new_u64(10)
    );

    // ══════════════════════════════════════════════════════════════
    // Step 2: authority itself revokes delegation
    // ══════════════════════════════════════════════════════════════
    let auth_revoke = sign_eip7702_authorization(
        0,
        Address::zero(),
        2, // authority nonce AFTER sender nonce increment
        &authority_sk.secret_key,
    );

    let revoke_tx = Transaction7702 {
        chain_id: runner.chain_id,
        nonce: 1.into(), // authority's current nonce as tx sender
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: contract_address,
        value: Wei::zero(),
        data: vec![],
        access_list: vec![],
        authorization_list: vec![auth_revoke],
    };

    let signed_tx_2 = utils::sign_eip_7702_transaction(revoke_tx, &authority_sk.secret_key);
    let tx_bytes_2: Vec<u8> = iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx_2))
        .collect();

    let outcome_2 = runner.call(utils::SUBMIT, relay, tx_bytes_2).unwrap();
    let result_2 =
        SubmitResult::try_from_slice(&outcome_2.return_data.as_value().unwrap()).unwrap();
    assert!(result_2.status.is_ok());

    // Delegation revoked — code must be empty
    assert!(
        runner.get_code(authority_address).is_empty(),
        "authority code must be empty after self-revocation"
    );
    // Nonce: 1 → 2 (sender increment) → 3 (auth processing increment)
    assert_eq!(
        runner.get_nonce(authority_address),
        3.into(),
        "authority nonce: 1 +1 (sender) +1 (auth) = 3"
    );
    // Signer untouched
    assert_eq!(runner.get_nonce(signer_address), 3.into());

    // gas_price * gas_used
    let gas_fee = Wei::new_u64(0x0A * result_2.gas_used);
    assert_eq!(
        runner.get_balance(authority_address),
        INITIAL_BALANCE + Wei::new_u64(10) - gas_fee
    );
}

#[test]
fn test_eip_7702_wrong_auth_chain_id() {
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

    // Authority address should not increase Nonce because authorization failed
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

/// Build contract code: PUSH20 <authority> | EXTCODESIZE | PUSH1 0 | SSTORE | STOP
#[must_use]
fn sample_code_for_contract_eip7702(authority_address: Address) -> Vec<u8> {
    let mut code = Vec::with_capacity(26);
    code.push(0x73); // PUSH20
    code.extend_from_slice(authority_address.as_bytes());
    code.push(0x3B); // EXTCODESIZE
    code.push(0x60); // PUSH1
    code.push(0x00); // 0x00
    code.push(0x55); // SSTORE
    code.push(0x00); // STOP
    code
}

/// Used for authorization list signer in EIP-7702 tests.
fn example_authority_signer() -> utils::Signer {
    const AUTHORITY_SECRET_KEY: &str =
        "b71c71a67e1177ad4e901695e1b4b9ee17ae16c6668d313eac2f96dbcda3f291";

    let secret_key =
        libsecp256k1::SecretKey::parse_slice(&hex::decode(AUTHORITY_SECRET_KEY).unwrap()).unwrap();

    utils::Signer {
        nonce: 0,
        secret_key,
    }
}

fn eip7702_transaction(auth_chain_id: u64) -> Transaction7702 {
    let authority_sk_1 = example_authority_signer();
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let auth1 = sign_eip7702_authorization(
        auth_chain_id,
        contract_address,
        0,
        &authority_sk_1.secret_key,
    );

    Transaction7702 {
        chain_id: 1,
        nonce: INITIAL_NONCE.into(),
        gas_limit: U256::from(0x3d0900),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: contract_address,
        value: Wei::zero(),
        data: vec![],
        access_list: vec![],
        authorization_list: vec![auth1],
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
