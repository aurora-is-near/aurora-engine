use crate::{EVMHandler, EngineEVM, TransactionInfo};
use aurora_engine_precompiles::Precompiles;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
use aurora_engine_types::types::Address;
use aurora_engine_types::{Vec, H160, H256, U256};
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::{executor, Config};

/// SputnikVM handler
pub struct SputnikVMHandler<'env, I: IO, E: Env, H> {
    env_state: &'env E,
    state: ContractState<'env, I, E>,
    precompiles: Precompiles<'env, I, E, H>,
}

/// Init REVM
pub fn init_evm<'tx, 'env, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler>(
    io: &I,
    env: &'env E,
    transaction: &'tx TransactionInfo,
    precompiles: Precompiles<'env, I, E, H>,
    config: &'env Config,
) -> EngineEVM<'tx, 'env, I, E, SputnikVMHandler<'env, I, E, H>> {
    let handler = SputnikVMHandler::new(io, env, &transaction, precompiles, config);
    EngineEVM::new(io, env, transaction, handler)
}

pub struct ContractState<'env, I: IO, E: Env> {
    io: I,
    env_state: &'env E,
}

impl<'env, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> SputnikVMHandler<'env, I, E, H> {
    pub fn new(
        io: &I,
        env_state: &'env E,
        transaction: &TransactionInfo,
        precompiles: Precompiles<'env, I, E, H>,
        config: &'env Config,
    ) -> Self {
        let _metadata = executor::stack::StackSubstateMetadata::new(transaction.gas_limit, config);
        todo!()
    }
}

impl<'env, I: IO + Copy, E: Env, H> EVMHandler for SputnikVMHandler<'env, I, E, H> {
    fn transact_create(&mut self) {
        todo!()
    }

    fn transact_create_fixed(&mut self) {
        todo!()
    }

    fn transact_call(&mut self) {
        todo!()
    }
}

impl<'env, I: IO, E: Env> Backend for ContractState<'env, I, E> {
    /// Returns the "effective" gas price (as defined by EIP-1559)
    fn gas_price(&self) -> U256 {
        // self.gas_price
        todo!()
    }

    /// Returns the origin address that created the contract.
    fn origin(&self) -> H160 {
        // self.origin.raw()
        todo!()
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
        todo!()
        // let idx = U256::from(self.env.block_height());
        // if idx.saturating_sub(U256::from(256)) <= number && number < idx {
        //     // since `idx` comes from `u64` it is always safe to downcast `number` from `U256`
        //     compute_block_hash(
        //         self.state.chain_id,
        //         number.low_u64(),
        //         self.current_account_id.as_bytes(),
        //     )
        // } else {
        //     H256::zero()
        // }
    }

    /// Returns the current block index number.
    fn block_number(&self) -> U256 {
        // U256::from(self.env.block_height())
        todo!()
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
        // U256::from(self.env.block_timestamp().secs())
        todo!()
    }

    /// Returns the current block difficulty.
    ///
    /// See: `https://doc.aurora.dev/develop/compat/evm#difficulty`
    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    /// Get environmental block randomness.
    fn block_randomness(&self) -> Option<H256> {
        // Some(self.env.random_seed())
        todo!()
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
        // U256::from(self.state.chain_id)
        todo!()
    }

    /// Checks if an address exists.
    fn exists(&self, address: H160) -> bool {
        // let address = Address::new(address);
        // let mut cache = self.account_info_cache.borrow_mut();
        // let basic_info = cache.get_or_insert_with(address, || Basic {
        //     nonce: get_nonce(&self.io, &address),
        //     balance: get_balance(&self.io, &address).raw(),
        // });
        // if !basic_info.balance.is_zero() || !basic_info.nonce.is_zero() {
        //     return true;
        // }
        // let mut cache = self.contract_code_cache.borrow_mut();
        // let code = cache.get_or_insert_with(address, || get_code(&self.io, &address));
        // !code.is_empty()
        todo!()
    }

    /// Returns basic account information.
    fn basic(&self, address: H160) -> Basic {
        // let address = Address::new(address);
        // let result = self
        //     .account_info_cache
        //     .borrow_mut()
        //     .get_or_insert_with(address, || Basic {
        //         nonce: get_nonce(&self.io, &address),
        //         balance: get_balance(&self.io, &address).raw(),
        //     })
        //     .clone();
        // result
        todo!()
    }

    /// Returns the code of the contract from an address.
    fn code(&self, address: H160) -> Vec<u8> {
        let address = Address::new(address);
        // self.contract_code_cache
        //     .borrow_mut()
        //     .get_or_insert_with(address, || get_code(&self.io, &address))
        //     .clone()
        todo!()
    }

    /// Get storage value of address at index.
    fn storage(&self, address: H160, index: H256) -> H256 {
        // let address = Address::new(address);
        // let generation = *self
        //     .generation_cache
        //     .borrow_mut()
        //     .entry(address)
        //     .or_insert_with(|| get_generation(&self.io, &address));
        // let result = *self
        //     .contract_storage_cache
        //     .borrow_mut()
        //     .get_or_insert_with((address, index), || {
        //         get_storage(&self.io, &address, &index, generation)
        //     });
        // result
        todo!()
    }

    /// Get original storage value of address at index, if available.
    ///
    /// Since `SputnikVM` collects storage changes in memory until the transaction is over,
    /// the "original storage" will always be the same as the storage because no values
    /// are written to storage until after the transaction is complete.
    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        //Some(self.storage(address, index))
        todo!()
    }
}

impl<'env, J: IO, E: Env> ApplyBackend for ContractState<'env, J, E> {
    fn apply<A, I, L>(&mut self, values: A, _logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
        todo!()
        /*
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
                        sdk::log!("code_write_at_address {:?} {}", address, code_bytes_written);
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
                sdk::log!("Burn {} ETH due to SELFDESTRUCT", amount);
                // Apply changes for eth-connector. We ignore the `StorageReadError` intentionally since
                // if we cannot read the storage then there is nothing to remove.
                #[cfg(not(feature = "ext-connector"))]
                connector::EthConnectorContract::init(self.io)
                    .map(|mut connector| {
                        // The `unwrap` is safe here because (a) if the connector
                        // is implemented correctly then the total supply will never underflow and (b) we are passing
                        // in the balance directly so there will always be enough balance.
                        connector.internal_remove_eth(Wei::new(amount)).unwrap();
                    })
                    .ok();
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
        sdk::log!(
            "total_writes_count {}\ntotal_written_bytes {}",
            writes_counter,
            total_bytes
        );
         */
    }
}
