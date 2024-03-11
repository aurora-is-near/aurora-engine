use aurora_engine::engine::{EngineError, EngineErrorKind, GasPaymentError};
use aurora_engine::parameters::{SubmitArgs, ViewCallArgs};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh::BorshDeserialize;
#[cfg(not(feature = "ext-connector"))]
use aurora_engine_types::parameters::connector::FungibleTokenMetadata;
#[cfg(feature = "ext-connector")]
use aurora_engine_types::parameters::connector::{
    SetEthConnectorContractAccountArgs, WithdrawSerializeType,
};
use aurora_engine_types::parameters::engine::{NewCallArgs, NewCallArgsV4};
use aurora_engine_types::parameters::silo::FixedGasArgs;
use aurora_engine_types::types::{EthGas, PromiseResult};
use libsecp256k1::{self, Message, PublicKey, SecretKey};
use near_parameters::vm::VMKind;
use near_parameters::{RuntimeConfigStore, RuntimeFeesConfig};
use near_primitives::version::PROTOCOL_VERSION;
use near_primitives_core::config::ViewConfig;
use near_vm_runner::logic::errors::FunctionCallError;
use near_vm_runner::logic::mocks::mock_external::MockedExternal;
use near_vm_runner::logic::types::ReturnData;
use near_vm_runner::logic::{Config, HostError, VMContext, VMOutcome};
use near_vm_runner::{ContractCode, MockCompiledContractCache, ProfileDataV3};
use rlp::RlpStream;
use std::borrow::Cow;

#[cfg(not(feature = "ext-connector"))]
use crate::prelude::parameters::InitCallArgs;
use crate::prelude::parameters::{StartHashchainArgs, SubmitResult, TransactionStatus};
use crate::prelude::transactions::{
    eip_1559::{self, SignedTransaction1559, Transaction1559},
    eip_2930::{self, SignedTransaction2930, Transaction2930},
    legacy::{LegacyEthSignedTransaction, TransactionLegacy},
};
use crate::prelude::{sdk, Address, Wei, H256, U256};
use crate::utils::solidity::{ContractConstructor, DeployedContract};

pub const DEFAULT_AURORA_ACCOUNT_ID: &str = "aurora";
pub const SUBMIT: &str = "submit";
pub const SUBMIT_WITH_ARGS: &str = "submit_with_args";
pub const PAUSE_PRECOMPILES: &str = "pause_precompiles";
pub const PAUSED_PRECOMPILES: &str = "paused_precompiles";
pub const RESUME_PRECOMPILES: &str = "resume_precompiles";
pub const DEFAULT_CHAIN_ID: u64 = 1_313_161_556; // NEAR localnet

const CALLER_ACCOUNT_ID: &str = "some-account.near";

pub mod mocked_external;
pub mod one_inch;
pub mod rust;
pub mod solidity;
pub mod standalone;
pub mod workspace;

pub struct Signer {
    pub nonce: u64,
    pub secret_key: SecretKey,
}

impl Signer {
    pub const fn new(secret_key: SecretKey) -> Self {
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

pub struct AuroraRunner {
    pub aurora_account_id: String,
    pub chain_id: u64,
    pub code: ContractCode,
    pub cache: MockCompiledContractCache,
    pub ext: mocked_external::MockedExternalWithTrie,
    pub context: VMContext,
    pub wasm_config: Config,
    pub fees_config: RuntimeFeesConfig,
    pub current_protocol_version: u32,
    pub previous_logs: Vec<String>,
    // Use the standalone in parallel if set. This allows checking both
    // implementations give the same results.
    pub standalone_runner: Option<standalone::StandaloneRunner>,
    // Empty by default. Can be set in tests if the transaction should be
    // executed as if it was a callback.
    pub promise_results: Vec<PromiseResult>,
    // None by default. Can be set if the transaction requires randomness
    // from the Near runtime.
    // Note: this only sets the random value for the block, the random
    // value available in the runtime is derived from this value and
    // another hash that depends on the transaction itself.
    pub block_random_value: Option<H256>,
}

/// Same as `AuroraRunner`, but consumes `self` on execution (thus preventing building on
/// the `ext` post-state with future calls to the contract.
#[derive(Clone)]
pub struct OneShotAuroraRunner<'a> {
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
    ) -> Result<(VMOutcome, ExecutionProfile), EngineError> {
        self.call(method_name, caller_account_id, input)
            .map(|outcome| {
                let profile = ExecutionProfile::new(&outcome);
                (outcome, profile)
            })
    }

    pub fn call(
        mut self,
        method_name: &str,
        caller_account_id: &str,
        input: Vec<u8>,
    ) -> Result<VMOutcome, EngineError> {
        AuroraRunner::update_context(
            &mut self.context,
            caller_account_id,
            caller_account_id,
            input,
        );

        let outcome = near_vm_runner::run(
            &self.base.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.base.wasm_config,
            &self.base.fees_config,
            &[],
            Some(&self.base.cache),
        )
        .unwrap();

        if let Some(aborted) = outcome.aborted.as_ref() {
            Err(into_engine_error(outcome.used_gas, aborted))
        } else {
            Ok(outcome)
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
        context.block_height += 1;
        context.block_timestamp += 1_000_000_000;
        context.input = input;
        context.signer_account_id = signer_account_id.parse().unwrap();
        context.predecessor_account_id = caller_account_id.parse().unwrap();
    }

    pub fn call(
        &mut self,
        method_name: &str,
        caller_account_id: &str,
        input: Vec<u8>,
    ) -> Result<VMOutcome, EngineError> {
        self.call_with_signer(method_name, caller_account_id, caller_account_id, input)
    }

    pub fn call_with_signer(
        &mut self,
        method_name: &str,
        caller_account_id: &str,
        signer_account_id: &str,
        input: Vec<u8>,
    ) -> Result<VMOutcome, EngineError> {
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
                PromiseResult::Failed => near_vm_runner::logic::types::PromiseResult::Failed,
                PromiseResult::NotReady => near_vm_runner::logic::types::PromiseResult::NotReady,
                PromiseResult::Successful(bytes) => {
                    near_vm_runner::logic::types::PromiseResult::Successful(bytes.clone())
                }
            })
            .collect();
        let outcome = near_vm_runner::run(
            &self.code,
            method_name,
            &mut self.ext,
            self.context.clone(),
            &self.wasm_config,
            &self.fees_config,
            &vm_promise_results,
            Some(&self.cache),
        )
        .unwrap();

        println!("{:?}", outcome.logs);

        if let Some(error) = outcome.aborted.as_ref() {
            return Err(into_engine_error(outcome.used_gas, error));
        }

        self.context.storage_usage = outcome.storage_usage;
        self.previous_logs = outcome.logs.clone();

        if let Some(standalone_runner) = &mut self.standalone_runner {
            standalone_runner.submit_raw(
                method_name,
                &self.context,
                &self.promise_results,
                self.block_random_value,
            )?;
            self.validate_standalone();
        }

        Ok(outcome)
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

    pub fn create_address(&mut self, address: Address, init_balance: Wei, init_nonce: U256) {
        self.internal_create_address(address, init_balance, init_nonce, None);
    }

    pub fn create_address_with_code(
        &mut self,
        address: Address,
        init_balance: Wei,
        init_nonce: U256,
        code: Vec<u8>,
    ) {
        self.internal_create_address(address, init_balance, init_nonce, Some(code));
    }

    fn internal_create_address(
        &mut self,
        address: Address,
        init_balance: Wei,
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

        trie.insert(balance_key.to_vec(), balance_value.to_vec());
        if !init_nonce.is_zero() {
            trie.insert(nonce_key.to_vec(), nonce_value.to_vec());
        }

        #[cfg(not(feature = "ext-connector"))]
        {
            use aurora_engine::contract_methods::connector::fungible_token::FungibleToken;
            let ft_key = crate::prelude::storage::bytes_to_key(
                crate::prelude::storage::KeyPrefix::EthConnector,
                &[crate::prelude::storage::EthConnectorStorageId::FungibleToken.into()],
            );
            let ft_value = {
                let mut current_ft: FungibleToken = trie
                    .get(&ft_key)
                    .map(|bytes| FungibleToken::try_from_slice(bytes).unwrap())
                    .unwrap_or_default();
                current_ft.total_eth_supply_on_near = current_ft.total_eth_supply_on_near
                    + aurora_engine_types::types::NEP141Wei::new(init_balance.raw().as_u128());
                current_ft.total_eth_supply_on_aurora = current_ft.total_eth_supply_on_aurora
                    + aurora_engine_types::types::NEP141Wei::new(init_balance.raw().as_u128());
                current_ft
            };

            let aurora_balance_key = [
                ft_key.as_slice(),
                self.context.current_account_id.as_bytes(),
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

            trie.insert(ft_key, borsh::to_vec(&ft_value).unwrap());
            trie.insert(proof_key, vec![0]);
            trie.insert(
                aurora_balance_key,
                borsh::to_vec(&aurora_balance_value).unwrap(),
            );
        }

        if let Some(standalone_runner) = &mut self.standalone_runner {
            standalone_runner.env.block_height = self.context.block_height;
            standalone_runner.mint_account(address, init_balance, init_nonce, code);
            self.validate_standalone();
        }

        self.context.block_height += 1;
    }

    pub fn submit_with_signer<F: FnOnce(U256) -> TransactionLegacy>(
        &mut self,
        signer: &mut Signer,
        make_tx: F,
    ) -> Result<SubmitResult, EngineError> {
        self.submit_with_signer_profiled(signer, make_tx)
            .map(|(result, _)| result)
    }

    pub fn submit_with_signer_profiled<F: FnOnce(U256) -> TransactionLegacy>(
        &mut self,
        signer: &mut Signer,
        make_tx: F,
    ) -> Result<(SubmitResult, ExecutionProfile), EngineError> {
        let nonce = signer.use_nonce();
        let tx = make_tx(nonce.into());
        self.submit_transaction_profiled(&signer.secret_key, tx)
    }

    pub fn submit_transaction(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
    ) -> Result<SubmitResult, EngineError> {
        self.submit_transaction_profiled(account, transaction)
            .map(|(result, _)| result)
    }

    pub fn submit_transaction_profiled(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
    ) -> Result<(SubmitResult, ExecutionProfile), EngineError> {
        let signed_tx = sign_transaction(transaction, Some(self.chain_id), account);
        self.call(SUBMIT, CALLER_ACCOUNT_ID, rlp::encode(&signed_tx).to_vec())
            .map(Self::profile_outcome)
    }

    pub fn submit_transaction_with_args(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
        max_gas_price: u128,
        gas_token_address: Option<Address>,
    ) -> Result<SubmitResult, EngineError> {
        self.submit_transaction_with_args_profiled(
            account,
            transaction,
            max_gas_price,
            gas_token_address,
        )
        .map(|(result, _)| result)
    }

    pub fn submit_transaction_with_args_profiled(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
        max_gas_price: u128,
        gas_token_address: Option<Address>,
    ) -> Result<(SubmitResult, ExecutionProfile), EngineError> {
        let signed_tx = sign_transaction(transaction, Some(self.chain_id), account);
        let args = SubmitArgs {
            tx_data: rlp::encode(&signed_tx).to_vec(),
            max_gas_price: Some(max_gas_price),
            gas_token_address,
        };

        self.call(
            SUBMIT_WITH_ARGS,
            CALLER_ACCOUNT_ID,
            borsh::to_vec(&args).unwrap(),
        )
        .map(Self::profile_outcome)
    }

    fn profile_outcome(outcome: VMOutcome) -> (SubmitResult, ExecutionProfile) {
        let profile = ExecutionProfile::new(&outcome);
        let submit_result =
            SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

        (submit_result, profile)
    }

    pub fn deploy_contract<F: FnOnce(&T) -> TransactionLegacy, T: Into<ContractConstructor>>(
        &mut self,
        account: &SecretKey,
        constructor_tx: F,
        contract_constructor: T,
    ) -> DeployedContract {
        let tx = constructor_tx(&contract_constructor);
        let signed_tx = sign_transaction(tx, Some(self.chain_id), account);
        let outcome = self.call(SUBMIT, CALLER_ACCOUNT_ID, rlp::encode(&signed_tx).to_vec());
        let submit_result =
            SubmitResult::try_from_slice(&outcome.unwrap().return_data.as_value().unwrap())
                .unwrap();
        let address = Address::try_from_slice(&unwrap_success(submit_result)).unwrap();
        let contract_constructor: ContractConstructor = contract_constructor.into();
        DeployedContract {
            abi: contract_constructor.abi,
            address,
        }
    }

    pub fn view_call(&self, args: &ViewCallArgs) -> Result<TransactionStatus, EngineError> {
        let input = borsh::to_vec(&args).unwrap();
        let mut runner = self.one_shot();
        runner.context.view_config = Some(ViewConfig {
            max_gas_burnt: u64::MAX,
        });

        runner.call("view", "viewer", input).map(|outcome| {
            TransactionStatus::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap()
        })
    }

    pub fn profiled_view_call(
        &self,
        args: &ViewCallArgs,
    ) -> Result<(TransactionStatus, ExecutionProfile), EngineError> {
        let input = borsh::to_vec(&args).unwrap();
        let mut runner = self.one_shot();

        runner.context.view_config = Some(ViewConfig {
            max_gas_burnt: u64::MAX,
        });

        let (outcome, profile) = runner.profiled_call("view", "viewer", input)?;
        let status =
            TransactionStatus::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

        Ok((status, profile))
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

    pub fn get_fixed_gas(&mut self) -> Option<EthGas> {
        let outcome = self
            .one_shot()
            .call("get_fixed_gas", "getter", vec![])
            .unwrap();
        let val = outcome.return_data.as_value()?;
        FixedGasArgs::try_from_slice(&val).unwrap().fixed_gas
    }

    pub fn get_storage(&self, address: Address, key: H256) -> H256 {
        let input = aurora_engine::parameters::GetStorageAtArgs {
            address,
            key: key.0,
        };
        let outcome = self
            .one_shot()
            .call("get_storage_at", "getter", borsh::to_vec(&input).unwrap())
            .unwrap();
        let output = outcome.return_data.as_value().unwrap();
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
        let outcome = self
            .one_shot()
            .call(method_name, "getter", address.as_bytes().to_vec())
            .unwrap();
        outcome.return_data.as_value().unwrap()
    }

    pub const fn with_block_random_value(mut self, random_seed: H256) -> Self {
        self.block_random_value = Some(random_seed);
        self
    }

    fn validate_standalone(&self) {
        if let Some(standalone_runner) = &self.standalone_runner {
            let standalone_state = standalone_runner.get_current_state();
            // The number of keys in standalone_state may be larger because values are never deleted
            // (they are replaced with a Deleted identifier instead; this is important for replaying transactions).
            let fake_trie_len = self.ext.underlying.fake_trie.len();
            let stand_alone_len = standalone_state.iter().count();

            if fake_trie_len > stand_alone_len {
                let fake_keys = self
                    .ext
                    .underlying
                    .fake_trie
                    .keys()
                    .map(Clone::clone)
                    .collect::<std::collections::HashSet<_>>();
                let standalone_keys = standalone_state
                    .iter()
                    .map(|x| x.0.clone())
                    .collect::<std::collections::HashSet<_>>();
                let diff = fake_keys.difference(&standalone_keys).collect::<Vec<_>>();

                panic!("The standalone state has fewer amount of keys: {fake_trie_len} vs {stand_alone_len}\nDiff: {diff:?}");
            }

            for (key, value) in standalone_state {
                let trie_value = self.ext.underlying.fake_trie.get(key).map(Vec::as_slice);
                let standalone_value = value.value();
                assert_eq!(
                    trie_value, standalone_value,
                    "Standalone mismatch at {key:?}.\nStandalone: {standalone_value:?}\nWasm: {trie_value:?}",
                );
            }
        }
    }

    pub fn get_engine_code() -> Vec<u8> {
        let path = if cfg!(feature = "mainnet-test") {
            if cfg!(feature = "ext-connector") {
                "../bin/aurora-mainnet-silo-test.wasm"
            } else {
                "../bin/aurora-mainnet-test.wasm"
            }
        } else if cfg!(feature = "testnet-test") {
            if cfg!(feature = "ext-connector") {
                "../bin/aurora-testnet-silo-test.wasm"
            } else {
                "../bin/aurora-testnet-test.wasm"
            }
        } else {
            panic!("AuroraRunner requires mainnet-test or testnet-test feature enabled.")
        };

        std::fs::read(path).unwrap()
    }

    pub fn get_engine_v331_code() -> Vec<u8> {
        let path = if cfg!(feature = "ext-connector") {
            "src/tests/res/aurora_silo_v3.3.1.wasm"
        } else {
            "src/tests/res/aurora_v3.3.1.wasm"
        };
        std::fs::read(path).unwrap()
    }

    pub const fn get_default_chain_id() -> u64 {
        DEFAULT_CHAIN_ID
    }
}

impl Default for AuroraRunner {
    fn default() -> Self {
        let evm_wasm_bytes = Self::get_engine_code();
        // Fetch config (mainly costs) for the latest protocol version.
        let runtime_config_store = RuntimeConfigStore::test();
        let runtime_config = runtime_config_store.get_config(PROTOCOL_VERSION);
        let mut wasm_config = runtime_config.wasm_config.clone();

        if cfg!(not(target_arch = "x86_64")) {
            wasm_config.vm_kind = VMKind::Wasmtime;
        } else {
            wasm_config.vm_kind = VMKind::Wasmer2;
        }

        let origin_account_id: near_primitives::types::AccountId =
            DEFAULT_AURORA_ACCOUNT_ID.parse().unwrap();

        Self {
            aurora_account_id: DEFAULT_AURORA_ACCOUNT_ID.to_string(),
            chain_id: DEFAULT_CHAIN_ID,
            code: ContractCode::new(evm_wasm_bytes, None),
            cache: MockCompiledContractCache::default(),
            ext: mocked_external::MockedExternalWithTrie::new(MockedExternal::default()),
            context: VMContext {
                current_account_id: origin_account_id.clone(),
                signer_account_id: origin_account_id.clone(),
                signer_account_pk: vec![],
                predecessor_account_id: origin_account_id,
                input: vec![],
                block_height: 0,
                block_timestamp: 0,
                epoch_height: 0,
                account_balance: 10u128.pow(25),
                account_locked_balance: 0,
                storage_usage: 100,
                attached_deposit: 0,
                prepaid_gas: 10u64.pow(18),
                random_seed: vec![],
                output_data_receivers: vec![],
                view_config: None,
            },
            wasm_config,
            fees_config: RuntimeFeesConfig::test(),
            current_protocol_version: u32::MAX,
            previous_logs: Vec::new(),
            standalone_runner: Some(standalone::StandaloneRunner::default()),
            promise_results: Vec::new(),
            block_random_value: None,
        }
    }
}

/// Wrapper around `ProfileData` to still include the wasm gas usage
/// (which was removed in `https://github.com/near/nearcore/pull/4438`).
#[derive(Debug, Default, Clone)]
pub struct ExecutionProfile {
    pub host_breakdown: ProfileDataV3,
    total_gas_cost: u64,
}

impl ExecutionProfile {
    pub fn new(outcome: &VMOutcome) -> Self {
        Self {
            host_breakdown: outcome.profile.clone(),
            total_gas_cost: outcome.burnt_gas,
        }
    }

    pub fn wasm_gas(&self) -> u64 {
        self.host_breakdown.get_wasm_cost()
    }

    pub const fn all_gas(&self) -> u64 {
        self.total_gas_cost
    }
}

pub fn deploy_runner() -> AuroraRunner {
    let mut runner = AuroraRunner::default();
    let aurora_account_id = str_to_account_id(runner.aurora_account_id.as_str());
    let args = NewCallArgs::V4(NewCallArgsV4 {
        chain_id: crate::prelude::u256_to_arr(&U256::from(runner.chain_id)),
        owner_id: aurora_account_id.clone(),
        upgrade_delay_blocks: 1,
        key_manager: aurora_account_id,
        initial_hashchain: Some([0u8; 32]),
    });

    let account_id = runner.aurora_account_id.clone();
    let result = runner.call("new", &account_id, borsh::to_vec(&args).unwrap());

    assert!(result.is_ok());

    #[cfg(not(feature = "ext-connector"))]
    let result = {
        let args = InitCallArgs {
            prover_account: str_to_account_id("prover.near"),
            eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
            metadata: FungibleTokenMetadata::default(),
        };
        runner.call(
            "new_eth_connector",
            &account_id,
            borsh::to_vec(&args).unwrap(),
        )
    };

    #[cfg(feature = "ext-connector")]
    let result = {
        let args = SetEthConnectorContractAccountArgs {
            account: AccountId::new("aurora_eth_connector.root").unwrap(),
            withdraw_serialize_type: WithdrawSerializeType::Borsh,
        };

        runner.call(
            "set_eth_connector_contract_account",
            &account_id,
            borsh::to_vec(&args).unwrap(),
        )
    };

    assert!(result.is_ok());

    runner
}

pub fn init_hashchain(
    runner: &mut AuroraRunner,
    caller_account_id: &str,
    block_height: Option<u64>,
) {
    // Set up hashchain:
    //   1. Pause contract (hashchain can only be started if contract is paused first)
    //   2. Start hashchain

    let result: Result<VMOutcome, EngineError> =
        runner.call("pause_contract", caller_account_id, Vec::new());
    assert!(result.is_ok());

    if let Some(h) = block_height {
        runner.context.block_height = h;
    }

    let args = StartHashchainArgs {
        block_height: runner.context.block_height,
        block_hashchain: [0u8; 32],
    };
    let result = runner.call(
        "start_hashchain",
        caller_account_id,
        borsh::to_vec(&args).unwrap(),
    );
    assert!(result.is_ok());
}

pub fn transfer(to: Address, amount: Wei, nonce: U256) -> TransactionLegacy {
    transfer_with_price(to, amount, nonce, U256::zero())
}

pub fn transfer_with_price(
    to: Address,
    amount: Wei,
    nonce: U256,
    gas_price: U256,
) -> TransactionLegacy {
    TransactionLegacy {
        nonce,
        gas_price,
        gas_limit: u64::MAX.into(),
        to: Some(to),
        value: amount,
        data: Vec::new(),
    }
}

pub fn create_deploy_transaction(contract_bytes: Vec<u8>, nonce: U256) -> TransactionLegacy {
    create_deploy_transaction_with_price(contract_bytes, nonce, U256::zero())
}

pub fn create_deploy_transaction_with_price(
    contract_bytes: Vec<u8>,
    nonce: U256,
    gas_price: U256,
) -> TransactionLegacy {
    let len = u16::try_from(contract_bytes.len())
        .unwrap_or_else(|_| panic!("Cannot deploy a contract with that many bytes!"));
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
        .chain(contract_bytes)
        .collect();

    TransactionLegacy {
        nonce,
        gas_price,
        gas_limit: u64::MAX.into(),
        to: None,
        value: Wei::zero(),
        data,
    }
}

pub fn create_eth_transaction(
    to: Option<Address>,
    value: Wei,
    data: Vec<u8>,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    // nonce, gas_price and gas are not used by EVM contract currently
    let tx = TransactionLegacy {
        nonce: U256::default(),
        gas_price: U256::default(),
        gas_limit: u64::MAX.into(),
        to,
        value,
        data,
    };
    sign_transaction(tx, chain_id, secret_key)
}

pub fn as_view_call(tx: TransactionLegacy, sender: Address) -> ViewCallArgs {
    ViewCallArgs {
        sender,
        address: tx.to.unwrap(),
        amount: tx.value.to_bytes(),
        input: tx.data,
    }
}

pub fn sign_transaction(
    tx: TransactionLegacy,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    let mut rlp_stream = RlpStream::new();
    tx.rlp_append_unsigned(&mut rlp_stream, chain_id);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let v: u64 = chain_id.map_or_else(
        || u64::from(recovery_id.serialize()) + 27,
        |chain_id| u64::from(recovery_id.serialize()) + 2 * chain_id + 35,
    );
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());
    LegacyEthSignedTransaction {
        transaction: tx,
        v,
        r,
        s,
    }
}

pub fn sign_access_list_transaction(
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

pub fn sign_eip_1559_transaction(
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

pub fn address_from_secret_key(sk: &SecretKey) -> Address {
    let pk = PublicKey::from_secret_key(sk);
    let hash = sdk::keccak(&pk.serialize()[1..]);
    Address::try_from_slice(&hash[12..]).unwrap()
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
    expected_balance: Wei,
    expected_nonce: U256,
) -> anyhow::Result<()> {
    let actual_balance = runner.get_balance(address);

    if actual_balance != expected_balance {
        anyhow::bail!(
            "Expected and actual balance mismatch: {expected_balance} vs {actual_balance}"
        );
    }

    let actual_nonce = runner.get_nonce(address);

    if actual_nonce != expected_nonce {
        anyhow::bail!("Expected and actual nonce mismatch: {expected_nonce} vs {actual_nonce}");
    }

    Ok(())
}

pub fn address_from_hex(address: &str) -> Address {
    let bytes = address.strip_prefix("0x").map_or_else(
        || hex::decode(address).unwrap(),
        |address| hex::decode(address).unwrap(),
    );

    Address::try_from_slice(&bytes).unwrap()
}

pub fn str_to_account_id(account_id: &str) -> AccountId {
    account_id.parse().unwrap()
}

pub fn unwrap_success(result: SubmitResult) -> Vec<u8> {
    match result.status {
        TransactionStatus::Succeed(ret) => ret,
        other => panic!("Unexpected status: {other:?}"),
    }
}

pub fn unwrap_success_slice(result: &SubmitResult) -> &[u8] {
    match &result.status {
        TransactionStatus::Succeed(ret) => ret,
        other => panic!("Unexpected status: {other:?}"),
    }
}

pub fn unwrap_revert_slice(result: &SubmitResult) -> &[u8] {
    match &result.status {
        TransactionStatus::Revert(ret) => ret,
        other => panic!("Unexpected status: {other:?}"),
    }
}

pub fn panic_on_fail(status: TransactionStatus) {
    match status {
        TransactionStatus::Succeed(_) => (),
        TransactionStatus::Revert(message) => panic!("{}", String::from_utf8_lossy(&message)),
        other => panic!("{}", String::from_utf8_lossy(other.as_ref())),
    }
}

/// Checks if `total_gas` is within 1 Tgas of `tgas_bound`.
pub fn assert_gas_bound(total_gas: u64, tgas_bound: u64) {
    const TERA: i128 = 1_000_000_000_000;
    let total_gas: i128 = total_gas.into();
    let tgas_bound: i128 = i128::from(tgas_bound) * TERA;
    let diff = (total_gas - tgas_bound).abs() / TERA;
    assert_eq!(
        diff,
        0,
        "{} Tgas is not equal to {} Tgas",
        total_gas / TERA,
        tgas_bound / TERA,
    );
}

/// Returns true if `abs(a - b) / max(a, b) <= x / 100`. The implementation is written differently than
/// this simpler formula to avoid floating point arithmetic.
pub const fn within_x_percent(x: u64, a: u64, b: u64) -> bool {
    let (larger, smaller) = if a < b { (b, a) } else { (a, b) };

    (100 / x) * (larger - smaller) <= larger
}

fn into_engine_error(gas_used: u64, aborted: &FunctionCallError) -> EngineError {
    let kind = match aborted {
        FunctionCallError::HostError(HostError::GuestPanic { panic_msg }) => {
            match panic_msg.as_str() {
                "ERR_INVALID_CHAIN_ID" => EngineErrorKind::InvalidChainId,
                "ERR_OUT_OF_FUND" => EngineErrorKind::GasPayment(GasPaymentError::OutOfFund),
                "ERR_GAS_OVERFLOW" => EngineErrorKind::GasOverflow,
                "ERR_INTRINSIC_GAS" => EngineErrorKind::IntrinsicGasNotMet,
                "ERR_NOT_ALLOWED" => EngineErrorKind::NotAllowed,
                "ERR_SAME_OWNER" => EngineErrorKind::SameOwner,
                "ERR_FIXED_GAS_OVERFLOW" => EngineErrorKind::FixedGasOverflow,
                "ERR_PAUSED" => EngineErrorKind::EvmFatal(aurora_engine_evm::ExitFatal::Other(
                    "ERR_PAUSED".into(),
                )),
                msg if msg.starts_with("ERR_INCORRECT_NONCE") => {
                    EngineErrorKind::IncorrectNonce(msg.to_string())
                }
                msg => EngineErrorKind::EvmFatal(aurora_engine_evm::ExitFatal::Other(Cow::Owned(
                    msg.into(),
                ))),
            }
        }
        other => panic!("Other FunctionCallError: {other:?}"),
    };

    EngineError { kind, gas_used }
}
