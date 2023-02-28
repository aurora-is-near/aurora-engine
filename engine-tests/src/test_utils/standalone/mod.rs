use aurora_engine::engine;
use aurora_engine::parameters::{
    CallArgs, DeployErc20TokenArgs, PausePrecompilesCallArgs, SetOwnerArgs, SubmitResult,
    TransactionStatus,
};
use aurora_engine_sdk::env::{self, Env};
use aurora_engine_transactions::legacy::{LegacyEthSignedTransaction, TransactionLegacy};
use aurora_engine_types::types::{Address, NearGas, PromiseResult, Wei};
use aurora_engine_types::{H256, U256};
use borsh::BorshDeserialize;
use engine_standalone_storage::{
    sync::{
        self,
        types::{TransactionKind, TransactionMessage},
    },
    BlockMetadata, Diff, Storage,
};
use libsecp256k1::SecretKey;
use tempfile::TempDir;

use crate::test_utils;

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
        self.init_evm_with_chain_id(self.chain_id)
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
        let tx_msg = Self::template_tx_msg(storage, env, 0, transaction_hash, &[]);
        let result = storage.with_engine_access(env.block_height, 0, &[], |io| {
            mocks::init_evm(io, env, chain_id);
        });
        let outcome = sync::TransactionIncludedOutcome {
            hash: transaction_hash,
            info: tx_msg,
            diff: result.diff,
            maybe_result: Ok(None),
        };
        self.cumulative_diff.append(outcome.diff.clone());
        test_utils::standalone::storage::commit(storage, &outcome);
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
        let tx_msg = Self::template_tx_msg(storage, env, 0, transaction_hash, &[]);

        let result = storage.with_engine_access(env.block_height, 0, &[], |io| {
            mocks::mint_evm_account(address, balance, nonce, code, io, env)
        });
        let outcome = sync::TransactionIncludedOutcome {
            hash: transaction_hash,
            info: tx_msg,
            diff: result.diff,
            maybe_result: Ok(None),
        };
        self.cumulative_diff.append(outcome.diff.clone());
        test_utils::standalone::storage::commit(storage, &outcome);
    }

    pub fn transfer_with_signer(
        &mut self,
        signer: &mut test_utils::Signer,
        amount: Wei,
        dest: Address,
    ) -> Result<SubmitResult, engine::EngineError> {
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
    ) -> Result<SubmitResult, engine::EngineError> {
        let storage = &mut self.storage;
        let env = &mut self.env;
        env.block_height += 1;
        let signed_tx = test_utils::sign_transaction(transaction, Some(self.chain_id), account);
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
    ) -> Result<SubmitResult, engine::EngineError> {
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

        let mut tx_msg = Self::template_tx_msg(storage, env, 0, transaction_hash, &[]);
        tx_msg.position = transaction_position;
        tx_msg.transaction =
            TransactionKind::Submit(transaction_bytes.as_slice().try_into().unwrap());
        let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();

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

    pub fn submit_raw(
        &mut self,
        method_name: &str,
        ctx: &near_vm_logic::VMContext,
        promise_results: &[PromiseResult],
    ) -> Result<SubmitResult, engine::EngineError> {
        let mut env = self.env.clone();
        env.block_height = ctx.block_index;
        env.attached_deposit = ctx.attached_deposit;
        env.block_timestamp = aurora_engine_sdk::env::Timestamp::new(ctx.block_timestamp);
        env.predecessor_account_id = ctx.predecessor_account_id.as_ref().parse().unwrap();
        env.current_account_id = ctx.current_account_id.as_ref().parse().unwrap();
        env.signer_account_id = ctx.signer_account_id.as_ref().parse().unwrap();
        env.prepaid_gas = NearGas::new(ctx.prepaid_gas);

        let storage = &mut self.storage;
        if method_name == test_utils::SUBMIT {
            let transaction_bytes = &ctx.input;
            Self::internal_submit_transaction(
                transaction_bytes,
                0,
                storage,
                &mut env,
                &mut self.cumulative_diff,
                promise_results,
            )
        } else if method_name == test_utils::CALL {
            let call_args = CallArgs::try_from_slice(&ctx.input).unwrap();
            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let mut tx_msg =
                Self::template_tx_msg(storage, &env, 0, transaction_hash, promise_results);
            tx_msg.transaction = TransactionKind::Call(call_args);

            let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();
            self.cumulative_diff.append(outcome.diff.clone());
            test_utils::standalone::storage::commit(storage, &outcome);

            unwrap_result(outcome)
        } else if method_name == test_utils::DEPLOY_ERC20 {
            let deploy_args = DeployErc20TokenArgs::try_from_slice(&ctx.input).unwrap();
            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let mut tx_msg =
                Self::template_tx_msg(storage, &env, 0, transaction_hash, promise_results);
            tx_msg.transaction = TransactionKind::DeployErc20(deploy_args);

            let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();
            self.cumulative_diff.append(outcome.diff.clone());
            test_utils::standalone::storage::commit(storage, &outcome);

            let address = match outcome.maybe_result.unwrap().unwrap() {
                sync::TransactionExecutionResult::DeployErc20(address) => address,
                _ => unreachable!(),
            };
            Ok(SubmitResult::new(
                TransactionStatus::Succeed(address.raw().as_ref().to_vec()),
                0,
                Vec::new(),
            ))
        } else if method_name == test_utils::RESUME_PRECOMPILES {
            let input = &ctx.input[..];
            let call_args = PausePrecompilesCallArgs::try_from_slice(input)
                .expect("Unable to parse input as PausePrecompilesCallArgs");

            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let mut tx_msg =
                Self::template_tx_msg(storage, &env, 0, transaction_hash, promise_results);
            tx_msg.transaction = TransactionKind::ResumePrecompiles(call_args);

            let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();
            self.cumulative_diff.append(outcome.diff.clone());
            storage::commit(storage, &outcome);

            Ok(SubmitResult::new(
                TransactionStatus::Succeed(Vec::new()),
                0,
                Vec::new(),
            ))
        } else if method_name == test_utils::PAUSE_PRECOMPILES {
            let input = &ctx.input[..];
            let call_args = PausePrecompilesCallArgs::try_from_slice(input)
                .expect("Unable to parse input as PausePrecompilesCallArgs");

            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let mut tx_msg =
                Self::template_tx_msg(storage, &env, 0, transaction_hash, promise_results);
            tx_msg.transaction = TransactionKind::PausePrecompiles(call_args);

            let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();
            self.cumulative_diff.append(outcome.diff.clone());
            storage::commit(storage, &outcome);

            Ok(SubmitResult::new(
                TransactionStatus::Succeed(Vec::new()),
                0,
                Vec::new(),
            ))
        } else if method_name == test_utils::SET_OWNER {
            let input = &ctx.input[..];
            let call_args =
                SetOwnerArgs::try_from_slice(input).expect("Unable to parse input as SetOwnerArgs");

            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let mut tx_msg =
                Self::template_tx_msg(storage, &env, 0, transaction_hash, promise_results);
            tx_msg.transaction = TransactionKind::SetOwner(call_args);

            let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();
            self.cumulative_diff.append(outcome.diff.clone());
            storage::commit(storage, &outcome);

            Ok(SubmitResult::new(
                TransactionStatus::Succeed(Vec::new()),
                0,
                Vec::new(),
            ))
        } else {
            panic!("Unsupported standalone method {}", method_name);
        }
    }

    pub fn get_current_state(&self) -> &Diff {
        &self.cumulative_diff
    }

    pub fn get_balance(&mut self, address: &Address) -> Wei {
        self.storage
            .with_engine_access(self.env.block_height + 1, 0, &[], |io| {
                engine::get_balance(&io, address)
            })
            .result
    }

    pub fn get_nonce(&mut self, address: &Address) -> U256 {
        self.storage
            .with_engine_access(self.env.block_height + 1, 0, &[], |io| {
                engine::get_nonce(&io, address)
            })
            .result
    }

    pub fn get_code(&mut self, address: &Address) -> Vec<u8> {
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
    ) -> TransactionMessage {
        let block_hash = mocks::compute_block_hash(env.block_height);
        let block_metadata = BlockMetadata {
            timestamp: env.block_timestamp,
            random_seed: env.random_seed,
        };
        storage
            .set_block_data(block_hash, env.block_height, block_metadata)
            .unwrap();
        let promise_data = promise_results
            .iter()
            .map(|p| match p {
                PromiseResult::Failed | PromiseResult::NotReady => None,
                PromiseResult::Successful(bytes) => Some(bytes.clone()),
            })
            .collect();
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
        }
    }

    fn internal_submit_transaction<'db>(
        transaction_bytes: &[u8],
        transaction_position: u16,
        storage: &'db mut Storage,
        env: &mut env::Fixed,
        cumulative_diff: &mut Diff,
        promise_results: &[PromiseResult],
    ) -> Result<SubmitResult, engine::EngineError> {
        let transaction_hash = aurora_engine_sdk::keccak(transaction_bytes);
        let mut tx_msg = Self::template_tx_msg(
            storage,
            env,
            transaction_position,
            transaction_hash,
            promise_results,
        );
        tx_msg.transaction = TransactionKind::Submit(transaction_bytes.try_into().unwrap());

        let outcome = sync::execute_transaction_message(storage, tx_msg).unwrap();
        cumulative_diff.append(outcome.diff.clone());
        test_utils::standalone::storage::commit(storage, &outcome);

        unwrap_result(outcome)
    }
}

fn unwrap_result(
    outcome: sync::TransactionIncludedOutcome,
) -> Result<SubmitResult, engine::EngineError> {
    match outcome.maybe_result.unwrap().unwrap() {
        sync::TransactionExecutionResult::Submit(result) => result,
        sync::TransactionExecutionResult::Promise(_) => panic!("Unexpected promise."),
        sync::TransactionExecutionResult::DeployErc20(_) => panic!("Unexpected DeployErc20."),
    }
}

impl Default for StandaloneRunner {
    fn default() -> Self {
        let (storage_dir, storage) = storage::create_db();
        let env = mocks::default_env(0);
        let chain_id = test_utils::AuroraRunner::default().chain_id;
        Self {
            storage_dir,
            storage,
            env,
            chain_id,
            cumulative_diff: Diff::default(),
        }
    }
}
