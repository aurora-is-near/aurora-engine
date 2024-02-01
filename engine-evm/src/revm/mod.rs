use crate::{BlockInfo, EVMHandler, TransactionInfo};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::types::{NEP141Wei, Wei};
use aurora_engine_types::{Box, H160};
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
    state: ContractState<'env, I, E>,
    env: Box<Env>,
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> REVMHandler<'env, I, E> {
    pub fn new(
        io: I,
        env_state: &'env E,
        transaction: &'env TransactionInfo,
        _block: &'env BlockInfo,
    ) -> Self {
        let state = ContractState::new(io, env_state);
        let mut env = Box::new(Env::default());

        // env.cfg.chain_id = self.chain_id;
        // Set Block data
        env.block.gas_limit = U256::MAX;
        env.block.number = U256::from(env_state.block_height());
        env.block.coinbase = Address::new([
            0x44, 0x44, 0x58, 0x84, 0x43, 0xC3, 0xa9, 0x12, 0x88, 0xc5, 0x00, 0x24, 0x83, 0x44,
            0x9A, 0xba, 0x10, 0x54, 0x19, 0x2b,
        ]);
        env.block.timestamp = U256::from(env_state.block_timestamp().secs());
        env.block.difficulty = U256::ZERO;
        env.block.basefee = U256::ZERO;
        // For callback test
        let balance = Box::new(Wei::from(NEP141Wei::new(1)));
        let address = Box::new(aurora_engine_types::types::Address::new(
            H160::from_low_u64_be(0),
        ));

        Self {
            state,
            env_state,
            env,
        }

        /* TODO: remove - for investigation only
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
pub struct ContractState<'env, I: IO, E: aurora_engine_sdk::env::Env> {
    io: I,
    env_state: &'env E,
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> ContractState<'env, I, E> {
    pub fn new(io: I, env_state: &'env E) -> Self {
        Self { io, env_state }
    }
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> Database for ContractState<'env, I, E> {
    type Error = ();

    fn basic(&mut self, _address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        todo!()
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        todo!()
    }

    fn storage(&mut self, _address: Address, _index: U256) -> Result<U256, Self::Error> {
        todo!()
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        let idx = U256::from(self.env_state.block_height());
        if idx.saturating_sub(U256::from(256)) <= number && number < idx {
            // TODO: refactor
            // compute_block_hash(
            //     self.state.chain_id,
            //     number.low_u64(),
            //     self.current_account_id.as_bytes(),
            // )
            Ok(B256::ZERO)
        } else {
            Ok(B256::ZERO)
        }
    }
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> DatabaseCommit
    for ContractState<'env, I, E>
{
    fn commit(&mut self, _evm_state: HashMap<Address, Account>) {
        todo!()
    }
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> EVMHandler for REVMHandler<'env, I, E> {
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
