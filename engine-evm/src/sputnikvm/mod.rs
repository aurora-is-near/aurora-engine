use crate::{BlockInfo, EVMHandler, TransactResult, TransactionInfo};

use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::caching::FullCache;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
use aurora_engine_types::storage::{address_to_key, storage_to_key, KeyPrefix};
use aurora_engine_types::types::{u256_to_arr, Address, Wei};
use aurora_engine_types::Box;
use aurora_engine_types::{BTreeMap, Vec, H160, H256, U256};
use core::cell::RefCell;
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::{executor, Config, ExitError, ExitReason};

mod accounting;

const CONFIG: &Config = &Config::shanghai();

/// SputnikVM handler
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
    fn transact_create(&mut self) {
        todo!()
    }

    fn transact_create_fixed(&mut self) {
        todo!()
    }

    fn transact_call(&mut self) -> TransactResult {
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
            // TODO: check it
            self.transaction.address.unwrap(),
            self.transaction.value.raw(),
            self.transaction.input.clone(),
            self.transaction.gas_limit,
            self.transaction.access_list.clone(),
        );
        let used_gas = executor.used_gas();
        let (values, logs) = executor.into_state().deconstruct();
        contract_state.apply(values, Vec::<Log>::new(), true);
        let status = exit_reason_into_result(exit_reason, result).unwrap();
        TransactResult {
            submit_result: SubmitResult::new(status, used_gas, Vec::new()),
            logs: logs.into_iter().collect(),
            remove_eth: contract_state.get_remove_eth(),
        }
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
    remove_eth: Option<U256>,
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
            remove_eth: None,
            remove_eth_fn,
        }
    }

    pub fn get_remove_eth(&self) -> Option<U256> {
        self.remove_eth
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

const BLOCK_HASH_PREFIX: u8 = 0;
const BLOCK_HASH_PREFIX_SIZE: usize = 1;
const BLOCK_HEIGHT_SIZE: usize = 8;
const CHAIN_ID_SIZE: usize = 32;

/// There is one Aurora block per NEAR block height (note: when heights in NEAR are skipped
/// they are interpreted as empty blocks on Aurora). The blockhash is derived from the height
/// according to
/// ```text
/// block_hash = sha256(concat(
///     BLOCK_HASH_PREFIX,
///     block_height as u64,
///     chain_id,
///     engine_account_id,
/// ))
/// ```
#[must_use]
pub fn compute_block_hash(chain_id: [u8; 32], block_height: u64, account_id: &[u8]) -> H256 {
    debug_assert_eq!(
        BLOCK_HASH_PREFIX_SIZE,
        core::mem::size_of_val(&BLOCK_HASH_PREFIX)
    );
    debug_assert_eq!(BLOCK_HEIGHT_SIZE, core::mem::size_of_val(&block_height));
    debug_assert_eq!(CHAIN_ID_SIZE, core::mem::size_of_val(&chain_id));
    let mut data = Vec::with_capacity(
        BLOCK_HASH_PREFIX_SIZE + BLOCK_HEIGHT_SIZE + CHAIN_ID_SIZE + account_id.len(),
    );
    data.push(BLOCK_HASH_PREFIX);
    data.extend_from_slice(&chain_id);
    data.extend_from_slice(account_id);
    data.extend_from_slice(&block_height.to_be_bytes());

    aurora_engine_sdk::sha256(&data)
}

fn get_generation<I: IO>(io: &I, address: &Address) -> u32 {
    io.read_storage(&address_to_key(KeyPrefix::Generation, address))
        .map_or(0, |value| {
            let mut bytes = [0u8; 4];
            value.copy_to_slice(&mut bytes);
            u32::from_be_bytes(bytes)
        })
}

fn get_storage<I: IO>(io: &I, address: &Address, key: &H256, generation: u32) -> H256 {
    io.read_storage(storage_to_key(address, key, generation).as_ref())
        .and_then(|value| {
            if value.len() == 32 {
                let mut buf = [0u8; 32];
                value.copy_to_slice(&mut buf);
                Some(H256(buf))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn get_code<I: IO>(io: &I, address: &Address) -> Vec<u8> {
    io.read_storage(&address_to_key(KeyPrefix::Code, address))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

fn get_balance<I: IO>(io: &I, address: &Address) -> Wei {
    let raw = io
        .read_u256(&address_to_key(KeyPrefix::Balance, address))
        .unwrap_or_else(|_| U256::zero());
    Wei::new(raw)
}

fn get_nonce<I: IO>(io: &I, address: &Address) -> U256 {
    io.read_u256(&address_to_key(KeyPrefix::Nonce, address))
        .unwrap_or_else(|_| U256::zero())
}

fn is_account_empty<I: IO>(io: &I, address: &Address) -> bool {
    get_balance(io, address).is_zero()
        && get_nonce(io, address).is_zero()
        && get_code_size(io, address) == 0
}

fn get_code_size<I: IO>(io: &I, address: &Address) -> usize {
    io.read_storage_len(&address_to_key(KeyPrefix::Code, address))
        .unwrap_or(0)
}

fn set_nonce<I: IO>(io: &mut I, address: &Address, nonce: &U256) {
    io.write_storage(
        &address_to_key(KeyPrefix::Nonce, address),
        &u256_to_arr(nonce),
    );
}

/// Removes an account.
fn remove_account<I: IO + Copy>(io: &mut I, address: &Address, generation: u32) {
    remove_nonce(io, address);
    remove_balance(io, address);
    remove_code(io, address);
    remove_all_storage(io, address, generation);
}

fn remove_nonce<I: IO>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Nonce, address));
}

fn remove_balance<I: IO + Copy>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Balance, address));
}

fn remove_code<I: IO>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Code, address));
}

/// Removes all storage for the given address.
fn remove_all_storage<I: IO>(io: &mut I, address: &Address, generation: u32) {
    // FIXME: there is presently no way to prefix delete trie state.
    // NOTE: There is not going to be a method on runtime for this.
    //     You may need to store all keys in a list if you want to do this in a contract.
    //     Maybe you can incentivize people to delete dead old keys. They can observe them from
    //     external indexer node and then issue special cleaning transaction.
    //     Either way you may have to store the nonce per storage address root. When the account
    //     has to be deleted the storage nonce needs to be increased, and the old nonce keys
    //     can be deleted over time. That's how TurboGeth does storage.
    set_generation(io, address, generation + 1);
}

/// Increments storage generation for a given address.
fn set_generation<I: IO>(io: &mut I, address: &Address, generation: u32) {
    io.write_storage(
        &address_to_key(KeyPrefix::Generation, address),
        &generation.to_be_bytes(),
    );
}

fn remove_storage<I: IO>(io: &mut I, address: &Address, key: &H256, generation: u32) {
    io.remove_storage(storage_to_key(address, key, generation).as_ref());
}

fn set_storage<I: IO>(io: &mut I, address: &Address, key: &H256, value: &H256, generation: u32) {
    io.write_storage(storage_to_key(address, key, generation).as_ref(), &value.0);
}

fn set_balance<I: IO>(io: &mut I, address: &Address, balance: &Wei) {
    io.write_storage(
        &address_to_key(KeyPrefix::Balance, address),
        &balance.to_bytes(),
    );
}

fn set_code<I: IO>(io: &mut I, address: &Address, code: &[u8]) {
    io.write_storage(&address_to_key(KeyPrefix::Code, address), code);
}

fn exit_reason_into_result(
    exit_reason: ExitReason,
    data: Vec<u8>,
) -> Result<TransactionStatus, ExitReason> {
    match exit_reason {
        ExitReason::Succeed(_) => Ok(TransactionStatus::Succeed(data)),
        ExitReason::Revert(_) => Ok(TransactionStatus::Revert(data)),
        ExitReason::Error(ExitError::OutOfOffset) => Ok(TransactionStatus::OutOfOffset),
        ExitReason::Error(ExitError::OutOfFund) => Ok(TransactionStatus::OutOfFund),
        ExitReason::Error(ExitError::OutOfGas) => Ok(TransactionStatus::OutOfGas),
        _ => Err(exit_reason),
    }
}
