use revm::precompile::{Address, B256};
use revm::primitives::{Account, AccountInfo, Bytecode, HashMap, U256};
use revm::{Database, DatabaseCommit, Evm};

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

fn init() {
    let mut contract_state = ContractState::new();
    let mut evm = Evm::builder().with_db(&mut contract_state).build();
    let _res = evm.transact();
}
