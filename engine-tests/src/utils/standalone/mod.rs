use aurora_engine::engine;
use aurora_engine::parameters::{SubmitResult, TransactionStatus};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::env::{self, Env};
use aurora_engine_transactions::legacy::{LegacyEthSignedTransaction, TransactionLegacy};
use aurora_engine_types::types::{Address, NearGas, PromiseResult, Wei};
use aurora_engine_types::{H256, U256};
use engine_standalone_storage::{
    sync::{
        self,
        types::{TransactionKind, TransactionMessage},
    },
    BlockMetadata, Diff, Storage,
};
use libsecp256k1::SecretKey;
use tempfile::TempDir;

use crate::utils;

pub mod mocks;
pub mod storage;

pub struct StandaloneRunner {
    pub storage_dir: TempDir,
    pub storage: Storage,
    pub env: env::Fixed,
    pub chain_id: u64,
    // Cumulative diff from all transactions (ie full state representation)
    pub cumulative_diff: Diff,
}

impl StandaloneRunner {
    pub fn init_evm(&mut self) {
        self.init_evm_with_chain_id(self.chain_id);
    }

    pub fn init_evm_with_chain_id(&mut self, chain_id: u64) {
        self.chain_id = chain_id;
        let storage = &mut self.storage;
        let env = &mut self.env;
        storage
            .set_engine_account_id(&env.current_account_id)
            .unwrap();
        env.block_height += 1;
        let transaction_hash = H256::zero();
        let tx_msg = Self::template_tx_msg(storage, env, 0, transaction_hash, &[], Vec::new());
        let result = storage.with_engine_access(env.block_height, 0, &[], |io| {
            mocks::init_evm(io, env, chain_id);
            #[cfg(feature = "ext-connector")]
            mocks::init_connector(io);
            #[cfg(not(feature = "ext-connector"))]
            mocks::init_connector(io, env);
        });
        let outcome = sync::TransactionIncludedOutcome {
            hash: transaction_hash,
            info: tx_msg,
            diff: result.diff,
            maybe_result: Ok(None),
        };
        self.cumulative_diff.append(outcome.diff.clone());
        storage::commit(storage, &outcome);
    }

    pub fn mint_account(
        &mut self,
        address: Address,
        balance: Wei,
        nonce: U256,
        code: Option<Vec<u8>>,
    ) {
        let storage = &mut self.storage;
        let env = &mut self.env;
        let transaction_hash = {
            let bytes = [
                address.raw().as_ref(),
                &balance.to_bytes(),
                &aurora_engine_types::types::u256_to_arr(&nonce),
            ]
            .concat();
            aurora_engine_sdk::keccak(&bytes)
        };

        env.block_height += 1;
        let tx_msg = Self::template_tx_msg(storage, env, 0, transaction_hash, &[], Vec::new());

        let result = storage.with_engine_access(env.block_height, 0, &[], |io| {
            mocks::mint_evm_account(address, balance, nonce, code, io, env);
        });
        let outcome = sync::TransactionIncludedOutcome {
            hash: transaction_hash,
            info: tx_msg,
            diff: result.diff,
            maybe_result: Ok(None),
        };
        self.cumulative_diff.append(outcome.diff.clone());
        storage::commit(storage, &outcome);
    }

    pub fn transfer_with_signer(
        &mut self,
        signer: &mut utils::Signer,
        amount: Wei,
        dest: Address,
    ) -> Result<SubmitResult, sync::error::Error> {
        let tx = TransactionLegacy {
            nonce: signer.use_nonce().into(),
            gas_price: U256::zero(),
            gas_limit: u64::MAX.into(),
            to: Some(dest),
            value: amount,
            data: Vec::new(),
        };
        self.submit_transaction(&signer.secret_key, tx)
    }

    pub fn submit_transaction(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
    ) -> Result<SubmitResult, sync::error::Error> {
        let storage = &mut self.storage;
        let env = &mut self.env;
        env.block_height += 1;
        let signed_tx = utils::sign_transaction(transaction, Some(self.chain_id), account);
        let transaction_bytes = rlp::encode(&signed_tx).to_vec();

        Self::internal_submit_transaction(
            &transaction_bytes,
            0,
            storage,
            env,
            &mut self.cumulative_diff,
            &[],
        )
    }

    pub fn submit_raw_transaction_bytes(
        &mut self,
        transaction_bytes: &[u8],
    ) -> Result<SubmitResult, sync::error::Error> {
        self.env.predecessor_account_id = "some-account.near".parse().unwrap();
        let storage = &mut self.storage;
        let env = &mut self.env;
        env.block_height += 1;

        Self::internal_submit_transaction(
            transaction_bytes,
            0,
            storage,
            env,
            &mut self.cumulative_diff,
            &[],
        )
    }

    /// Note: does not persist the diff in the DB.
    pub fn execute_transaction_at_position(
        &mut self,
        signed_tx: &LegacyEthSignedTransaction,
        block_height: u64,
        transaction_position: u16,
    ) -> Result<sync::TransactionIncludedOutcome, engine::EngineError> {
        let storage = &mut self.storage;
        let env = &mut self.env;

        env.block_height = block_height;
        let transaction_bytes = rlp::encode(signed_tx).to_vec();
        let transaction_hash = aurora_engine_sdk::keccak(&transaction_bytes);

        let mut tx_msg = Self::template_tx_msg(
            storage,
            env,
            0,
            transaction_hash,
            &[],
            transaction_bytes.clone(),
        );
        tx_msg.position = transaction_position;
        tx_msg.transaction =
            TransactionKind::Submit(transaction_bytes.as_slice().try_into().unwrap());
        let outcome = sync::execute_transaction_message::<AuroraModExp>(storage, tx_msg).unwrap();

        match outcome.maybe_result.as_ref().unwrap().as_ref().unwrap() {
            sync::TransactionExecutionResult::Submit(result) => {
                if let Err(e) = result.as_ref() {
                    return Err(e.clone());
                }
            }
            _ => unreachable!(),
        };

        Ok(outcome)
    }

    #[allow(clippy::too_many_lines)]
    pub fn submit_raw(
        &mut self,
        method_name: &str,
        ctx: &near_vm_runner::logic::VMContext,
        promise_results: &[PromiseResult],
        block_random_value: Option<H256>,
    ) -> Result<SubmitResult, engine::EngineError> {
        let mut env = self.env.clone();
        env.block_height = ctx.block_height;
        env.attached_deposit = ctx.attached_deposit;
        env.block_timestamp = env::Timestamp::new(ctx.block_timestamp);
        env.predecessor_account_id = ctx.predecessor_account_id.as_str().parse().unwrap();
        env.current_account_id = ctx.current_account_id.as_str().parse().unwrap();
        env.signer_account_id = ctx.signer_account_id.as_str().parse().unwrap();
        env.prepaid_gas = NearGas::new(ctx.prepaid_gas);
        if let Some(value) = block_random_value {
            env.random_seed = value;
        }

        let promise_data: Vec<_> = promise_results
            .iter()
            .map(|r| match r {
                PromiseResult::Successful(bytes) => Some(bytes.clone()),
                PromiseResult::Failed | PromiseResult::NotReady => None,
            })
            .collect();
        let transaction_kind =
            sync::parse_transaction_kind(method_name, ctx.input.clone(), &promise_data)
                .expect("All method names must be known by standalone");

        let transaction_hash = if let TransactionKind::SubmitWithArgs(args) = &transaction_kind {
            aurora_engine_sdk::keccak(&args.tx_data)
        } else {
            aurora_engine_sdk::keccak(&ctx.input)
        };

        let storage = &mut self.storage;
        let mut tx_msg = Self::template_tx_msg(
            storage,
            &env,
            0,
            transaction_hash,
            promise_results,
            ctx.input.clone(),
        );
        tx_msg.transaction = transaction_kind;

        if ctx.random_seed.len() == 32 {
            let runtime_random_value = {
                use near_primitives_core::hash::CryptoHash;
                let action_hash = CryptoHash(tx_msg.action_hash.0);
                let random_seed = CryptoHash(env.random_seed.0);
                near_primitives::utils::create_random_seed(u32::MAX, action_hash, random_seed)
            };
            assert_eq!(
                ctx.random_seed, runtime_random_value,
                "Runtime random value should match computed value when it is specified"
            );
        }

        let outcome = sync::execute_transaction_message::<AuroraModExp>(storage, tx_msg).unwrap();
        self.cumulative_diff.append(outcome.diff.clone());
        storage::commit(storage, &outcome);

        match outcome.maybe_result.unwrap() {
            Some(sync::TransactionExecutionResult::Submit(result)) => result,
            Some(sync::TransactionExecutionResult::DeployErc20(address)) => Ok(SubmitResult::new(
                TransactionStatus::Succeed(address.raw().as_ref().to_vec()),
                0,
                Vec::new(),
            )),
            _ => Ok(SubmitResult::new(
                TransactionStatus::Succeed(Vec::new()),
                0,
                Vec::new(),
            )),
        }
    }

    pub const fn get_current_state(&self) -> &Diff {
        &self.cumulative_diff
    }

    pub fn get_balance(&self, address: &Address) -> Wei {
        self.storage
            .with_engine_access(self.env.block_height + 1, 0, &[], |io| {
                engine::get_balance(&io, address)
            })
            .result
    }

    pub fn get_nonce(&self, address: &Address) -> U256 {
        self.storage
            .with_engine_access(self.env.block_height + 1, 0, &[], |io| {
                engine::get_nonce(&io, address)
            })
            .result
    }

    pub fn get_code(&self, address: &Address) -> Vec<u8> {
        self.storage
            .with_engine_access(self.env.block_height + 1, 0, &[], |io| {
                engine::get_code(&io, address)
            })
            .result
    }

    pub fn close(self) {
        drop(self.storage);
        self.storage_dir.close().unwrap();
    }

    pub(crate) fn template_tx_msg(
        storage: &mut Storage,
        env: &env::Fixed,
        transaction_position: u16,
        transaction_hash: H256,
        promise_results: &[PromiseResult],
        raw_input: Vec<u8>,
    ) -> TransactionMessage {
        let block_hash = mocks::compute_block_hash(env.block_height);
        let block_metadata = BlockMetadata {
            timestamp: env.block_timestamp,
            random_seed: env.random_seed,
        };
        storage
            .set_block_data(block_hash, env.block_height, &block_metadata)
            .unwrap();
        let promise_data = promise_results
            .iter()
            .map(|p| match p {
                PromiseResult::Failed | PromiseResult::NotReady => None,
                PromiseResult::Successful(bytes) => Some(bytes.clone()),
            })
            .collect();
        let action_hash = {
            let mut bytes = Vec::with_capacity(32 + 32 + 8);
            bytes.extend_from_slice(transaction_hash.as_bytes());
            bytes.extend_from_slice(block_hash.as_bytes());
            bytes.extend_from_slice(&(u64::MAX - u64::from(transaction_position)).to_le_bytes());
            aurora_engine_sdk::sha256(&bytes)
        };
        TransactionMessage {
            block_hash,
            near_receipt_id: transaction_hash,
            position: transaction_position,
            succeeded: true,
            signer: env.signer_account_id(),
            caller: env.predecessor_account_id(),
            attached_near: env.attached_deposit,
            transaction: TransactionKind::Unknown,
            promise_data,
            raw_input,
            action_hash,
        }
    }

    fn internal_submit_transaction(
        transaction_bytes: &[u8],
        transaction_position: u16,
        storage: &mut Storage,
        env: &env::Fixed,
        cumulative_diff: &mut Diff,
        promise_results: &[PromiseResult],
    ) -> Result<SubmitResult, sync::error::Error> {
        let transaction_hash = aurora_engine_sdk::keccak(transaction_bytes);
        let mut tx_msg = Self::template_tx_msg(
            storage,
            env,
            transaction_position,
            transaction_hash,
            promise_results,
            transaction_bytes.to_vec(),
        );
        tx_msg.transaction = TransactionKind::Submit(transaction_bytes.try_into().unwrap());

        let outcome = sync::execute_transaction_message::<AuroraModExp>(storage, tx_msg).unwrap();
        cumulative_diff.append(outcome.diff.clone());
        storage::commit(storage, &outcome);

        unwrap_result(outcome)
    }
}

fn unwrap_result(
    outcome: sync::TransactionIncludedOutcome,
) -> Result<SubmitResult, sync::error::Error> {
    match outcome.maybe_result?.unwrap() {
        sync::TransactionExecutionResult::Submit(result) => result.map_err(Into::into),
        sync::TransactionExecutionResult::Promise(_) => panic!("Unexpected promise."),
        sync::TransactionExecutionResult::DeployErc20(_) => panic!("Unexpected DeployErc20."),
    }
}

impl Default for StandaloneRunner {
    fn default() -> Self {
        let (storage_dir, mut storage) = storage::create_db();
        let env = mocks::default_env(0);
        storage
            .set_engine_account_id(&env.current_account_id)
            .unwrap();
        Self {
            storage_dir,
            storage,
            env,
            chain_id: utils::DEFAULT_CHAIN_ID,
            cumulative_diff: Diff::default(),
        }
    }
}
