use borsh::{BorshDeserialize, BorshSerialize};
use evm::Context;
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
use std::borrow::Cow;

use crate::fungible_token::{FungibleToken, FungibleTokenMetadata};
use crate::parameters::{InitCallArgs, NewCallArgs, SubmitResult, TransactionStatus, ViewCallArgs};
use crate::prelude::Address;
use crate::storage;
use crate::test_utils::solidity::{ContractConstructor, DeployedContract};
use crate::transaction::{
    access_list::{self, AccessListEthSignedTransaction, AccessListEthTransaction},
    LegacyEthSignedTransaction, LegacyEthTransaction,
};
use crate::types;
use crate::types::AccountId;

// TODO(Copied from #84): Make sure that there is only one Signer after both PR are merged.

pub fn origin() -> AccountId {
    "aurora".to_string()
}

pub fn erc20_admin_account() -> AccountId {
    [crate::connector::ERC20_ADMIN_PREFIX, &origin()].concat()
}

pub(crate) const SUBMIT: &str = "submit";

pub(crate) mod erc20;
pub(crate) mod exit_precompile;
pub(crate) mod self_destruct;
pub(crate) mod solidity;
pub(crate) mod standard_precompiles;

pub(crate) struct Signer {
    pub nonce: u64,
    pub secret_key: SecretKey,
}

impl Signer {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            nonce: 0,
            secret_key,
        }
    }

    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let sk = SecretKey::random(&mut rng);
        Self::new(sk)
    }

    pub fn use_nonce(&mut self) -> u64 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }
}

pub(crate) struct AuroraRunner {
    pub aurora_account_id: String,
    pub chain_id: u64,
    pub code: ContractCode,
    pub cache: MockCompiledContractCache,
    pub ext: MockedExternal,
    pub context: VMContext,
    pub wasm_config: VMConfig,
    pub fees_config: RuntimeFeesConfig,
    pub current_protocol_version: u32,
    pub previous_logs: Vec<String>,
}

/// Same as `AuroraRunner`, but consumes `self` on execution (thus preventing building on
/// the `ext` post-state with future calls to the contract.
#[derive(Clone)]
pub(crate) struct OneShotAuroraRunner<'a> {
    pub base: &'a AuroraRunner,
    pub ext: MockedExternal,
    pub context: VMContext,
}

impl<'a> OneShotAuroraRunner<'a> {
    pub fn call(
        self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        self.call_with_optional_profile(method_name, caller_account_id, input, None)
    }

    pub fn profiled_call(
        self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>, ProfileData) {
        let profile = Default::default();
        let (outcome, error) =
            self.call_with_optional_profile(method_name, caller_account_id, input, Some(&profile));
        (outcome, error, profile)
    }

    fn call_with_optional_profile(
        mut self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
        maybe_profile: Option<&ProfileData>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        AuroraRunner::update_context(
            &mut self.context,
            caller_account_id.clone(),
            caller_account_id,
            input,
        );

        let profile = maybe_profile.map(Cow::Borrowed).unwrap_or_default();
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
            profile.as_ref(),
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

    pub fn update_context(
        context: &mut VMContext,
        caller_account_id: String,
        signer_account_id: String,
        input: Vec<u8>,
    ) {
        context.block_index += 1;
        context.block_timestamp += 100;
        context.input = input;
        context.signer_account_id = signer_account_id;
        context.predecessor_account_id = caller_account_id;
    }

    pub fn call(
        &mut self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        self.call_with_signer(
            method_name,
            caller_account_id.clone(),
            caller_account_id,
            input,
            None,
        )
    }

    // Might be useful for optimizing performance in the future
    #[allow(dead_code)]
    pub fn profiled_call(
        &mut self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>, ProfileData) {
        let profile = Default::default();
        let (outcome, error) = self.call_with_signer(
            method_name,
            caller_account_id.clone(),
            caller_account_id,
            input,
            Some(&profile),
        );
        (outcome, error, profile)
    }

    pub fn call_with_signer(
        &mut self,
        method_name: &str,
        caller_account_id: String,
        signer_account_id: String,
        input: Vec<u8>,
        maybe_profile: Option<&ProfileData>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        Self::update_context(
            &mut self.context,
            caller_account_id,
            signer_account_id,
            input,
        );

        let profile = maybe_profile.map(Cow::Borrowed).unwrap_or_default();
        let (maybe_outcome, maybe_error) = near_vm_runner::run(
            &self.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.wasm_config,
            &self.fees_config,
            &[],
            self.current_protocol_version,
            Some(&self.cache),
            profile.as_ref(),
        );
        if let Some(outcome) = &maybe_outcome {
            self.context.storage_usage = outcome.storage_usage;
            self.previous_logs = outcome.logs.clone();
        }
        (maybe_outcome, maybe_error)
    }

    pub fn create_address(&mut self, address: Address, init_balance: types::Wei, init_nonce: U256) {
        let trie = &mut self.ext.fake_trie;

        let balance_key = storage::address_to_key(storage::KeyPrefix::Balance, &address);
        let balance_value = init_balance.to_bytes();

        let nonce_key = storage::address_to_key(storage::KeyPrefix::Nonce, &address);
        let nonce_value = types::u256_to_arr(&init_nonce);

        let ft_key = storage::bytes_to_key(
            storage::KeyPrefix::EthConnector,
            &[storage::EthConnectorStorageId::FungibleToken as u8],
        );
        let ft_value = {
            let mut current_ft: FungibleToken = trie
                .get(&ft_key)
                .map(|bytes| FungibleToken::try_from_slice(&bytes).unwrap())
                .unwrap_or_default();
            current_ft.total_eth_supply_on_aurora += init_balance.raw().as_u128();
            current_ft
        };

        trie.insert(balance_key.to_vec(), balance_value.to_vec());
        trie.insert(nonce_key.to_vec(), nonce_value.to_vec());
        trie.insert(ft_key, ft_value.try_to_vec().unwrap());
    }

    pub fn submit_with_signer<F: FnOnce(U256) -> LegacyEthTransaction>(
        &mut self,
        signer: &mut Signer,
        make_tx: F,
    ) -> Result<SubmitResult, VMError> {
        let nonce = signer.use_nonce();
        let tx = make_tx(nonce.into());
        self.submit_transaction(&signer.secret_key, tx)
    }

    pub fn submit_transaction(
        &mut self,
        account: &SecretKey,
        transaction: LegacyEthTransaction,
    ) -> Result<SubmitResult, VMError> {
        let calling_account_id = "some-account.near".to_string();
        let signed_tx = sign_transaction(transaction, Some(self.chain_id), account);

        let (output, maybe_err) =
            self.call(SUBMIT, calling_account_id, rlp::encode(&signed_tx).to_vec());

        if let Some(err) = maybe_err {
            Err(err)
        } else {
            let submit_result =
                SubmitResult::try_from_slice(&output.unwrap().return_data.as_value().unwrap())
                    .unwrap();
            Ok(submit_result)
        }
    }

    pub fn deploy_contract<F: FnOnce(&T) -> LegacyEthTransaction, T: Into<ContractConstructor>>(
        &mut self,
        account: &SecretKey,
        constructor_tx: F,
        contract_constructor: T,
    ) -> DeployedContract {
        let calling_account_id = "some-account.near".to_string();
        let tx = constructor_tx(&contract_constructor);
        let signed_tx = sign_transaction(tx, Some(self.chain_id), account);
        let (output, maybe_err) =
            self.call(SUBMIT, calling_account_id, rlp::encode(&signed_tx).to_vec());
        assert!(maybe_err.is_none());
        let submit_result =
            SubmitResult::try_from_slice(&output.unwrap().return_data.as_value().unwrap()).unwrap();
        let address = Address::from_slice(&unwrap_success(submit_result));
        let contract_constructor: ContractConstructor = contract_constructor.into();
        DeployedContract {
            abi: contract_constructor.abi,
            address,
        }
    }

    pub fn view_call(&self, args: ViewCallArgs) -> Result<TransactionStatus, VMError> {
        let input = args.try_to_vec().unwrap();
        let (outcome, maybe_error) = self.one_shot().call("view", "VIEWER".to_string(), input);
        Ok(
            TransactionStatus::try_from_slice(&Self::bytes_from_outcome(outcome, maybe_error)?)
                .unwrap(),
        )
    }

    pub fn profiled_view_call(
        &self,
        args: ViewCallArgs,
    ) -> (Result<TransactionStatus, VMError>, ProfileData) {
        let input = args.try_to_vec().unwrap();
        let (outcome, maybe_error, profile) =
            self.one_shot()
                .profiled_call("view", "VIEWER".to_string(), input);
        let status = Self::bytes_from_outcome(outcome, maybe_error)
            .map(|bytes| TransactionStatus::try_from_slice(&bytes).unwrap());

        (status, profile)
    }

    pub fn get_balance(&self, address: Address) -> types::Wei {
        types::Wei::new(self.getter_method_call("get_balance", address))
    }

    pub fn get_nonce(&self, address: Address) -> U256 {
        self.getter_method_call("get_nonce", address)
    }

    // Used in `get_balance` and `get_nonce`. This function exists to avoid code duplication
    // since the contract's `get_nonce` and `get_balance` have the same type signature.
    fn getter_method_call(&self, method_name: &str, address: Address) -> U256 {
        let (outcome, maybe_error) = self.one_shot().call(
            method_name,
            "GETTER".to_string(),
            address.as_bytes().to_vec(),
        );
        assert!(maybe_error.is_none());
        let bytes = outcome.unwrap().return_data.as_value().unwrap();
        U256::from_big_endian(&bytes)
    }

    fn bytes_from_outcome(
        maybe_outcome: Option<VMOutcome>,
        maybe_error: Option<VMError>,
    ) -> Result<Vec<u8>, VMError> {
        if let Some(error) = maybe_error {
            Err(error)
        } else {
            let bytes = maybe_outcome.unwrap().return_data.as_value().unwrap();
            Ok(bytes)
        }
    }
}

impl Default for AuroraRunner {
    fn default() -> Self {
        let aurora_account_id = "aurora".to_string();
        let evm_wasm_bytes = if cfg!(feature = "mainnet-test") {
            std::fs::read("mainnet-test.wasm").unwrap()
        } else if cfg!(feature = "testnet-test") {
            std::fs::read("testnet-test.wasm").unwrap()
        } else {
            std::fs::read("betanet-test.wasm").unwrap()
        };

        Self {
            aurora_account_id: aurora_account_id.clone(),
            chain_id: 1313161556, // NEAR betanet
            code: ContractCode::new(evm_wasm_bytes, None),
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
            previous_logs: Default::default(),
        }
    }
}

pub(crate) fn deploy_evm() -> AuroraRunner {
    let mut runner = AuroraRunner::default();
    let args = NewCallArgs {
        chain_id: types::u256_to_arr(&U256::from(runner.chain_id)),
        owner_id: runner.aurora_account_id.clone(),
        bridge_prover_id: "bridge_prover.near".to_string(),
        upgrade_delay_blocks: 1,
    };

    let (_, maybe_error) = runner.call(
        "new",
        runner.aurora_account_id.clone(),
        args.try_to_vec().unwrap(),
    );

    assert!(maybe_error.is_none());

    let args = InitCallArgs {
        prover_account: "prover.near".to_string(),
        eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
        metadata: FungibleTokenMetadata::default(),
    };
    let (_, maybe_error) = runner.call(
        "new_eth_connector",
        runner.aurora_account_id.clone(),
        args.try_to_vec().unwrap(),
    );

    assert!(maybe_error.is_none());

    runner
}

pub(crate) fn transfer(to: Address, amount: types::Wei, nonce: U256) -> LegacyEthTransaction {
    LegacyEthTransaction {
        nonce,
        gas_price: Default::default(),
        gas: u64::MAX.into(),
        to: Some(to),
        value: amount,
        data: Vec::new(),
    }
}

pub(crate) fn create_eth_transaction(
    to: Option<Address>,
    value: types::Wei,
    data: Vec<u8>,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    // nonce, gas_price and gas are not used by EVM contract currently
    let tx = LegacyEthTransaction {
        nonce: Default::default(),
        gas_price: Default::default(),
        gas: u64::MAX.into(),
        to,
        value,
        data,
    };
    sign_transaction(tx, chain_id, secret_key)
}

pub(crate) fn as_view_call(tx: LegacyEthTransaction, sender: Address) -> ViewCallArgs {
    ViewCallArgs {
        sender: sender.0,
        address: tx.to.unwrap().0,
        amount: tx.value.to_bytes(),
        input: tx.data,
    }
}

pub(crate) fn sign_transaction(
    tx: LegacyEthTransaction,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
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
    LegacyEthSignedTransaction {
        transaction: tx,
        v,
        r,
        s,
    }
}

pub(crate) fn sign_access_list_transaction(
    tx: AccessListEthTransaction,
    secret_key: &SecretKey,
) -> AccessListEthSignedTransaction {
    let mut rlp_stream = RlpStream::new();
    rlp_stream.append(&access_list::TYPE_BYTE);
    tx.rlp_append_unsigned(&mut rlp_stream);
    let message_hash = types::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = secp256k1::sign(&message, secret_key);
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());

    AccessListEthSignedTransaction {
        transaction_data: tx,
        parity: recovery_id.serialize(),
        r,
        s,
    }
}

pub(crate) fn address_from_secret_key(sk: &SecretKey) -> Address {
    let pk = PublicKey::from_secret_key(sk);
    let hash = types::keccak(&pk.serialize()[1..]);
    Address::from_slice(&hash[12..])
}

pub(crate) fn parse_eth_gas(output: &VMOutcome) -> u64 {
    let submit_result_bytes = match &output.return_data {
        ReturnData::Value(bytes) => bytes.as_slice(),
        ReturnData::None | ReturnData::ReceiptIndex(_) => panic!("Unexpected ReturnData"),
    };
    let submit_result = SubmitResult::try_from_slice(submit_result_bytes).unwrap();
    submit_result.gas_used
}

pub(crate) fn validate_address_balance_and_nonce(
    runner: &AuroraRunner,
    address: Address,
    expected_balance: types::Wei,
    expected_nonce: U256,
) {
    assert_eq!(runner.get_balance(address), expected_balance, "balance");
    assert_eq!(runner.get_nonce(address), expected_nonce, "nonce");
}

pub fn new_context() -> Context {
    Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: Default::default(),
    }
}

pub(crate) fn address_from_hex(address: &str) -> Address {
    let bytes = if address.starts_with("0x") {
        hex::decode(&address[2..]).unwrap()
    } else {
        hex::decode(address).unwrap()
    };

    Address::from_slice(&bytes)
}

pub fn unwrap_success(result: SubmitResult) -> Vec<u8> {
    match result.status {
        TransactionStatus::Succeed(ret) => ret,
        other => panic!("Unexpected status: {:?}", other),
    }
}

pub fn unwrap_revert(result: SubmitResult) -> Vec<u8> {
    match result.status {
        TransactionStatus::Revert(ret) => ret,
        other => panic!("Unexpected status: {:?}", other),
    }
}

pub fn panic_on_fail(status: TransactionStatus) {
    match status {
        TransactionStatus::Succeed(_) => (),
        TransactionStatus::Revert(message) => panic!("{}", String::from_utf8_lossy(&message)),
        other => panic!("{}", String::from_utf8_lossy(other.as_ref())),
    }
}
