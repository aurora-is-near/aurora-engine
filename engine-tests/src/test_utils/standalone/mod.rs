use aurora_engine::engine;
use aurora_engine::parameters::{CallArgs, DeployErc20TokenArgs, SubmitResult, TransactionStatus};
use aurora_engine::transaction::legacy::{LegacyEthSignedTransaction, TransactionLegacy};
use aurora_engine_sdk::env::{self, Env};
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{types::Wei, types_new::Address, H256, U256};
use borsh::BorshDeserialize;
use engine_standalone_storage::engine_state;
use engine_standalone_storage::{BlockMetadata, Diff, Storage};
use secp256k1::SecretKey;
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
        env.block_height += 1;
        let io = Self::get_engine_io(storage, env, 0, H256([0u8; 32]));
        mocks::init_evm(io.engine_io, env, chain_id);
        io.finish().commit(storage, &mut self.cumulative_diff);
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
                address.as_ref(),
                &balance.to_bytes(),
                &aurora_engine_types::types::u256_to_arr(&nonce),
            ]
            .concat();
            aurora_engine_sdk::keccak(&bytes)
        };

        env.block_height += 1;
        let io = Self::get_engine_io(storage, env, 0, transaction_hash);

        mocks::mint_evm_account(address, balance, nonce, code, io.engine_io, env);

        io.finish().commit(storage, &mut self.cumulative_diff);
    }

    pub fn submit_transaction(
        &mut self,
        account: &SecretKey,
        transaction: TransactionLegacy,
    ) -> Result<SubmitResult, engine::EngineError> {
        self.env.predecessor_account_id = "some-account.near".parse().unwrap();
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
            &transaction_bytes,
            0,
            storage,
            env,
            &mut self.cumulative_diff,
        )
    }

    /// Note: does not persist the diff in the DB.
    pub fn execute_transaction_at_position(
        &mut self,
        signed_tx: &LegacyEthSignedTransaction,
        block_height: u64,
        transaction_position: u16,
    ) -> Result<TransactionComplete, engine::EngineError> {
        let storage = &mut self.storage;
        let env = &mut self.env;

        env.block_height = block_height;
        let transaction_bytes = rlp::encode(signed_tx).to_vec();
        let transaction_hash = aurora_engine_sdk::keccak(&transaction_bytes);
        let relayer_address = Self::relayer_address(env);

        let io = Self::get_engine_io(storage, env, transaction_position, transaction_hash);
        let engine_state = engine::get_state(&io.engine_io).unwrap();
        let mut handler = mocks::promise::PromiseTracker::default();

        engine::submit(
            io.engine_io,
            env,
            &transaction_bytes,
            engine_state,
            env.current_account_id(),
            relayer_address,
            &mut handler,
        )?;

        Ok(io.finish())
    }

    pub fn submit_raw(
        &mut self,
        method_name: &str,
        ctx: &near_vm_logic::VMContext,
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
            )
        } else if method_name == test_utils::CALL {
            let call_args = CallArgs::try_from_slice(&ctx.input).unwrap();
            let mut handler = mocks::promise::PromiseTracker::default();
            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let io = Self::get_engine_io(storage, &env, 0, transaction_hash);
            let origin = aurora_engine_sdk::types::near_account_to_evm_address(
                env.predecessor_account_id.as_bytes(),
            );
            let mut engine =
                engine::Engine::new(origin, env.current_account_id(), io.engine_io, &env).unwrap();
            let result = engine.call_with_args(call_args, &mut handler)?;
            io.finish().commit(storage, &mut self.cumulative_diff);
            Ok(result)
        } else if method_name == test_utils::DEPLOY_ERC20 {
            let deploy_args = DeployErc20TokenArgs::try_from_slice(&ctx.input).unwrap();
            let mut handler = mocks::promise::PromiseTracker::default();
            let transaction_hash = aurora_engine_sdk::keccak(&ctx.input);
            let io = Self::get_engine_io(storage, &env, 0, transaction_hash);
            let address = engine::deploy_erc20_token(deploy_args, io.engine_io, &env, &mut handler)
                .map_err(mocks::unsafe_to_string)
                .unwrap();
            io.finish().commit(storage, &mut self.cumulative_diff);
            Ok(SubmitResult::new(
                TransactionStatus::Succeed(address.as_ref().to_vec()),
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
        let io = self
            .storage
            .access_engine_storage_at_position(self.env.block_height + 1, 0, &[]);
        engine::get_balance(&io, address)
    }

    pub fn get_nonce(&mut self, address: &Address) -> U256 {
        let io = self
            .storage
            .access_engine_storage_at_position(self.env.block_height + 1, 0, &[]);
        engine::get_nonce(&io, address)
    }

    pub fn get_code(&mut self, address: &Address) -> Vec<u8> {
        let io = self
            .storage
            .access_engine_storage_at_position(self.env.block_height + 1, 0, &[]);
        engine::get_code(&io, address)
    }

    pub fn close(self) {
        drop(self.storage);
        self.storage_dir.close().unwrap();
    }

    fn get_engine_io<'db>(
        storage: &'db mut Storage,
        env: &env::Fixed,
        transaction_position: u16,
        transaction_hash: H256,
    ) -> TransactionIO<'db> {
        let block_hash = mocks::compute_block_hash(env.block_height);
        let block_metadata = BlockMetadata {
            timestamp: env.block_timestamp,
            random_seed: env.random_seed,
        };
        storage
            .set_block_data(block_hash, env.block_height, block_metadata)
            .unwrap();
        let io =
            storage.access_engine_storage_at_position(env.block_height, transaction_position, &[]);
        TransactionIO {
            engine_io: io,
            block_hash,
            transaction_position,
            transaction_hash,
        }
    }

    fn internal_submit_transaction<'db>(
        transaction_bytes: &[u8],
        transaction_position: u16,
        storage: &'db mut Storage,
        env: &mut env::Fixed,
        cumulative_diff: &mut Diff,
    ) -> Result<SubmitResult, engine::EngineError> {
        let relayer_address = Self::relayer_address(env);
        let transaction_hash = aurora_engine_sdk::keccak(&transaction_bytes);
        let io = Self::get_engine_io(storage, env, transaction_position, transaction_hash);
        let engine_state = engine::get_state(&io.engine_io).unwrap();
        let mut handler = mocks::promise::PromiseTracker::default();

        let result = engine::submit(
            io.engine_io,
            env,
            &transaction_bytes,
            engine_state,
            env.current_account_id(),
            relayer_address,
            &mut handler,
        )?;
        io.finish().commit(storage, cumulative_diff);

        Ok(result)
    }

    fn relayer_address(env: &env::Fixed) -> Address {
        aurora_engine_sdk::types::near_account_to_evm_address(env.predecessor_account_id.as_bytes())
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

struct TransactionIO<'db> {
    engine_io: engine_state::EngineStateAccess<'db, 'db, 'db>,
    block_hash: H256,
    transaction_position: u16,
    transaction_hash: H256,
}

impl<'db> TransactionIO<'db> {
    // Drops `self.engine_io` which releases the borrow on the storage instance used to create it.
    // This allows the same storage instance to be passed to the `TransactionComplete::commit` function.
    fn finish(self) -> TransactionComplete {
        TransactionComplete {
            diff: self.engine_io.get_transaction_diff(),
            block_hash: self.block_hash,
            transaction_position: self.transaction_position,
            transaction_hash: self.transaction_hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionComplete {
    pub diff: Diff,
    pub block_hash: H256,
    pub transaction_position: u16,
    pub transaction_hash: H256,
}

impl TransactionComplete {
    pub fn commit(self, storage: &mut Storage, cumulative_diff: &mut Diff) {
        cumulative_diff.append(self.diff.clone());
        storage::commit(
            storage,
            self.diff,
            self.block_hash,
            self.transaction_position,
            self.transaction_hash,
        );
    }
}
