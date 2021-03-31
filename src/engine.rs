use borsh::{BorshDeserialize, BorshSerialize};
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::executor::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use evm::{Config, CreateScheme, ExitFatal, ExitReason};

use crate::parameters::{FunctionCallArgs, NewCallArgs, ViewCallArgs};
use crate::precompiles;
use crate::prelude::{Address, Vec, H256, U256};
use crate::sdk;
use crate::storage::{address_to_key, storage_to_key, KeyPrefix};
use crate::types::{bytes_to_hex, log_to_bytes, u256_to_arr, AccountId};

/// Engine internal state, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct EngineState {
    /// Chain id, according to the EIP-115 / ethereum-lists spec.
    pub chain_id: [u8; 32],
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// Account of the bridge prover.
    /// Use empty to not use base token as bridged asset.
    pub bridge_prover_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        EngineState {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            bridge_prover_id: args.bridge_prover_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
        }
    }
}

pub struct Engine {
    state: EngineState,
    origin: Address,
}

// TODO: upgrade to Berlin HF
const CONFIG: &'static Config = &Config::istanbul();

/// Key for storing the state of the engine.
const STATE_KEY: &[u8; 6] = b"\0STATE";

impl Engine {
    pub fn new(origin: Address) -> Self {
        Self::new_with_state(Engine::get_state(), origin)
    }

    pub fn new_with_state(state: EngineState, origin: Address) -> Self {
        Self { state, origin }
    }

    /// Saves state into the storage.
    pub fn set_state(state: EngineState) {
        sdk::write_storage(STATE_KEY, &state.try_to_vec().expect("ERR_SER"));
    }

    /// Fails if state is not found.
    pub fn get_state() -> EngineState {
        match sdk::read_storage(STATE_KEY) {
            None => Default::default(),
            Some(bytes) => EngineState::try_from_slice(&bytes).expect("ERR_DESER"),
        }
    }

    pub fn set_code(address: &Address, code: &[u8]) {
        sdk::write_storage(&address_to_key(KeyPrefix::Code, address), code);
    }

    pub fn remove_code(address: &Address) {
        sdk::remove_storage(&address_to_key(KeyPrefix::Code, address))
    }

    pub fn get_code(address: &Address) -> Vec<u8> {
        sdk::read_storage(&address_to_key(KeyPrefix::Code, address)).unwrap_or_else(Vec::new)
    }

    pub fn get_code_size(address: &Address) -> usize {
        Engine::get_code(&address).len()
    }

    pub fn set_nonce(address: &Address, nonce: &U256) {
        sdk::write_storage(
            &address_to_key(KeyPrefix::Nonce, address),
            &u256_to_arr(nonce),
        );
    }

    pub fn remove_nonce(address: &Address) {
        sdk::remove_storage(&address_to_key(KeyPrefix::Nonce, address))
    }

    pub fn get_nonce(address: &Address) -> U256 {
        sdk::read_storage(&address_to_key(KeyPrefix::Nonce, address))
            .map(|value| U256::from_big_endian(&value))
            .unwrap_or_else(U256::zero)
    }

    pub fn set_balance(address: &Address, balance: &U256) {
        sdk::write_storage(
            &address_to_key(KeyPrefix::Balance, address),
            &u256_to_arr(balance),
        );
    }

    pub fn remove_balance(address: &Address) {
        sdk::remove_storage(&address_to_key(KeyPrefix::Balance, address))
    }

    pub fn get_balance(address: &Address) -> U256 {
        sdk::read_storage(&address_to_key(KeyPrefix::Balance, address))
            .map(|value| U256::from_big_endian(&value))
            .unwrap_or_else(U256::zero)
    }

    pub fn remove_storage(address: &Address, key: &H256) {
        sdk::remove_storage(&storage_to_key(address, key));
    }

    pub fn set_storage(address: &Address, key: &H256, value: &H256) {
        sdk::write_storage(&storage_to_key(address, key), &value.0);
    }

    pub fn get_storage(address: &Address, key: &H256) -> H256 {
        sdk::read_storage(&storage_to_key(address, key))
            .map(|value| H256::from_slice(&value))
            .unwrap_or_else(H256::default)
    }

    pub fn is_account_empty(address: &Address) -> bool {
        let balance = Self::get_balance(address);
        let nonce = Self::get_nonce(address);
        let code_len = Self::get_code_size(address);
        balance == U256::zero() && nonce == U256::zero() && code_len == 0
    }

    /// Removes all storage for the given address.
    pub fn remove_all_storage(_address: &Address) {
        // FIXME: there is presently no way to prefix delete trie state.
    }

    /// Removes an account.
    pub fn remove_account(address: &Address) {
        Self::remove_nonce(address);
        Self::remove_balance(address);
        Self::remove_code(address);
        Self::remove_all_storage(address);
    }

    /// Removes an account if it is empty.
    pub fn remove_account_if_empty(address: &Address) {
        if Self::is_account_empty(address) {
            Self::remove_account(address);
        }
    }

    pub fn transfer(&mut self, _sender: Address, _receiver: Address, _value: U256) -> ExitReason {
        ExitReason::Fatal(ExitFatal::NotSupported) // TODO: implement balance transfers
    }

    pub fn deploy_code_with_input(&mut self, input: &[u8]) -> (ExitReason, Address) {
        let origin = self.origin();
        let value = U256::zero();
        self.deploy_code(origin, value, input)
    }

    pub fn deploy_code(
        &mut self,
        origin: Address,
        value: U256,
        input: &[u8],
    ) -> (ExitReason, Address) {
        let mut executor = self.make_executor();
        let address = executor.create_address(CreateScheme::Legacy { caller: origin });
        let (status, result) = (
            executor.transact_create(origin, value, Vec::from(input), u64::max_value()),
            address,
        );
        let (values, logs) = executor.into_state().deconstruct();
        self.apply(values, logs, true);
        (status, result)
    }

    pub fn call_with_args(&mut self, args: FunctionCallArgs) -> (ExitReason, Vec<u8>) {
        let origin = self.origin();
        let contract = Address(args.contract);
        let value = U256::zero();
        self.call(origin, contract, value, args.input)
    }

    pub fn call(
        &mut self,
        origin: Address,
        contract: Address,
        value: U256,
        input: Vec<u8>,
    ) -> (ExitReason, Vec<u8>) {
        let mut executor = self.make_executor();
        let (status, result) =
            executor.transact_call(origin, contract, value, input, u64::max_value());
        let (values, logs) = executor.into_state().deconstruct();
        self.apply(values, logs, true);
        (status, result)
    }

    pub fn view_with_args(&self, args: ViewCallArgs) -> (ExitReason, Vec<u8>) {
        let origin = Address::from_slice(&args.sender);
        let contract = Address::from_slice(&args.address);
        let value = U256::from_big_endian(&args.amount);
        self.view(origin, contract, value, args.input)
    }

    pub fn view(
        &self,
        origin: Address,
        contract: Address,
        value: U256,
        input: Vec<u8>,
    ) -> (ExitReason, Vec<u8>) {
        let mut executor = self.make_executor();
        executor.transact_call(origin, contract, value, input, u64::max_value())
    }

    fn make_executor(&self) -> StackExecutor<MemoryStackState<Engine>> {
        let metadata = StackSubstateMetadata::new(u64::max_value(), &CONFIG);
        let state = MemoryStackState::new(metadata, self);
        StackExecutor::new_with_precompile(state, &CONFIG, precompiles::istanbul_precompiles)
    }
}

impl evm::backend::Backend for Engine {
    fn gas_price(&self) -> U256 {
        U256::zero()
    }

    fn origin(&self) -> Address {
        self.origin
    }

    fn block_hash(&self, _number: U256) -> H256 {
        H256::zero() // TODO: https://github.com/near/nearcore/issues/3456
    }

    fn block_number(&self) -> U256 {
        U256::from(sdk::block_index())
    }

    fn block_coinbase(&self) -> Address {
        Address::zero()
    }

    fn block_timestamp(&self) -> U256 {
        U256::from(sdk::block_timestamp())
    }

    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    fn block_gas_limit(&self) -> U256 {
        U256::zero() // TODO
    }

    fn chain_id(&self) -> U256 {
        U256::from(self.state.chain_id)
    }

    fn exists(&self, address: Address) -> bool {
        !Engine::is_account_empty(&address)
    }

    fn basic(&self, address: Address) -> Basic {
        Basic {
            nonce: Engine::get_nonce(&address),
            balance: Engine::get_balance(&address),
        }
    }

    fn code(&self, address: Address) -> Vec<u8> {
        Engine::get_code(&address)
    }

    fn storage(&self, address: Address, index: H256) -> H256 {
        Engine::get_storage(&address, &index)
    }

    fn original_storage(&self, _address: Address, _index: H256) -> Option<H256> {
        None
    }
}

impl ApplyBackend for Engine {
    fn apply<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
        for apply in values {
            match apply {
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage,
                } => {
                    Engine::set_nonce(&address, &basic.nonce);
                    Engine::set_balance(&address, &basic.balance);
                    if let Some(code) = code {
                        Engine::set_code(&address, &code)
                    }

                    if reset_storage {
                        Engine::remove_all_storage(&address)
                    }

                    for (index, value) in storage {
                        if value == H256::default() {
                            Engine::remove_storage(&address, &index)
                        } else {
                            Engine::set_storage(&address, &index, &value)
                        }
                    }

                    if delete_empty {
                        Engine::remove_account_if_empty(&address)
                    }
                }
                Apply::Delete { address } => Engine::remove_account(&address),
            }
        }

        for log in logs {
            sdk::log_utf8(&bytes_to_hex(&log_to_bytes(log)).into_bytes())
        }
    }
}

#[cfg(test)]
mod tests {}
