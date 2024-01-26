use crate::Box;
use crate::{EVMHandler, TransactionInfo};
use aurora_engine_types::types::{NEP141Wei, Wei};
use aurora_engine_types::H160;
use revm::precompile::{Address, B256};
use revm::primitives::{
    Account, AccountInfo, Bytecode, Env, HashMap, ResultAndState, SpecId, U256,
};
use revm::{Database, DatabaseCommit, Evm};

pub const EVM_FORK: SpecId = SpecId::LATEST;

/// REVM handler
pub struct REVMHandler {
    state: ContractState,
    env: Box<Env>,
}

impl REVMHandler {
    pub fn new(transaction: &TransactionInfo) -> Self {
        let state = ContractState::new();
        let mut env = Box::new(Env::default());

        // env.cfg.chain_id = self.chain_id;
        env.block.gas_limit = U256::MAX;
        // env.block.number = U256::from((transaction.block_height)());
        // env.block.coinbase = Address::new((transaction.coinbase)());
        // env.block.timestamp = U256::from((transaction.time_stamp)());
        // (transaction.set_balance_handler)(address, balance);
        env.block.difficulty = U256::ZERO;
        env.block.basefee = U256::ZERO;
        // For callback test
        let balance = Box::new(Wei::from(NEP141Wei::new(1)));
        let address = Box::new(aurora_engine_types::types::Address::new(
            H160::from_low_u64_be(0),
        ));

        Self { state, env }

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

    // EVM precompiles
    // pub fn set_precompiles<'a>(
    //     precompiles: &LoadPrecompilesHandle<'a>,
    // ) -> LoadPrecompilesHandle<'a> {
    //     // TODO: extend precompiles
    //     let c = precompiles();
    //     Arc::new(move || c.clone())
    // }
}

/// REVM contract state handler
/// Operates with REVM `DB`
pub struct ContractState;

impl ContractState {
    pub fn new() -> Self {
        Self
    }
}

impl Database for ContractState {
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

impl DatabaseCommit for ContractState {
    fn commit(&mut self, _evm_state: HashMap<Address, Account>) {
        todo!()
    }
}

impl EVMHandler for REVMHandler {
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
