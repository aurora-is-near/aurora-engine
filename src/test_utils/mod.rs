use borsh::{BorshDeserialize, BorshSerialize};
use near_primitives_core::config::VMConfig;
use near_primitives_core::contract::ContractCode;
use near_primitives_core::profile::ProfileData;
use near_primitives_core::runtime::fees::RuntimeFeesConfig;
use near_vm_logic::mocks::mock_external::MockedExternal;
use near_vm_logic::types::ReturnData;
use near_vm_logic::{VMContext, VMOutcome};
use near_vm_runner::{MockCompiledContractCache, VMError};

use primitive_types::U256;
use rlp::RlpStream;
use secp256k1::{self, Message, PublicKey, SecretKey};

use crate::parameters::{NewCallArgs, SubmitResult};
use crate::prelude::Address;
use crate::storage;
use crate::transaction::{EthSignedTransaction, EthTransaction};
use crate::types;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "release.wasm"
}

pub const SUBMIT: &str = "submit";

pub mod solidity;

pub struct AuroraRunner {
    pub aurora_account_id: String,
    pub chain_id: u64,
    pub code: ContractCode,
    pub cache: MockCompiledContractCache,
    pub ext: MockedExternal,
    pub context: VMContext,
    pub wasm_config: VMConfig,
    pub fees_config: RuntimeFeesConfig,
    pub current_protocol_version: u32,
    pub profile: ProfileData,
}

/// Same as `AuroraRunner`, but consumes `self` on execution (thus preventing building on
/// the `ext` post-state with future calls to the contract.
#[derive(Clone)]
pub struct OneShotAuroraRunner<'a> {
    pub base: &'a AuroraRunner,
    pub ext: MockedExternal,
    pub context: VMContext,
}

impl<'a> OneShotAuroraRunner<'a> {
    pub fn call(
        mut self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        AuroraRunner::update_context(&mut self.context, caller_account_id, input);

        near_vm_runner::run(
            &self.base.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.base.wasm_config,
            &self.base.fees_config,
            &[],
            self.base.current_protocol_version,
            Some(&self.base.cache),
            &self.base.profile,
        )
    }
}

impl AuroraRunner {
    pub fn one_shot(&self) -> OneShotAuroraRunner {
        OneShotAuroraRunner {
            base: &self,
            ext: self.ext.clone(),
            context: self.context.clone(),
        }
    }

    pub fn update_context(context: &mut VMContext, caller_account_id: String, input: Vec<u8>) {
        context.block_index += 1;
        context.block_timestamp += 100;
        context.input = input;
        context.signer_account_id = caller_account_id.clone();
        context.predecessor_account_id = caller_account_id;
    }

    pub fn call(
        &mut self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        Self::update_context(&mut self.context, caller_account_id, input);

        near_vm_runner::run(
            &self.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.wasm_config,
            &self.fees_config,
            &[],
            self.current_protocol_version,
            Some(&self.cache),
            &self.profile,
        )
    }

    pub fn create_address(&mut self, address: Address, init_balance: U256, init_nonce: U256) {
        let trie = &mut self.ext.fake_trie;

        let balance_key = storage::address_to_key(storage::KeyPrefix::Balance, &address);
        let balance_value = types::u256_to_arr(&init_balance);

        let nonce_key = storage::address_to_key(storage::KeyPrefix::Nonce, &address);
        let nonce_value = types::u256_to_arr(&init_nonce);

        trie.insert(balance_key.to_vec(), balance_value.to_vec());
        trie.insert(nonce_key.to_vec(), nonce_value.to_vec());
    }

    pub fn get_balance(&self, address: Address) -> U256 {
        self.getter_method_call("get_balance", address)
    }

    pub fn get_nonce(&self, address: Address) -> U256 {
        self.getter_method_call("get_nonce", address)
    }

    // Used in `get_balance` and `get_nonce`. This function exists to avoid code duplication
    // since the contract's `get_nonce` and `get_balance` have the same type signature.
    fn getter_method_call(&self, method_name: &str, address: Address) -> U256 {
        let mut context = self.context.clone();
        Self::update_context(
            &mut context,
            "GETTER".to_string(),
            address.as_bytes().to_vec(),
        );
        let (outcome, maybe_error) = near_vm_runner::run(
            &self.code,
            method_name,
            &mut self.ext.clone(),
            context,
            &self.wasm_config,
            &self.fees_config,
            &[],
            self.current_protocol_version,
            Some(&self.cache),
            &self.profile,
        );
        assert!(maybe_error.is_none());
        let bytes = outcome.unwrap().return_data.as_value().unwrap();
        U256::from_big_endian(&bytes)
    }
}

impl Default for AuroraRunner {
    fn default() -> Self {
        let aurora_account_id = "aurora".to_string();
        Self {
            aurora_account_id: aurora_account_id.clone(),
            chain_id: 1313161556, // NEAR betanet
            code: ContractCode::new(EVM_WASM_BYTES.to_vec(), None),
            cache: Default::default(),
            ext: Default::default(),
            context: VMContext {
                current_account_id: aurora_account_id.clone(),
                signer_account_id: aurora_account_id.clone(),
                signer_account_pk: vec![],
                predecessor_account_id: aurora_account_id,
                input: vec![],
                block_index: 0,
                block_timestamp: 0,
                epoch_height: 0,
                account_balance: 10u128.pow(25),
                account_locked_balance: 0,
                storage_usage: 100,
                attached_deposit: 0,
                prepaid_gas: 10u64.pow(18),
                random_seed: vec![],
                is_view: false,
                output_data_receivers: vec![],
            },
            wasm_config: Default::default(),
            fees_config: Default::default(),
            current_protocol_version: u32::MAX,
            profile: Default::default(),
        }
    }
}

pub fn deploy_evm() -> AuroraRunner {
    let mut runner = AuroraRunner::default();
    let args = NewCallArgs {
        chain_id: types::u256_to_arr(&U256::from(runner.chain_id)),
        owner_id: runner.aurora_account_id.clone(),
        bridge_prover_id: "prover.near".to_string(),
        upgrade_delay_blocks: 1,
    };

    let (_, maybe_error) = runner.call(
        "new",
        runner.aurora_account_id.clone(),
        args.try_to_vec().unwrap(),
    );

    assert!(maybe_error.is_none());

    runner
}

pub fn create_eth_transaction(
    to: Option<Address>,
    value: U256,
    data: Vec<u8>,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> EthSignedTransaction {
    // nonce, gas_price and gas are not used by EVM contract currently
    let tx = EthTransaction {
        nonce: Default::default(),
        gas_price: Default::default(),
        gas: Default::default(),
        to,
        value,
        data,
    };
    sign_transaction(tx, chain_id, secret_key)
}

pub fn sign_transaction(
    tx: EthTransaction,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> EthSignedTransaction {
    let mut rlp_stream = RlpStream::new();
    tx.rlp_append_unsigned(&mut rlp_stream, chain_id);
    let message_hash = types::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = secp256k1::sign(&message, secret_key);
    let v: u64 = match chain_id {
        Some(chain_id) => (recovery_id.serialize() as u64) + 2 * chain_id + 35,
        None => (recovery_id.serialize() as u64) + 27,
    };
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());
    EthSignedTransaction {
        transaction: tx,
        v,
        r,
        s,
    }
}

pub fn address_from_secret_key(sk: &SecretKey) -> Address {
    let pk = PublicKey::from_secret_key(sk);
    let hash = types::keccak(&pk.serialize()[1..]);
    Address::from_slice(&hash[12..])
}

pub fn parse_eth_gas(output: &VMOutcome) -> u64 {
    let submit_result_bytes = match &output.return_data {
        ReturnData::Value(bytes) => bytes.as_slice(),
        ReturnData::None | ReturnData::ReceiptIndex(_) => panic!("Unexpected ReturnData"),
    };
    let submit_result = SubmitResult::try_from_slice(submit_result_bytes).unwrap();
    submit_result.gas_used
}

pub fn validate_address_balance_and_nonce(
    runner: &AuroraRunner,
    address: Address,
    expected_balance: U256,
    expected_nonce: U256,
) {
    assert_eq!(runner.get_balance(address), expected_balance);
    assert_eq!(runner.get_nonce(address), expected_nonce);
}
