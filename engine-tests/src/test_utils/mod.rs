use aurora_engine::parameters::ViewCallArgs;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::types::{NEP141Wei, PromiseResult};
use borsh::{BorshDeserialize, BorshSerialize};
use libsecp256k1::{self, Message, PublicKey, SecretKey};
use near_primitives::runtime::config_store::RuntimeConfigStore;
use near_primitives::version::PROTOCOL_VERSION;
use near_primitives_core::config::VMConfig;
use near_primitives_core::contract::ContractCode;
use near_primitives_core::profile::ProfileData;
use near_primitives_core::runtime::fees::RuntimeFeesConfig;
use near_vm_logic::types::ReturnData;
use near_vm_logic::{VMContext, VMOutcome, ViewConfig};
use near_vm_runner::{MockCompiledContractCache, VMError};
use rlp::RlpStream;

use crate::prelude::fungible_token::{FungibleToken, FungibleTokenMetadata};
use crate::prelude::parameters::{InitCallArgs, NewCallArgs, SubmitResult, TransactionStatus};
use crate::prelude::transactions::{
    eip_1559::{self, SignedTransaction1559, Transaction1559},
    eip_2930::{self, SignedTransaction2930, Transaction2930},
    legacy::{LegacyEthSignedTransaction, TransactionLegacy},
};
use crate::prelude::{sdk, Address, Wei, H256, U256};
use crate::test_utils::solidity::{ContractConstructor, DeployedContract};

// TODO(Copied from #84): Make sure that there is only one Signer after both PR are merged.

pub(crate) const ORIGIN: &str = "aurora";
pub(crate) const SUBMIT: &str = "submit";
pub(crate) const CALL: &str = "call";
pub(crate) const DEPLOY_ERC20: &str = "deploy_erc20_token";
pub(crate) const PAUSE_PRECOMPILES: &str = "pause_precompiles";
pub(crate) const PAUSED_PRECOMPILES: &str = "paused_precompiles";
pub(crate) const RESUME_PRECOMPILES: &str = "resume_precompiles";
pub(crate) const SET_OWNER: &str = "set_owner";

pub(crate) mod erc20;
pub(crate) mod exit_precompile;
pub(crate) mod mocked_external;
pub(crate) mod one_inch;
pub(crate) mod random;
pub(crate) mod rust;
pub(crate) mod self_destruct;
pub(crate) mod solidity;
pub(crate) mod standalone;
pub(crate) mod standard_precompiles;
pub(crate) mod uniswap;
pub(crate) mod weth;

pub struct Signer {
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
    pub ext: mocked_external::MockedExternalWithTrie,
    pub context: VMContext,
    pub wasm_config: VMConfig,
    pub fees_config: RuntimeFeesConfig,
    pub current_protocol_version: u32,
    pub previous_logs: Vec<String>,
    // Use the standalone in parallel if set. This allows checking both
    // implementations give the same results.
    pub standalone_runner: Option<standalone::StandaloneRunner>,
    // Empty by default. Can be set in tests if the transaction should be
    // executed as if it was a callback.
    pub promise_results: Vec<PromiseResult>,
}

/// Same as `AuroraRunner`, but consumes `self` on execution (thus preventing building on
/// the `ext` post-state with future calls to the contract.
#[derive(Clone)]
pub(crate) struct OneShotAuroraRunner<'a> {
    pub base: &'a AuroraRunner,
    pub ext: mocked_external::MockedExternalWithTrie,
    pub context: VMContext,
}

impl<'a> OneShotAuroraRunner<'a> {
    pub fn profiled_call(
        self,
        method_name: &str,
        caller_account_id: &str,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>, ExecutionProfile) {
        let (outcome, error) = self.call(method_name, caller_account_id, input);
        let profile = outcome
            .as_ref()
            .map(ExecutionProfile::new)
            .unwrap_or_default();
        (outcome, error, profile)
    }

    pub fn call(
        mut self,
        method_name: &str,
        caller_account_id: &str,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        AuroraRunner::update_context(
            &mut self.context,
            caller_account_id,
            caller_account_id,
            input,
        );

        match near_vm_runner::run(
            &self.base.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.base.wasm_config,
            &self.base.fees_config,
            &[],
            self.base.current_protocol_version,
            Some(&self.base.cache),
        ) {
            near_vm_runner::VMResult::Aborted(outcome, error) => (Some(outcome), Some(error)),
            near_vm_runner::VMResult::Ok(outcome) => (Some(outcome), None),
        }
    }
}

impl AuroraRunner {
    pub fn one_shot(&self) -> OneShotAuroraRunner {
        OneShotAuroraRunner {
            base: self,
            ext: self.ext.clone(),
            context: self.context.clone(),
        }
    }

    pub fn update_context(
        context: &mut VMContext,
        caller_account_id: &str,
        signer_account_id: &str,
        input: Vec<u8>,
    ) {
        context.block_index += 1;
        context.block_timestamp += 1_000_000_000;
        context.input = input;
        context.signer_account_id = as_account_id(signer_account_id);
        context.predecessor_account_id = as_account_id(caller_account_id);
    }

    pub fn call(
        &mut self,
        method_name: &str,
        caller_account_id: &str,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        self.call_with_signer(method_name, caller_account_id, caller_account_id, input)
    }

    pub fn call_with_signer(
        &mut self,
        method_name: &str,
        caller_account_id: &str,
        signer_account_id: &str,
        input: Vec<u8>,
    ) -> (Option<VMOutcome>, Option<VMError>) {
        Self::update_context(
            &mut self.context,
            caller_account_id,
            signer_account_id,
            input,
        );

        let vm_promise_results: Vec<_> = self
            .promise_results
            .iter()
            .map(|p| match p {
                PromiseResult::Failed => near_vm_logic::types::PromiseResult::Failed,
                PromiseResult::NotReady => near_vm_logic::types::PromiseResult::NotReady,
                PromiseResult::Successful(bytes) => {
                    near_vm_logic::types::PromiseResult::Successful(bytes.clone())
                }
            })
            .collect();
        let (maybe_outcome, maybe_error) = match near_vm_runner::run(
            &self.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.wasm_config,
            &self.fees_config,
            &vm_promise_results,
            self.current_protocol_version,
            Some(&self.cache),
        ) {
            near_vm_runner::VMResult::Aborted(outcome, error) => (Some(outcome), Some(error)),
            near_vm_runner::VMResult::Ok(outcome) => (Some(outcome), None),
        };
        if let Some(outcome) = &maybe_outcome {
            self.context.storage_usage = outcome.storage_usage;
            self.previous_logs = outcome.logs.clone();
        }

        if let Some(standalone_runner) = &mut self.standalone_runner {
            if maybe_error.is_none()
                && (method_name == SUBMIT
                    || method_name == CALL
                    || method_name == DEPLOY_ERC20
                    || method_name == PAUSE_PRECOMPILES
                    || method_name == RESUME_PRECOMPILES
                    || method_name == SET_OWNER)
            {
                standalone_runner
                    .submit_raw(method_name, &self.context, &self.promise_results)
                    .unwrap();
                self.validate_standalone();
            }
        }

        (maybe_outcome, maybe_error)
    }

    pub fn consume_json_snapshot(
        &mut self,
        snapshot: engine_standalone_storage::json_snapshot::types::JsonSnapshot,
    ) {
        let trie = &mut self.ext.underlying.fake_trie;
        for entry in snapshot.result.values {
            let key = aurora_engine_sdk::base64::decode(entry.key).unwrap();
            let value = aurora_engine_sdk::base64::decode(entry.value).unwrap();
            trie.insert(key, value);
        }
    }

    pub fn create_address(
        &mut self,
        address: Address,
        init_balance: crate::prelude::Wei,
        init_nonce: U256,
    ) {
        self.internal_create_address(address, init_balance, init_nonce, None)
    }

    pub fn create_address_with_code(
        &mut self,
        address: Address,
        init_balance: crate::prelude::Wei,
        init_nonce: U256,
        code: Vec<u8>,
    ) {
        self.internal_create_address(address, init_balance, init_nonce, Some(code))
    }

    fn internal_create_address(
        &mut self,
        address: Address,
        init_balance: crate::prelude::Wei,
        init_nonce: U256,
        code: Option<Vec<u8>>,
    ) {
        let trie = &mut self.ext.underlying.fake_trie;

        let balance_key = crate::prelude::storage::address_to_key(
            crate::prelude::storage::KeyPrefix::Balance,
            &address,
        );
        let balance_value = init_balance.to_bytes();

        let nonce_key = crate::prelude::storage::address_to_key(
            crate::prelude::storage::KeyPrefix::Nonce,
            &address,
        );
        let nonce_value = crate::prelude::u256_to_arr(&init_nonce);

        if let Some(code) = code.clone() {
            let code_key = crate::prelude::storage::address_to_key(
                crate::prelude::storage::KeyPrefix::Code,
                &address,
            );
            trie.insert(code_key.to_vec(), code);
        }

        let ft_key = crate::prelude::storage::bytes_to_key(
            crate::prelude::storage::KeyPrefix::EthConnector,
            &[crate::prelude::storage::EthConnectorStorageId::FungibleToken.into()],
        );
        let ft_value = {
            let mut current_ft: FungibleToken = trie
                .get(&ft_key)
                .map(|bytes| FungibleToken::try_from_slice(bytes).unwrap())
                .unwrap_or_default();
            current_ft.total_eth_supply_on_near =
                current_ft.total_eth_supply_on_near + NEP141Wei::new(init_balance.raw().as_u128());
            current_ft.total_eth_supply_on_aurora = current_ft.total_eth_supply_on_aurora
                + NEP141Wei::new(init_balance.raw().as_u128());
            current_ft
        };

        let aurora_balance_key = [
            ft_key.as_slice(),
            self.context.current_account_id.as_ref().as_bytes(),
        ]
        .concat();
        let aurora_balance_value = {
            let mut current_balance: u128 = trie
                .get(&aurora_balance_key)
                .map(|bytes| u128::try_from_slice(bytes).unwrap())
                .unwrap_or_default();
            current_balance += init_balance.raw().as_u128();
            current_balance
        };

        let proof_key = crate::prelude::storage::bytes_to_key(
            crate::prelude::storage::KeyPrefix::EthConnector,
            &[crate::prelude::storage::EthConnectorStorageId::UsedEvent.into()],
        );

        trie.insert(balance_key.to_vec(), balance_value.to_vec());
        if !init_nonce.is_zero() {
            trie.insert(nonce_key.to_vec(), nonce_value.to_vec());
        }
        trie.insert(ft_key, ft_value.try_to_vec().unwrap());
        trie.insert(proof_key, vec![0]);
        trie.insert(
            aurora_balance_key,
            aurora_balance_value.try_to_vec().unwrap(),
        );

        if let Some(standalone_runner) = &mut self.standalone_runner {
            standalone_runner.env.block_height = self.context.block_index;
            standalone_runner.mint_account(address, init_balance, init_nonce, code);
            self.validate_standalone();
        }

        self.context.block_index += 1;
    }

    pub fn submit_with_signer<F: FnOnce(U256) -> TransactionLegacy>(
        &mut self,
        signer: &mut Signer,
        make_tx: F,
    ) -> Result<SubmitResult, VMError> {
        self.submit_with_signer_profiled(signer, make_tx)
            .map(|(result, _)| result)
    }

    pub fn submit_with_signer_profiled<F: FnOnce(U256) -> TransactionLegacy>(
        &mut self,
        signer: &mut Signer,
        make_tx: F,
    ) -> Result<(SubmitResult, ExecutionProfile), VMError> {
        let nonce = signer.use_nonce();
        let tx = make_tx(nonce.into());
        self.submit_transaction_profiled(&signer.secret_key, tx)
    }

    pub fn submit_transaction(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
    ) -> Result<SubmitResult, VMError> {
        self.submit_transaction_profiled(account, transaction)
            .map(|(result, _)| result)
    }

    pub fn submit_transaction_profiled(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
    ) -> Result<(SubmitResult, ExecutionProfile), VMError> {
        let calling_account_id = "some-account.near";
        let signed_tx = sign_transaction(transaction, Some(self.chain_id), account);

        let (output, maybe_err) =
            self.call(SUBMIT, calling_account_id, rlp::encode(&signed_tx).to_vec());

        if let Some(err) = maybe_err {
            Err(err)
        } else {
            let output = output.unwrap();
            let profile = ExecutionProfile::new(&output);
            let submit_result =
                SubmitResult::try_from_slice(&output.return_data.as_value().unwrap()).unwrap();
            Ok((submit_result, profile))
        }
    }

    pub fn deploy_contract<F: FnOnce(&T) -> TransactionLegacy, T: Into<ContractConstructor>>(
        &mut self,
        account: &SecretKey,
        constructor_tx: F,
        contract_constructor: T,
    ) -> DeployedContract {
        let calling_account_id = "some-account.near";
        let tx = constructor_tx(&contract_constructor);
        let signed_tx = sign_transaction(tx, Some(self.chain_id), account);
        let (output, maybe_err) =
            self.call(SUBMIT, calling_account_id, rlp::encode(&signed_tx).to_vec());
        assert!(maybe_err.is_none());
        let submit_result =
            SubmitResult::try_from_slice(&output.unwrap().return_data.as_value().unwrap()).unwrap();
        let address = Address::try_from_slice(&unwrap_success(submit_result)).unwrap();
        let contract_constructor: ContractConstructor = contract_constructor.into();
        DeployedContract {
            abi: contract_constructor.abi,
            address,
        }
    }

    pub fn view_call(&self, args: ViewCallArgs) -> Result<TransactionStatus, VMError> {
        let input = args.try_to_vec().unwrap();
        let mut runner = self.one_shot();
        runner.context.view_config = Some(ViewConfig {
            max_gas_burnt: u64::MAX,
        });
        let (outcome, maybe_error) = runner.call("view", "viewer", input);
        Ok(
            TransactionStatus::try_from_slice(&Self::bytes_from_outcome(outcome, maybe_error)?)
                .unwrap(),
        )
    }

    pub fn profiled_view_call(
        &self,
        args: ViewCallArgs,
    ) -> (Result<TransactionStatus, VMError>, ExecutionProfile) {
        let input = args.try_to_vec().unwrap();
        let mut runner = self.one_shot();
        runner.context.view_config = Some(ViewConfig {
            max_gas_burnt: u64::MAX,
        });
        let (outcome, maybe_error, profile) = runner.profiled_call("view", "viewer", input);
        let status = Self::bytes_from_outcome(outcome, maybe_error)
            .map(|bytes| TransactionStatus::try_from_slice(&bytes).unwrap());

        (status, profile)
    }

    pub fn get_balance(&self, address: Address) -> Wei {
        Wei::new(self.u256_getter_method_call("get_balance", address))
    }

    pub fn get_nonce(&self, address: Address) -> U256 {
        self.u256_getter_method_call("get_nonce", address)
    }

    pub fn get_code(&self, address: Address) -> Vec<u8> {
        self.getter_method_call("get_code", address)
    }

    pub fn get_storage(&self, address: Address, key: H256) -> H256 {
        let input = aurora_engine::parameters::GetStorageAtArgs {
            address,
            key: key.0,
        };
        let (outcome, maybe_error) =
            self.one_shot()
                .call("get_storage_at", "getter", input.try_to_vec().unwrap());
        assert!(maybe_error.is_none());
        let output = outcome.unwrap().return_data.as_value().unwrap();
        let mut result = [0u8; 32];
        result.copy_from_slice(&output);
        H256(result)
    }

    fn u256_getter_method_call(&self, method_name: &str, address: Address) -> U256 {
        let bytes = self.getter_method_call(method_name, address);
        U256::from_big_endian(&bytes)
    }

    // Used in `get_balance` and `get_nonce`. This function exists to avoid code duplication
    // since the contract's `get_nonce` and `get_balance` have the same type signature.
    fn getter_method_call(&self, method_name: &str, address: Address) -> Vec<u8> {
        let (outcome, maybe_error) =
            self.one_shot()
                .call(method_name, "getter", address.as_bytes().to_vec());
        assert!(maybe_error.is_none());
        outcome.unwrap().return_data.as_value().unwrap()
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

    pub fn with_random_seed(mut self, random_seed: H256) -> Self {
        self.context.random_seed = random_seed.as_bytes().to_vec();
        self
    }

    fn validate_standalone(&self) {
        if let Some(standalone_runner) = &self.standalone_runner {
            let standalone_state = standalone_runner.get_current_state();
            // The number of keys in standalone_state may be larger because values are never deleted
            // (they are replaced with a Deleted identifier instead; this is important for replaying transactions).
            assert!(self.ext.underlying.fake_trie.len() <= standalone_state.iter().count());
            for (key, value) in standalone_state.iter() {
                let trie_value = self.ext.underlying.fake_trie.get(key).map(|v| v.as_slice());
                let standalone_value = value.value();
                if trie_value != standalone_value {
                    panic!(
                        "Standalone mismatch at {:?}.\nStandlaone: {:?}\nWasm      : {:?}",
                        key, standalone_value, trie_value
                    );
                }
            }
        }
    }
}

impl Default for AuroraRunner {
    fn default() -> Self {
        let evm_wasm_bytes = if cfg!(feature = "mainnet-test") {
            std::fs::read("../bin/aurora-mainnet-test.wasm").unwrap()
        } else if cfg!(feature = "testnet-test") {
            std::fs::read("../bin/aurora-testnet-test.wasm").unwrap()
        } else {
            panic!("AuroraRunner requires mainnet-test or testnet-test feature enabled.")
        };

        // Fetch config (mainly costs) for the latest protocol version.
        let runtime_config_store = RuntimeConfigStore::new(None);
        let runtime_config = runtime_config_store.get_config(PROTOCOL_VERSION);
        let wasm_config = runtime_config.wasm_config.clone();

        Self {
            aurora_account_id: ORIGIN.to_string(),
            chain_id: 1313161556, // NEAR localnet,
            code: ContractCode::new(evm_wasm_bytes, None),
            cache: Default::default(),
            ext: mocked_external::MockedExternalWithTrie::new(Default::default()),
            context: VMContext {
                current_account_id: as_account_id(ORIGIN),
                signer_account_id: as_account_id(ORIGIN),
                signer_account_pk: vec![],
                predecessor_account_id: as_account_id(ORIGIN),
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
                view_config: None,
                output_data_receivers: vec![],
            },
            wasm_config,
            fees_config: RuntimeFeesConfig::test(),
            current_protocol_version: u32::MAX,
            previous_logs: Default::default(),
            standalone_runner: None,
            promise_results: Vec::new(),
        }
    }
}

/// Wrapper around `ProfileData` to still include the wasm gas usage
/// (which was removed in https://github.com/near/nearcore/pull/4438).
#[derive(Debug, Default, Clone)]
pub(crate) struct ExecutionProfile {
    pub host_breakdown: ProfileData,
    wasm_gas: u64,
}

impl ExecutionProfile {
    pub fn new(outcome: &VMOutcome) -> Self {
        let wasm_gas =
            outcome.burnt_gas - outcome.profile.host_gas() - outcome.profile.action_gas();
        Self {
            host_breakdown: outcome.profile.clone(),
            wasm_gas,
        }
    }

    pub fn wasm_gas(&self) -> u64 {
        self.wasm_gas
    }

    pub fn all_gas(&self) -> u64 {
        self.wasm_gas + self.host_breakdown.host_gas() + self.host_breakdown.action_gas()
    }
}

pub(crate) fn deploy_evm() -> AuroraRunner {
    let mut runner = AuroraRunner::default();
    let args = NewCallArgs {
        chain_id: crate::prelude::u256_to_arr(&U256::from(runner.chain_id)),
        owner_id: str_to_account_id(runner.aurora_account_id.as_str()),
        bridge_prover_id: str_to_account_id("bridge_prover.near"),
        upgrade_delay_blocks: 1,
    };

    let account_id = runner.aurora_account_id.clone();
    let (_, maybe_error) = runner.call("new", &account_id, args.try_to_vec().unwrap());

    assert!(maybe_error.is_none());

    let args = InitCallArgs {
        prover_account: str_to_account_id("prover.near"),
        eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
        metadata: FungibleTokenMetadata::default(),
    };
    let (_, maybe_error) =
        runner.call("new_eth_connector", &account_id, args.try_to_vec().unwrap());

    assert!(maybe_error.is_none());

    let mut standalone_runner = standalone::StandaloneRunner::default();
    standalone_runner.init_evm();

    runner.standalone_runner = Some(standalone_runner);
    runner.validate_standalone();

    runner
}

pub(crate) fn transfer(to: Address, amount: Wei, nonce: U256) -> TransactionLegacy {
    TransactionLegacy {
        nonce,
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
        to: Some(to),
        value: amount,
        data: Vec::new(),
    }
}

pub(crate) fn create_deploy_transaction(contract_bytes: Vec<u8>, nonce: U256) -> TransactionLegacy {
    let len = contract_bytes.len();
    let len = u16::try_from(len).expect("Cannot deploy a contract with that many bytes!");
    // This bit of EVM byte code essentially says:
    // "If msg.value > 0 revert; otherwise return `len` amount of bytes that come after me
    // in the code." By prepending this to `contract_bytes` we create a valid EVM program which
    // returns `contract_bytes`, which is exactly what we want.
    let init_code = format!(
        "608060405234801561001057600080fd5b5061{}806100206000396000f300",
        hex::encode(len.to_be_bytes())
    );
    let data = hex::decode(init_code)
        .unwrap()
        .into_iter()
        .chain(contract_bytes.into_iter())
        .collect();

    TransactionLegacy {
        nonce,
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
        to: None,
        value: Wei::zero(),
        data,
    }
}

pub(crate) fn create_eth_transaction(
    to: Option<Address>,
    value: Wei,
    data: Vec<u8>,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    // nonce, gas_price and gas are not used by EVM contract currently
    let tx = TransactionLegacy {
        nonce: Default::default(),
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
        to,
        value,
        data,
    };
    sign_transaction(tx, chain_id, secret_key)
}

pub(crate) fn as_view_call(tx: TransactionLegacy, sender: Address) -> ViewCallArgs {
    ViewCallArgs {
        sender,
        address: tx.to.unwrap(),
        amount: tx.value.to_bytes(),
        input: tx.data,
    }
}

pub(crate) fn sign_transaction(
    tx: TransactionLegacy,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    let mut rlp_stream = RlpStream::new();
    tx.rlp_append_unsigned(&mut rlp_stream, chain_id);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let v: u64 = match chain_id {
        Some(chain_id) => u64::from(recovery_id.serialize()) + 2 * chain_id + 35,
        None => u64::from(recovery_id.serialize()) + 27,
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
    tx: Transaction2930,
    secret_key: &SecretKey,
) -> SignedTransaction2930 {
    let mut rlp_stream = RlpStream::new();
    rlp_stream.append(&eip_2930::TYPE_BYTE);
    tx.rlp_append_unsigned(&mut rlp_stream);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());

    SignedTransaction2930 {
        transaction: tx,
        parity: recovery_id.serialize(),
        r,
        s,
    }
}

pub(crate) fn sign_eip_1559_transaction(
    tx: Transaction1559,
    secret_key: &SecretKey,
) -> SignedTransaction1559 {
    let mut rlp_stream = RlpStream::new();
    rlp_stream.append(&eip_1559::TYPE_BYTE);
    tx.rlp_append_unsigned(&mut rlp_stream);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());

    SignedTransaction1559 {
        transaction: tx,
        parity: recovery_id.serialize(),
        r,
        s,
    }
}

pub(crate) fn address_from_secret_key(sk: &SecretKey) -> Address {
    let pk = PublicKey::from_secret_key(sk);
    let hash = sdk::keccak(&pk.serialize()[1..]);
    Address::try_from_slice(&hash[12..]).unwrap()
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
    expected_balance: Wei,
    expected_nonce: U256,
) {
    assert_eq!(runner.get_balance(address), expected_balance, "balance");
    assert_eq!(runner.get_nonce(address), expected_nonce, "nonce");
}

pub(crate) fn address_from_hex(address: &str) -> Address {
    let bytes = if let Some(address) = address.strip_prefix("0x") {
        hex::decode(address).unwrap()
    } else {
        hex::decode(address).unwrap()
    };

    Address::try_from_slice(&bytes).unwrap()
}

pub(crate) fn as_account_id(account_id: &str) -> near_primitives_core::types::AccountId {
    account_id.parse().unwrap()
}

pub(crate) fn str_to_account_id(account_id: &str) -> AccountId {
    use aurora_engine_types::str::FromStr;
    AccountId::from_str(account_id).unwrap()
}

pub fn unwrap_success(result: SubmitResult) -> Vec<u8> {
    match result.status {
        TransactionStatus::Succeed(ret) => ret,
        other => panic!("Unexpected status: {:?}", other),
    }
}

pub fn unwrap_success_slice(result: &SubmitResult) -> &[u8] {
    match &result.status {
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

pub fn assert_gas_bound(total_gas: u64, tgas_bound: u64) {
    // Add 1 to round up
    let tgas_used = (total_gas / 1_000_000_000_000) + 1;
    assert!(
        tgas_used == tgas_bound,
        "{} Tgas is not equal to {} Tgas",
        tgas_used,
        tgas_bound,
    );
}

/// Returns true if `abs(a - b) / max(a, b) <= x / 100`. The implementation is written differently than
/// this simpler formula to avoid floating point arithmetic.
pub fn within_x_percent(x: u64, a: u64, b: u64) -> bool {
    let (larger, smaller) = if a < b { (b, a) } else { (a, b) };

    (100 / x) * (larger - smaller) <= larger
}
