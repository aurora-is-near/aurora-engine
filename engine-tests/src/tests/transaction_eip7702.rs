//! Integration tests for EIP-7702 (type 0x04) transactions in the Aurora Engine.
//!
//! Covered scenarios:
//!   * RLP encoding/decoding round-trip of a signed EIP-7702 tx.
//!   * Happy-path delegation install and EVM execution in the delegated context.
//!   * Delegated-authority EOA sending its own EIP-1559 tx (EIP-3607 exception).
//!   * Sponsored tx: signer invokes delegated code in the authority's context.
//!   * Delegation revocation by an external signer and by the authority itself.
//!   * `auth.chain_id` mismatch - the auth entry is marked invalid while the
//!     outer tx still executes and charges gas.
//!   * Early-exit rejection paths (wrong `tx.chain_id`, `tx.nonce`, or insufficient
//!     balance): prove that `ecrecover` over the auth list is NOT invoked.
//!
//! NEAR-gas asserts are floor-rounded to 0.1 TGas (100 GGas) via
//! [`round_near_gas`] so sub-percent cost drift doesn't flake the suite
//! while any meaningful cost-model regression still fails loudly.

use aurora_engine::parameters::SubmitResult;
use aurora_engine_transactions::eip_7702;
use aurora_engine_transactions::eip_7702::{
    AuthorizationTuple, SignedTransaction7702, Transaction7702,
};
use aurora_engine_types::H160;
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::types::Address;
use std::convert::TryFrom;
use std::iter;

use crate::prelude::Wei;
use crate::prelude::transactions::EthTransactionKind;
use crate::prelude::transactions::eip_1559::{self, SignedTransaction1559, Transaction1559};
use crate::prelude::{H256, U256};
use crate::utils;
use crate::utils::{sign_eip_1559_transaction, sign_eip7702_authorization};

const SECRET_KEY: &str = "45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8";
const INITIAL_NONCE: u64 = 1;
const INITIAL_BALANCE: Wei = Wei::new_u64(0x0de0b6b3a7640000);

const CONTRACT_ADDRESS: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const CONTRACT_NONCE: u64 = INITIAL_NONCE;
const CONTRACT_BALANCE: Wei = INITIAL_BALANCE;

/// Contract that stores `EXTCODESIZE(authority)` into slot 0 - used by
/// length-cap tests as the tx's `to` target.
const AUTHORITY_EXTCODESIZE_PROBE_HEX: &str =
    "73a52a8a2229e3c512d6ed27b6e6e7d39958ca9fb33B60005500";

/// Default EVM `gas_limit` for single-auth EIP-7702 tests.
const DEFAULT_EVM_GAS_LIMIT: u64 = 0x3d_0900;
const RELAY_ACCOUNT: &str = "relay.aurora";

/// Round-trips a single-auth EIP-7702 tx through RLP and verifies that the
/// recovered sender matches the signer.
#[test]
fn test_eip_7702_tx_encoding_decoding() {
    let secret_key = example_signer().secret_key;
    let transaction = eip7702_single_auth_tx(1, 0);

    let signed_tx = utils::sign_eip_7702_transaction(transaction, &secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

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

/// Happy-path: signer submits an EIP-7702 tx that installs delegation on
/// the authority. Verifies EVM gas, nonces, balances and that the authority
/// now holds the `ef0100 || <target>` delegation designator code.
#[test]
fn test_eip_7702_success() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let authority_sk = example_authority_signer();
    let authority_address = utils::address_from_secret_key(&authority_sk.secret_key);

    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = sample_code_for_contract_eip7702(authority_address);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    let transaction = eip7702_single_auth_tx(runner.chain_id, 0);
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let actual_ggas_used = outcome.used_gas.as_gigagas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    assert_eq!(result.gas_used, 68206);
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());
    assert_eq!(runner.get_balance(contract_address), CONTRACT_BALANCE);
    assert_eq!(runner.get_nonce(contract_address), CONTRACT_NONCE.into());
    assert_eq!(runner.get_code(contract_address), contract_code);
    assert_eq!(
        runner.get_storage(contract_address, H256::zero()),
        H256::from(H160::from_low_u64_be(23))
    );

    assert_eq!(runner.get_nonce(authority_address), 1.into());
    assert_eq!(
        hex::encode(runner.get_code(authority_address)),
        "ef0100cccccccccccccccccccccccccccccccccccccccc"
    );

    assert_eq!(actual_ggas_used, 4595);
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
    let transaction = eip7702_single_auth_tx(runner.chain_id, 0);
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
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

    let signed_tx_2 = sign_eip_1559_transaction(eip1559_tx, &authority_sk.secret_key);
    let tx_bytes_2 = encode_signed_1559(&signed_tx_2);

    let outcome_2 = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes_2)
        .unwrap();
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
        gas_limit: DEFAULT_EVM_GAS_LIMIT.into(),
        to: Some(authority_address),
        value: Wei::new_u64(0),
        data: Vec::new(),
        access_list: Vec::new(),
    };

    let signed_tx_3 = sign_eip_1559_transaction(eip1559_tx2, &signer.secret_key);
    let tx_bytes_3 = encode_signed_1559(&signed_tx_3);

    let outcome_3 = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes_3)
        .unwrap();
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

    let revoke_tx = eip7702_tx_with_auth_list(
        runner.chain_id,
        3.into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        vec![auth_revoke],
    );

    let signed_tx_4 = utils::sign_eip_7702_transaction(revoke_tx, &signer.secret_key);
    let tx_bytes_4 = encode_signed_7702(&signed_tx_4);

    let outcome_4 = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes_4)
        .unwrap();
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
        contract_code,
    );

    // ══════════════════════════════════════════════════════════════
    // Step 1: signer installs delegation on authority
    // ══════════════════════════════════════════════════════════════
    let transaction = eip7702_single_auth_tx(runner.chain_id, 0);
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);
    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    // Transfer to authority_address
    let tx_transfer = Transaction1559 {
        chain_id: runner.chain_id,
        nonce: U256::from(2),
        gas_limit: DEFAULT_EVM_GAS_LIMIT.into(),
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: Some(authority_address),
        value: Wei::new_u64(10),
        data: vec![],
        access_list: vec![],
    };
    let signed_tx_transfer = sign_eip_1559_transaction(tx_transfer, &signer.secret_key);
    let tx_bytes = encode_signed_1559(&signed_tx_transfer);
    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
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

    let revoke_tx = eip7702_tx_with_auth_list(
        runner.chain_id,
        1.into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        vec![auth_revoke],
    );

    let signed_tx_2 = utils::sign_eip_7702_transaction(revoke_tx, &authority_sk.secret_key);
    let tx_bytes_2 = encode_signed_7702(&signed_tx_2);

    let outcome_2 = runner
        .call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes_2)
        .unwrap();
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

/// `auth.chain_id` is set to a value that matches neither 0 nor the tx
/// `chain_id`: the authorization entry must be marked invalid by the engine
/// (authority nonce and code unchanged), while the outer tx itself still
/// executes, advances the signer's nonce and bills EVM gas.
#[test]
fn test_eip_7702_wrong_auth_chain_id() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = hex::decode(AUTHORITY_EXTCODESIZE_PROBE_HEX).unwrap();

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code.clone(),
    );

    // Set wrong `chain_id = 10`
    let transaction = eip7702_single_auth_tx(runner.chain_id, 10);
    let signed_tx = utils::sign_eip_7702_transaction(transaction, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let actual_ggas_used = outcome.used_gas.as_gigagas();
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
    assert_eq!(actual_ggas_used, 3745);
}

/// Multi-auth happy path: 3 distinct authorities each delegate to `CONTRACT_ADDRESS`
/// in one tx — all three codes set, all three nonces incremented.
#[test]
fn test_eip_7702_multiple_distinct_authorities_succeed() {
    const AUTHORITY_SECRET_KEY_1: &str =
        "1111111111111111111111111111111111111111111111111111111111111111";
    const AUTHORITY_SECRET_KEY_2: &str =
        "2222222222222222222222222222222222222222222222222222222222222222";

    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let auth1 = example_authority_signer();
    let auth2 = example_authority_signer_with_key(AUTHORITY_SECRET_KEY_1);
    let auth3 = example_authority_signer_with_key(AUTHORITY_SECRET_KEY_2);
    let auth1_addr = utils::address_from_secret_key(&auth1.secret_key);
    let auth2_addr = utils::address_from_secret_key(&auth2.secret_key);
    let auth3_addr = utils::address_from_secret_key(&auth3.secret_key);

    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = sample_code_for_contract_eip7702(auth1_addr);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let auth_list = vec![
        sign_eip7702_authorization(0, contract_address, 0, &auth1.secret_key),
        sign_eip7702_authorization(0, contract_address, 0, &auth2.secret_key),
        sign_eip7702_authorization(0, contract_address, 0, &auth3.secret_key),
    ];
    let tx = eip7702_tx_with_auth_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        auth_list,
    );
    let signed_tx = utils::sign_eip_7702_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let actual_ggas_used = outcome.used_gas.as_gigagas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    let expected_code = format!("ef0100{}", hex::encode(contract_address.as_bytes()));
    assert_eq!(hex::encode(runner.get_code(auth1_addr)), expected_code);
    assert_eq!(hex::encode(runner.get_code(auth2_addr)), expected_code);
    assert_eq!(hex::encode(runner.get_code(auth3_addr)), expected_code);
    assert_eq!(runner.get_nonce(auth1_addr), 1.into());
    assert_eq!(runner.get_nonce(auth2_addr), 1.into());
    assert_eq!(runner.get_nonce(auth3_addr), 1.into());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());

    assert_eq!(result.gas_used, 118_206);
    assert_eq!(actual_ggas_used, 6421);
}

/// Same authority twice with same nonce: first auth applies and increments nonce,
/// second fails (nonce-match) - only first target survives in authority.code.
#[test]
fn test_eip_7702_duplicate_authority_same_nonce_only_first_applies() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let authority = example_authority_signer();
    let authority_addr = utils::address_from_secret_key(&authority.secret_key);

    let target_b = utils::address_from_hex(CONTRACT_ADDRESS);
    let target_c = utils::address_from_hex("0xdddddddddddddddddddddddddddddddddddddddd");
    let contract_code = sample_code_for_contract_eip7702(authority_addr);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        target_b,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let auth_list = vec![
        sign_eip7702_authorization(0, target_b, 0, &authority.secret_key),
        sign_eip7702_authorization(0, target_c, 0, &authority.secret_key),
    ];
    let tx = eip7702_tx_with_auth_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        auth_list,
    );
    let signed_tx = utils::sign_eip_7702_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let actual_ggas_used = outcome.used_gas.as_gigagas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    let expected_first = format!("ef0100{}", hex::encode(target_b.as_bytes()));
    assert_eq!(hex::encode(runner.get_code(authority_addr)), expected_first);
    assert_eq!(runner.get_nonce(authority_addr), 1.into());

    assert_eq!(result.gas_used, 93_206);
    assert_eq!(actual_ggas_used, 4941);
}

/// Authority pre-funded with non-delegated contract code: check skips the auth,
/// authority bytecode and nonce stay unchanged while tx itself executes and is billed.
#[test]
fn test_eip_7702_authority_with_contract_code_is_skipped() {
    let mut runner = utils::deploy_runner();
    let signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let authority = example_authority_signer();
    let authority_addr = utils::address_from_secret_key(&authority.secret_key);
    let original_code = hex::decode("6001600101").unwrap();

    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let contract_code = sample_code_for_contract_eip7702(authority_addr);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        authority_addr,
        INITIAL_BALANCE,
        0.into(),
        original_code.clone(),
    );
    runner.create_address_with_code(
        contract_address,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        contract_code,
    );

    let auth_list = vec![sign_eip7702_authorization(
        0,
        contract_address,
        0,
        &authority.secret_key,
    )];
    let tx = eip7702_tx_with_auth_list(
        runner.chain_id,
        INITIAL_NONCE.into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        auth_list,
    );
    let signed_tx = utils::sign_eip_7702_transaction(tx, &signer.secret_key);
    let tx_bytes = encode_signed_7702(&signed_tx);

    let outcome = runner.call(utils::SUBMIT, RELAY_ACCOUNT, tx_bytes).unwrap();
    let actual_ggas_used = outcome.used_gas.as_gigagas();
    let result = SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();
    assert!(result.status.is_ok());

    assert_eq!(runner.get_code(authority_addr), original_code);
    assert_eq!(runner.get_nonce(authority_addr), 0.into());
    assert_eq!(runner.get_nonce(signer_address), (signer.nonce + 1).into());

    assert_eq!(result.gas_used, 68_206);
    assert_eq!(actual_ggas_used, 4126);
}

/// Re-delegation: authority already points to `target_B`; a second tx swaps the
/// designator to `target_C`.
#[test]
fn test_eip_7702_redelegate_existing_delegation() {
    let mut runner = utils::deploy_runner();
    let mut signer = example_signer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let authority = example_authority_signer();
    let authority_addr = utils::address_from_secret_key(&authority.secret_key);

    let target_b = utils::address_from_hex(CONTRACT_ADDRESS);
    let target_c = utils::address_from_hex("0xdddddddddddddddddddddddddddddddddddddddd");
    let target_b_code = sample_code_for_contract_eip7702(authority_addr);

    runner.create_address(signer_address, INITIAL_BALANCE, signer.nonce.into());
    runner.create_address_with_code(
        target_b,
        CONTRACT_BALANCE,
        CONTRACT_NONCE.into(),
        target_b_code,
    );

    // Tx #1: install delegation A -> target_b.
    let auth_b = sign_eip7702_authorization(0, target_b, 0, &authority.secret_key);
    let tx1 = eip7702_tx_with_auth_list(
        runner.chain_id,
        signer.use_nonce().into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        vec![auth_b],
    );
    let signed_tx1 = utils::sign_eip_7702_transaction(tx1, &signer.secret_key);
    let outcome1 = runner
        .call(
            utils::SUBMIT,
            RELAY_ACCOUNT,
            encode_signed_7702(&signed_tx1),
        )
        .unwrap();
    let result1 = SubmitResult::try_from_slice(&outcome1.return_data.as_value().unwrap()).unwrap();
    assert!(result1.status.is_ok());
    assert_eq!(
        hex::encode(runner.get_code(authority_addr)),
        format!("ef0100{}", hex::encode(target_b.as_bytes()))
    );
    assert_eq!(runner.get_nonce(authority_addr), 1.into());

    // Tx #2: re-delegate A -> target_c (auth.nonce = 1 because of tx#1).
    let auth_c = sign_eip7702_authorization(0, target_c, 1, &authority.secret_key);
    let tx2 = eip7702_tx_with_auth_list(
        runner.chain_id,
        signer.use_nonce().into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        vec![auth_c],
    );
    let signed_tx2 = utils::sign_eip_7702_transaction(tx2, &signer.secret_key);
    let outcome2 = runner
        .call(
            utils::SUBMIT,
            RELAY_ACCOUNT,
            encode_signed_7702(&signed_tx2),
        )
        .unwrap();
    let actual_ggas_used = outcome2.used_gas.as_gigagas();
    let result2 = SubmitResult::try_from_slice(&outcome2.return_data.as_value().unwrap()).unwrap();
    assert!(result2.status.is_ok());

    assert_eq!(
        hex::encode(runner.get_code(authority_addr)),
        format!("ef0100{}", hex::encode(target_c.as_bytes()))
    );
    assert_eq!(runner.get_nonce(authority_addr), 2.into());

    assert_eq!(result2.gas_used, 38_645);
    assert_eq!(actual_ggas_used, 4589);
}

/// Signer for the *transaction sender* role — the EOA that submits an
/// EIP-7702 tx to the engine. Distinct key / nonce from the authority.
fn example_signer() -> utils::Signer {
    let secret_key =
        libsecp256k1::SecretKey::parse_slice(&hex::decode(SECRET_KEY).unwrap()).unwrap();

    utils::Signer {
        nonce: INITIAL_NONCE,
        secret_key,
    }
}

/// Signer for the *authority* role — the EOA whose code is being set via the
/// `authorization_list`. Its private key signs each `AuthorizationTuple`.
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

fn example_authority_signer_with_key(auth_secret_key: &str) -> utils::Signer {
    let secret_key =
        libsecp256k1::SecretKey::parse_slice(&hex::decode(auth_secret_key).unwrap()).unwrap();
    utils::Signer {
        nonce: 0,
        secret_key,
    }
}

/// Build an EIP-7702 tx with a pre-built authorization list.
fn eip7702_tx_with_auth_list(
    chain_id: u64,
    nonce: U256,
    gas_limit: U256,
    auth_list: Vec<AuthorizationTuple>,
) -> Transaction7702 {
    Transaction7702 {
        chain_id,
        nonce,
        gas_limit,
        max_fee_per_gas: U256::from(0x07d0),
        max_priority_fee_per_gas: U256::from(0x0a),
        to: utils::address_from_hex(CONTRACT_ADDRESS),
        value: Wei::zero(),
        data: vec![],
        access_list: vec![],
        authorization_list: auth_list,
    }
}

/// Convenience: EIP-7702 tx with a single authorization (`authority ⇒ CONTRACT_ADDRESS`).
/// `chain_id` is the *tx-level* `chain_id`; `auth_chain_id` goes into the
/// authorization tuple (use 0 to accept any chain, or a specific id to pin).
fn eip7702_single_auth_tx(chain_id: u64, auth_chain_id: u64) -> Transaction7702 {
    eip7702_tx_with_auth_list(
        chain_id,
        INITIAL_NONCE.into(),
        DEFAULT_EVM_GAS_LIMIT.into(),
        make_auth_list(1, auth_chain_id),
    )
}

/// Build `n` authorization tuples signed by the same authority with nonces `0..n`.
/// For length-cap tests the list *content* is secondary — only its size
/// matters (plus valid RLP).
#[allow(clippy::as_conversions)]
fn make_auth_list(n: usize, auth_chain_id: u64) -> Vec<AuthorizationTuple> {
    let authority_sk = example_authority_signer();
    let contract_address = utils::address_from_hex(CONTRACT_ADDRESS);
    let n = u64::try_from(n).unwrap();
    (0..n)
        .map(|i| {
            sign_eip7702_authorization(auth_chain_id, contract_address, i, &authority_sk.secret_key)
        })
        .collect()
}

/// Serialise a signed EIP-7702 tx to the byte-stream the contract expects.
fn encode_signed_7702(signed: &SignedTransaction7702) -> Vec<u8> {
    iter::once(eip_7702::TYPE_BYTE)
        .chain(rlp::encode(signed))
        .collect()
}

/// Serialise a signed EIP-1559 tx to the byte-stream the contract expects.
fn encode_signed_1559(signed: &SignedTransaction1559) -> Vec<u8> {
    iter::once(eip_1559::TYPE_BYTE)
        .chain(rlp::encode(signed))
        .collect()
}

/// Contract code for the delegation target:
/// `PUSH20 <authority> | EXTCODESIZE | PUSH1 0 | SSTORE | STOP`.
/// Used to witness that the authority's code is the `0xEF0100 || target`
/// designator (size 23) after a successful EIP-7702 delegation.
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
