use evm::backend::{Apply, Backend, Basic, Log};
use evm::executor::{MemoryStackState, StackState, StackSubstateMetadata};
use evm::{ExitError, Transfer};

use crate::engine::Engine;
use crate::parameters::PromiseCreateArgs;
use crate::prelude::{Vec, H160, H256, U256};
use crate::types::Stack;
use crate::AuroraState;

pub struct AuroraStackState<'backend, 'config> {
    memory_stack_state: MemoryStackState<'backend, 'config, Engine>,
    promises: Stack<PromiseCreateArgs>,
}

impl<'backend, 'config> AuroraStackState<'backend, 'config> {
    pub fn new(metadata: StackSubstateMetadata<'config>, backend: &'backend Engine) -> Self {
        Self {
            memory_stack_state: MemoryStackState::new(metadata, backend),
            promises: Stack::new(),
        }
    }

    #[must_use]
    pub fn deconstruct(
        self,
    ) -> (
        impl IntoIterator<Item = Apply<impl IntoIterator<Item = (H256, H256)>>>,
        impl IntoIterator<Item = Log>,
        impl IntoIterator<Item = PromiseCreateArgs>,
    ) {
        let (apply_iter, log_iter) = self.memory_stack_state.deconstruct();
        (apply_iter, log_iter, self.promises.into_vec())
    }
}

impl<'backend, 'config> AuroraState for AuroraStackState<'backend, 'config> {
    fn add_promise(&mut self, promise: PromiseCreateArgs) {
        self.promises.push(promise);
    }
}

impl<'backend, 'config> Backend for AuroraStackState<'backend, 'config> {
    fn gas_price(&self) -> U256 {
        self.memory_stack_state.gas_price()
    }

    fn origin(&self) -> H160 {
        self.memory_stack_state.origin()
    }

    fn block_hash(&self, number: U256) -> H256 {
        self.memory_stack_state.block_hash(number)
    }

    fn block_number(&self) -> U256 {
        self.memory_stack_state.block_number()
    }

    fn block_coinbase(&self) -> H160 {
        self.memory_stack_state.block_coinbase()
    }

    fn block_timestamp(&self) -> U256 {
        self.memory_stack_state.block_timestamp()
    }

    fn block_difficulty(&self) -> U256 {
        self.memory_stack_state.block_difficulty()
    }

    fn block_gas_limit(&self) -> U256 {
        self.memory_stack_state.block_gas_limit()
    }

    fn chain_id(&self) -> U256 {
        self.memory_stack_state.chain_id()
    }

    fn exists(&self, address: H160) -> bool {
        self.memory_stack_state.exists(address)
    }

    fn basic(&self, address: H160) -> Basic {
        self.memory_stack_state.basic(address)
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.memory_stack_state.code(address)
    }

    fn storage(&self, address: H160, index: H256) -> H256 {
        self.memory_stack_state.storage(address, index)
    }

    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        self.memory_stack_state.original_storage(address, index)
    }
}

impl<'backend, 'config> StackState<'config> for AuroraStackState<'backend, 'config> {
    fn metadata(&self) -> &StackSubstateMetadata<'config> {
        self.memory_stack_state.metadata()
    }

    fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
        self.memory_stack_state.metadata_mut()
    }

    fn enter(&mut self, gas_limit: u64, is_static: bool) {
        self.promises.enter();

        self.memory_stack_state.enter(gas_limit, is_static);
    }

    fn exit_commit(&mut self) -> Result<(), ExitError> {
        self.promises.commit();

        self.memory_stack_state.exit_commit()
    }

    fn exit_revert(&mut self) -> Result<(), ExitError> {
        self.promises.discard();

        self.memory_stack_state.exit_revert()
    }

    fn exit_discard(&mut self) -> Result<(), ExitError> {
        self.promises.discard();

        self.memory_stack_state.exit_discard()
    }

    fn is_empty(&self, address: H160) -> bool {
        self.memory_stack_state.is_empty(address)
    }

    fn deleted(&self, address: H160) -> bool {
        self.memory_stack_state.deleted(address)
    }

    fn inc_nonce(&mut self, address: H160) {
        self.memory_stack_state.inc_nonce(address)
    }

    fn set_storage(&mut self, address: H160, key: H256, value: H256) {
        self.memory_stack_state.set_storage(address, key, value)
    }

    fn reset_storage(&mut self, address: H160) {
        self.memory_stack_state.reset_storage(address)
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.memory_stack_state.log(address, topics, data)
    }

    fn set_deleted(&mut self, address: H160) {
        self.memory_stack_state.set_deleted(address)
    }

    fn set_code(&mut self, address: H160, code: Vec<u8>) {
        self.memory_stack_state.set_code(address, code)
    }

    fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
        self.memory_stack_state.transfer(transfer)
    }

    fn reset_balance(&mut self, address: H160) {
        self.memory_stack_state.reset_balance(address)
    }

    fn touch(&mut self, address: H160) {
        self.memory_stack_state.touch(address)
    }
}
