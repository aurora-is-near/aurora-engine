use crate::{BlockInfo, EVMHandler, TransactExecutionResult, TransactResult, TransactionInfo};

use crate::sputnikvm::utility::{
    compute_block_hash, exit_reason_into_result, get_balance, get_code, get_generation, get_nonce,
    get_storage, is_account_empty, remove_account, remove_all_storage, remove_storage, set_balance,
    set_code, set_nonce, set_storage,
};
use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::caching::FullCache;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::{PromiseHandler, ReadOnlyPromiseHandler};
use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{BTreeMap, Box, Vec, H160, H256, U256};
use core::cell::RefCell;
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::{executor, Config, CreateScheme};

mod accounting;
pub mod errors;
mod utility;

pub const CONFIG: &Config = &Config::shanghai();

/// `SputnikVM` handler
pub struct SputnikVMHandler<'env, I: IO, E: Env, H: PromiseHandler> {
    io: I,
    env: &'env E,
    precompiles: Precompiles<'env, I, E, H::ReadOnly>,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
}

impl<'env, I: IO + Copy, E: Env, H: PromiseHandler> SputnikVMHandler<'env, I, E, H> {
    pub fn new(
        io: I,
        env: &'env E,
        transaction: &'env TransactionInfo,
        block: &'env BlockInfo,
        precompiles: Precompiles<'env, I, E, H::ReadOnly>,
        remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
    ) -> Self {
        Self {
            io,
            env,
            precompiles,
            transaction,
            block,
            remove_eth_fn,
        }
    }
}

impl<'env, I: IO + Copy, E: Env, H: PromiseHandler> EVMHandler for SputnikVMHandler<'env, I, E, H> {
    fn transact_create(&mut self) -> TransactExecutionResult<TransactResult> {
        let mut contract_state = ContractState::new(
            self.io,
            self.env,
            self.transaction,
            self.block,
            self.remove_eth_fn.take(),
        );
        let executor_params =
            StackExecutorParams::new(self.transaction.gas_limit, &self.precompiles);
        let mut executor = executor_params.make_executor(&contract_state);
        let scheme = self.transaction.address.map_or_else(
            || CreateScheme::Legacy {
                caller: self.transaction.origin,
            },
            CreateScheme::Fixed,
        );
        let address = executor.create_address(scheme);
        let (exit_reason, return_value) = match scheme {
            CreateScheme::Legacy { caller } => executor.transact_create(
                caller,
                self.transaction.value.raw(),
                self.transaction.input.clone(),
                self.transaction.gas_limit,
                self.transaction.access_list.clone(),
            ),
            CreateScheme::Fixed(address) => executor.transact_create_fixed(
                self.transaction.origin,
                address,
                self.transaction.value.raw(),
                self.transaction.input.clone(),
                self.transaction.gas_limit,
                self.transaction.access_list.clone(),
            ),
            CreateScheme::Create2 { .. } => unreachable!(),
        };
        let result = if exit_reason.is_succeed() {
            address.0.to_vec()
        } else {
            return_value
        };
        let status = exit_reason_into_result(exit_reason, result)?;

        let used_gas = executor.used_gas();
        let (values, executor_logs) = executor.into_state().deconstruct();
        contract_state.apply(values, Vec::<Log>::new(), true);
        let logs: Vec<crate::Log> = executor_logs.into_iter().map(Into::into).collect();
        Ok(TransactResult {
            submit_result: SubmitResult::new(status, used_gas, Vec::new()),
            logs,
        })
    }

    fn transact_call(&mut self) -> TransactExecutionResult<TransactResult> {
        let mut contract_state = ContractState::new(
            self.io,
            self.env,
            self.transaction,
            self.block,
            self.remove_eth_fn.take(),
        );
        let executor_params =
            StackExecutorParams::new(self.transaction.gas_limit, &self.precompiles);
        let mut executor = executor_params.make_executor(&contract_state);
        let (exit_reason, result) = executor.transact_call(
            self.transaction.origin,
            self.transaction
                .address
                .expect("transact_call address must be set"),
            self.transaction.value.raw(),
            self.transaction.input.clone(),
            self.transaction.gas_limit,
            self.transaction.access_list.clone(),
        );
        let status = exit_reason_into_result(exit_reason, result)?;

        let used_gas = executor.used_gas();
        let (values, executor_logs) = executor.into_state().deconstruct();
        contract_state.apply(values, Vec::<Log>::new(), true);
        let logs: Vec<crate::Log> = executor_logs.into_iter().map(Into::into).collect();
        Ok(TransactResult {
            submit_result: SubmitResult::new(status, used_gas, Vec::new()),
            logs,
        })
    }

    fn view(&mut self) -> TransactExecutionResult<TransactionStatus> {
        let contract_state = ContractState::new(
            self.io,
            self.env,
            self.transaction,
            self.block,
            self.remove_eth_fn.take(),
        );
        let executor_params =
            StackExecutorParams::new(self.transaction.gas_limit, &self.precompiles);
        let mut executor = executor_params.make_executor(&contract_state);
        let (exit_reason, result) = executor.transact_call(
            self.transaction.origin,
            // TODO: check it
            self.transaction.address.unwrap(),
            self.transaction.value.raw(),
            self.transaction.input.clone(),
            self.transaction.gas_limit,
            self.transaction.access_list.clone(),
        );
        let status = exit_reason_into_result(exit_reason, result)?;
        Ok(status)
    }
}

pub struct StackExecutorParams<'env, I, E, H> {
    precompiles: &'env Precompiles<'env, I, E, H>,
    gas_limit: u64,
}

impl<'env, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> StackExecutorParams<'env, I, E, H> {
    const fn new(gas_limit: u64, precompiles: &'env Precompiles<'env, I, E, H>) -> Self {
        Self {
            precompiles,
            gas_limit,
        }
    }

    #[allow(clippy::type_complexity)]
    fn make_executor<'a>(
        &'a self,
        contract_state: &'a ContractState<'env, I, E>,
    ) -> executor::stack::StackExecutor<
        'env,
        'a,
        executor::stack::MemoryStackState<ContractState<'env, I, E>>,
        Precompiles<'env, I, E, H>,
    > {
        let metadata = executor::stack::StackSubstateMetadata::new(self.gas_limit, CONFIG);
        let state = executor::stack::MemoryStackState::new(metadata, contract_state);
        executor::stack::StackExecutor::new_with_precompiles(state, CONFIG, self.precompiles)
    }
}

pub struct ContractState<'env, I: IO, E: Env> {
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
    generation_cache: RefCell<BTreeMap<Address, u32>>,
    contract_storage_cache: RefCell<FullCache<(Address, H256), H256>>,
    account_info_cache: RefCell<FullCache<Address, Basic>>,
    contract_code_cache: RefCell<FullCache<Address, Vec<u8>>>,
    remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
}

impl<'env, I: IO + Copy, E: Env> ContractState<'env, I, E> {
    pub fn new(
        io: I,
        env: &'env E,
        transaction: &'env TransactionInfo,
        block: &'env BlockInfo,
        remove_eth_fn: Option<Box<dyn FnOnce(Wei) + 'env>>,
    ) -> Self {
        Self {
            io,
            env,
            transaction,
            block,
            generation_cache: RefCell::new(BTreeMap::new()),
            contract_storage_cache: RefCell::new(FullCache::default()),
            account_info_cache: RefCell::new(FullCache::default()),
            contract_code_cache: RefCell::new(FullCache::default()),
            remove_eth_fn,
        }
    }
}

impl<'env, I: IO, E: Env> Backend for ContractState<'env, I, E> {
    /// Returns the "effective" gas price (as defined by EIP-1559)
    fn gas_price(&self) -> U256 {
        self.block.gas_price
    }

    /// Returns the origin address that created the contract.
    fn origin(&self) -> H160 {
        self.transaction.origin
    }

    /// Returns a block hash from a given index.
    ///
    /// Currently, this returns
    /// 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff if
    /// only for the 256 most recent blocks, excluding of the current one.
    /// Otherwise, it returns 0x0.
    ///
    /// In other words, if the requested block index is less than the current
    /// block index, return
    /// 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff.
    /// Otherwise, return 0.
    ///
    /// This functionality may change in the future. Follow
    /// [nearcore#3456](https://github.com/near/nearcore/issues/3456) for more
    /// details.
    ///
    /// See: `https://doc.aurora.dev/develop/compat/evm#blockhash`
    fn block_hash(&self, number: U256) -> H256 {
        let idx = U256::from(self.env.block_height());
        if idx.saturating_sub(U256::from(256)) <= number && number < idx {
            // since `idx` comes from `u64` it is always safe to downcast `number` from `U256`
            compute_block_hash(
                self.block.chain_id,
                number.low_u64(),
                self.block.current_account_id.as_bytes(),
            )
        } else {
            H256::zero()
        }
    }

    /// Returns the current block index number.
    fn block_number(&self) -> U256 {
        U256::from(self.env.block_height())
    }

    /// Returns a mocked coinbase which is the EVM address for the Aurora
    /// account, being 0x4444588443C3a91288c5002483449Aba1054192b.
    ///
    /// See: `https://doc.aurora.dev/develop/compat/evm#coinbase`
    fn block_coinbase(&self) -> H160 {
        H160([
            0x44, 0x44, 0x58, 0x84, 0x43, 0xC3, 0xa9, 0x12, 0x88, 0xc5, 0x00, 0x24, 0x83, 0x44,
            0x9A, 0xba, 0x10, 0x54, 0x19, 0x2b,
        ])
    }

    /// Returns the current block timestamp.
    fn block_timestamp(&self) -> U256 {
        U256::from(self.env.block_timestamp().secs())
    }

    /// Returns the current block difficulty.
    ///
    /// See: `https://doc.aurora.dev/develop/compat/evm#difficulty`
    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    /// Get environmental block randomness.
    fn block_randomness(&self) -> Option<H256> {
        Some(self.env.random_seed())
    }

    /// Returns the current block gas limit.
    ///
    /// Currently, this returns 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
    /// as there isn't a gas limit alternative right now but this may change in
    /// the future.
    ///
    /// See: `https://doc.aurora.dev/develop/compat/evm#gaslimit`
    fn block_gas_limit(&self) -> U256 {
        U256::max_value()
    }

    /// Returns the current base fee for the current block.
    ///
    /// Currently, this returns 0 as there is no concept of a base fee at this
    /// time but this may change in the future.
    ///
    /// TODO: doc.aurora.dev link
    fn block_base_fee_per_gas(&self) -> U256 {
        U256::zero()
    }

    /// Returns the states chain ID.
    fn chain_id(&self) -> U256 {
        U256::from(self.block.chain_id)
    }

    /// Checks if an address exists.
    fn exists(&self, address: H160) -> bool {
        let address = Address::new(address);
        let mut cache = self.account_info_cache.borrow_mut();
        let basic_info = cache.get_or_insert_with(address, || Basic {
            nonce: get_nonce(&self.io, &address),
            balance: get_balance(&self.io, &address).raw(),
        });
        if !basic_info.balance.is_zero() || !basic_info.nonce.is_zero() {
            return true;
        }
        let mut cache = self.contract_code_cache.borrow_mut();
        let code = cache.get_or_insert_with(address, || get_code(&self.io, &address));
        !code.is_empty()
    }

    /// Returns basic account information.
    fn basic(&self, address: H160) -> Basic {
        let address = Address::new(address);
        let result = self
            .account_info_cache
            .borrow_mut()
            .get_or_insert_with(address, || Basic {
                nonce: get_nonce(&self.io, &address),
                balance: get_balance(&self.io, &address).raw(),
            })
            .clone();
        result
    }

    /// Returns the code of the contract from an address.
    fn code(&self, address: H160) -> Vec<u8> {
        let address = Address::new(address);
        self.contract_code_cache
            .borrow_mut()
            .get_or_insert_with(address, || get_code(&self.io, &address))
            .clone()
    }

    /// Get storage value of address at index.
    fn storage(&self, address: H160, index: H256) -> H256 {
        let address = Address::new(address);
        let generation = *self
            .generation_cache
            .borrow_mut()
            .entry(address)
            .or_insert_with(|| get_generation(&self.io, &address));
        let result = *self
            .contract_storage_cache
            .borrow_mut()
            .get_or_insert_with((address, index), || {
                get_storage(&self.io, &address, &index, generation)
            });
        result
    }

    /// Get original storage value of address at index, if available.
    ///
    /// Since `SputnikVM` collects storage changes in memory until the transaction is over,
    /// the "original storage" will always be the same as the storage because no values
    /// are written to storage until after the transaction is complete.
    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        Some(self.storage(address, index))
    }
}

impl<'env, J: IO + Copy, E: Env> ApplyBackend for ContractState<'env, J, E> {
    fn apply<A, I, L>(&mut self, values: A, _logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
        let mut writes_counter: usize = 0;

        let mut code_bytes_written: usize = 0;
        let mut accounting = accounting::Accounting::default();
        for apply in values {
            match apply {
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage,
                } => {
                    let current_basic = self.basic(address);
                    accounting.change(accounting::Change {
                        new_value: basic.balance,
                        old_value: current_basic.balance,
                    });

                    let address = Address::new(address);
                    let generation = get_generation(&self.io, &address);

                    if current_basic.nonce != basic.nonce {
                        set_nonce(&mut self.io, &address, &basic.nonce);
                        writes_counter += 1;
                    }
                    if current_basic.balance != basic.balance {
                        set_balance(&mut self.io, &address, &Wei::new(basic.balance));
                        writes_counter += 1;
                    }

                    if let Some(code) = code {
                        set_code(&mut self.io, &address, &code);
                        code_bytes_written = code.len();
                        aurora_engine_sdk::log!(
                            "code_write_at_address {:?} {}",
                            address,
                            code_bytes_written
                        );
                    }

                    let next_generation = if reset_storage {
                        remove_all_storage(&mut self.io, &address, generation);
                        generation + 1
                    } else {
                        generation
                    };

                    for (index, value) in storage {
                        if value == H256::default() {
                            remove_storage(&mut self.io, &address, &index, next_generation);
                        } else {
                            set_storage(&mut self.io, &address, &index, &value, next_generation);
                        }
                        writes_counter += 1;
                    }

                    // We only need to remove the account if:
                    // 1. we are supposed to delete an empty account
                    // 2. the account is empty
                    // 3. we didn't already clear out the storage (because if we did then there is
                    //    nothing to do)
                    if delete_empty
                        && is_account_empty(&self.io, &address)
                        && generation == next_generation
                    {
                        remove_account(&mut self.io, &address, generation);
                        writes_counter += 1;
                    }
                }
                Apply::Delete { address } => {
                    let current_basic = self.basic(address);
                    accounting.remove(current_basic.balance);

                    let address = Address::new(address);
                    let generation = get_generation(&self.io, &address);
                    remove_account(&mut self.io, &address, generation);
                    writes_counter += 1;
                }
            }
        }
        match accounting.net() {
            // Net loss is possible if `SELFDESTRUCT(self)` calls are made.
            accounting::Net::Lost(amount) => {
                let _ = amount;
                aurora_engine_sdk::log!("Burn {} ETH due to SELFDESTRUCT", amount);
                // Apply changes for eth-connector. We ignore the `StorageReadError` intentionally since
                // if we cannot read the storage then there is nothing to remove.
                if let Some(remove_eth) = self.remove_eth_fn.take() {
                    remove_eth(Wei::new(amount));
                }
            }
            accounting::Net::Zero => (),
            accounting::Net::Gained(_) => {
                // It should be impossible to gain ETH using normal EVM operations in production.
                // In tests, we have convenience functions that can poof addresses with ETH out of nowhere.
                #[cfg(all(not(feature = "integration-test"), feature = "contract"))]
                {
                    panic!("ERR_INVALID_ETH_SUPPLY_INCREASE");
                }
            }
        }
        // These variable are only used if logging feature is enabled.
        // In production logging is always enabled, so we can ignore the warnings.
        #[allow(unused_variables)]
        let total_bytes = 32 * writes_counter + code_bytes_written;
        #[allow(unused_assignments)]
        if code_bytes_written > 0 {
            writes_counter += 1;
        }
        aurora_engine_sdk::log!(
            "total_writes_count {}\ntotal_written_bytes {}",
            writes_counter,
            total_bytes
        );
    }
}

impl From<Log> for crate::Log {
    fn from(value: Log) -> Self {
        Self {
            address: value.address,
            topics: value.topics,
            data: value.data,
        }
    }
}

impl From<Config> for crate::Config {
    fn from(value: Config) -> Self {
        Self {
            gas_ext_code: value.gas_ext_code,
            gas_ext_code_hash: value.gas_ext_code_hash,
            gas_sstore_set: value.gas_sstore_set,
            gas_sstore_reset: value.gas_sstore_reset,
            refund_sstore_clears: value.refund_sstore_clears,
            max_refund_quotient: value.max_refund_quotient,
            gas_balance: value.gas_balance,
            gas_sload: value.gas_sload,
            gas_sload_cold: value.gas_sload_cold,
            gas_suicide: value.gas_suicide,

            gas_suicide_new_account: value.gas_suicide_new_account,
            gas_call: value.gas_call,
            gas_expbyte: value.gas_expbyte,
            gas_transaction_create: value.gas_transaction_create,
            gas_transaction_call: value.gas_transaction_call,
            gas_transaction_zero_data: value.gas_transaction_zero_data,
            gas_transaction_non_zero_data: value.gas_transaction_non_zero_data,
            gas_access_list_address: value.gas_access_list_address,
            gas_access_list_storage_key: value.gas_access_list_storage_key,
            gas_account_access_cold: value.gas_account_access_cold,
            gas_storage_read_warm: value.gas_storage_read_warm,
            sstore_gas_metering: value.sstore_gas_metering,
            sstore_revert_under_stipend: value.sstore_revert_under_stipend,
            increase_state_access_gas: value.increase_state_access_gas,
            decrease_clears_refund: value.decrease_clears_refund,
            disallow_executable_format: value.disallow_executable_format,
            warm_coinbase_address: value.warm_coinbase_address,
            err_on_call_with_more_gas: value.err_on_call_with_more_gas,
            call_l64_after_gas: value.call_l64_after_gas,
            empty_considered_exists: value.empty_considered_exists,
            create_increase_nonce: value.create_increase_nonce,
            stack_limit: value.stack_limit,
            memory_limit: value.memory_limit,
            call_stack_limit: value.call_stack_limit,
            create_contract_limit: value.create_contract_limit,
            max_initcode_size: value.max_initcode_size,
            call_stipend: value.call_stipend,
            has_delegate_call: value.has_delegate_call,
            has_create2: value.has_create2,
            has_revert: value.has_revert,
            has_return_data: value.has_return_data,
            has_bitwise_shifting: value.has_bitwise_shifting,
            has_chain_id: value.has_chain_id,
            has_self_balance: value.has_self_balance,
            has_ext_code_hash: value.has_ext_code_hash,
            has_base_fee: value.has_base_fee,
            has_push0: value.has_push0,
            estimate: value.estimate,
        }
    }
}
