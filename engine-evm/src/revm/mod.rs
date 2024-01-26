use crate::Box;
use crate::{EVMHandler, TransactionInfo};
use aurora_engine_sdk::io::IO;
use revm::handler::LoadPrecompilesHandle;
use revm::precompile::{Address, B256};
use revm::primitives::{
    Account, AccountInfo, Bytecode, Env, HashMap, ResultAndState, SpecId, U256,
};
use revm::{Database, DatabaseCommit, Evm};

pub const EVM_FORK: SpecId = SpecId::LATEST;

/// REVM handler
pub struct REVMHandler<'env, I: IO, E: aurora_engine_sdk::env::Env> {
    env_state: &'env E,
    state: ContractState<I>,
    env: Box<Env>,
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> REVMHandler<'env, I, E> {
    pub fn new(io: &I, env_state: &'env E, transaction: &TransactionInfo) -> Self {
        let state = ContractState::new(io);
        let env = Box::new(Env::default());
        Self {
            state,
            env_state,
            env,
        }

        /*
        // env.cfg.chain_id +
        // env.block.number +
        // env.block.coinbase +
        // env.block.difficulty  +
        // env.block.timestamp +
        // env.block.gas_limit +
        // env.block.basefee +
        // env.tx.transact_to +
        // env.tx.caller +
        // env.tx.gas_price +
        // env.tx.gas_priority_fee
        // env.tx.gas_limit +
        // env.tx.data +
        // env.tx.transact_to + -> for Deploy it's value from CREATE
        // env.tx.value +
        // env.tx.nonce
        // env.tx.access_list +

        // TRANSACT_CREATE
        // caller: H160,
        // value: U256,
        // init_code: Vec<u8>,
        // gas_limit: u64,
        // access_list: Vec<(H160, Vec<H256>)>,

        // TRANSACT_CALL
        // caller: H160,
        // address: H160,
        // value: U256,
        // data: Vec<u8>,
        // gas_limit: u64,
        // access_list: Vec<(H160, Vec<H256>)>,


        impl<'env, I: IO + Copy, E: Env, M: ModExpAlgorithm> Backend for Engine<'env, I, E, M> {
            /// Returns the "effective" gas price (as defined by EIP-1559)
            fn gas_price(&self) -> U256 {
                self.gas_price
            }

            /// Returns the origin address that created the contract.
            fn origin(&self) -> H160 {
                self.origin.raw()
            }


            fn block_hash(&self, number: U256) -> H256 {
                let idx = U256::from(self.env.block_height());
                if idx.saturating_sub(U256::from(256)) <= number && number < idx {
                    // since `idx` comes from `u64` it is always safe to downcast `number` from `U256`
                    compute_block_hash(
                        self.state.chain_id,
                        number.low_u64(),
                        self.current_account_id.as_bytes(),
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
                U256::from(self.state.chain_id)
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
        */
    }

    /// EVM precompiles
    pub fn set_precompiles<'a>(
        precompiles: &LoadPrecompilesHandle<'a>,
    ) -> LoadPrecompilesHandle<'a> {
        // TODO: extend precompiles
        // let c = precompiles();
        // Arc::new(move || c.clone())
        precompiles.clone()
    }
}

/// REVM contract state handler
/// Operates with REVM `DB`
pub struct ContractState<I: IO> {
    io: I,
}

impl<I: IO + Copy> ContractState<I> {
    pub fn new(io: &I) -> Self {
        Self { io: *io }
    }
}

impl<I: IO> Database for ContractState<I> {
    type Error = ();

    fn basic(&mut self, _address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        todo!()
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        todo!()
    }

    fn storage(&mut self, _address: Address, _index: U256) -> Result<U256, Self::Error> {
        todo!()
    }

    fn block_hash(&mut self, _number: U256) -> Result<B256, Self::Error> {
        todo!()
    }
}

impl<I: IO> DatabaseCommit for ContractState<I> {
    fn commit(&mut self, _evm_state: HashMap<Address, Account>) {
        todo!()
    }
}

impl<'env, I: IO, E: aurora_engine_sdk::env::Env> EVMHandler for REVMHandler<'env, I, E> {
    fn transact_create(&mut self) {
        let mut evm = Evm::builder()
            .with_db(&mut self.state)
            .modify_env(|e| *e = *self.env.clone())
            .spec_id(EVM_FORK)
            .build();
        // let precompiles = evm.handler.pre_execution.load_precompiles;
        // evm.handler.pre_execution.load_precompiles = Self::set_precompiles(&precompiles);
        // TODO: handle error and remove unwrap
        let ResultAndState { result, state } = evm.transact().unwrap();
        evm.context.evm.db.commit(state);
    }

    fn transact_create_fixed(&mut self) {
        let mut evm = Evm::builder()
            .with_db(&mut self.state)
            .modify_env(|e| *e = *self.env.clone())
            .spec_id(EVM_FORK)
            .build();
        // let precompiles = evm.handler.pre_execution.load_precompiles;
        // evm.handler.pre_execution.load_precompiles = Self::set_precompiles(&precompiles);
        // TODO: handle error and remove unwrap
        let ResultAndState { result, state } = evm.transact().unwrap();
        evm.context.evm.db.commit(state);
    }

    fn transact_call(&mut self) {
        let mut evm = Evm::builder()
            .with_db(&mut self.state)
            .modify_env(|e| *e = *self.env.clone())
            .spec_id(EVM_FORK)
            .build();
        // let precompiles = evm.handler.pre_execution.load_precompiles;
        // evm.handler.pre_execution.load_precompiles = Self::set_precompiles(&precompiles);
        // TODO: handle error and remove unwrap
        let ResultAndState { result, state } = evm.transact().unwrap();
        evm.context.evm.db.commit(state);
    }
}
