use borsh::{BorshDeserialize, BorshSerialize};
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::executor::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use evm::{Config, CreateScheme, ExitError, ExitReason, ExitSucceed};

use crate::parameters::{FunctionCallArgs, NewCallArgs, ViewCallArgs};
use crate::precompiles;
use crate::prelude::{Address, Borrowed, Vec, H256, U256};
use crate::sdk;
use crate::storage::{address_to_key, storage_to_key, KeyPrefix};
use crate::types::{bytes_to_hex, log_to_bytes, u256_to_arr, AccountId, NonceError};

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
const CONFIG: &Config = &Config::istanbul();

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

    /// Checks the nonce for the address matches the transaction nonce, and if so
    /// returns the next nonce (if it exists). Note: this does not modify the actual
    /// nonce of the account in storage. The nonce still needs to be set to the new value
    /// if this is required.
    #[inline]
    pub fn check_nonce(address: &Address, transaction_nonce: &U256) -> Result<U256, NonceError> {
        let account_nonce = Self::get_nonce(address);

        if transaction_nonce != &account_nonce {
            return Err(NonceError::IncorrectNonce);
        }

        account_nonce
            .checked_add(U256::one())
            .ok_or(NonceError::NonceOverflow)
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

    /// Checks if the balance can be increased by an amount for a given address.
    ///
    /// Returns the new balance on success.
    ///
    /// # Errors
    ///
    /// * If the balance is > `U256::MAX`
    fn check_increase_balance(address: &Address, amount: &U256) -> Result<U256, ExitError> {
        let balance = Self::get_balance(address);
        if let Some(new_balance) = balance.checked_add(*amount) {
            Ok(new_balance)
        } else {
            Err(ExitError::Other(Borrowed(
                "balance is too high, can not increase",
            )))
        }
    }

    /// Checks if the balance can be decreased by an amount for a given address.
    ///
    /// Returns the new balance on success.
    ///
    /// # Errors
    ///
    /// * If the balance is < `U256::zero()`
    fn check_decrease_balance(address: &Address, amount: &U256) -> Result<U256, ExitError> {
        let balance = Self::get_balance(address);
        if let Some(new_balance) = balance.checked_sub(*amount) {
            Ok(new_balance)
        } else {
            Err(ExitError::Other(Borrowed(
                "balance is too low, can not decrease",
            )))
        }
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

    /// Transfers an amount from a given sender to a receiver, provided that
    /// the have enough in their balance.
    ///
    /// If the sender can send, and the receiver can receive, then the transfer
    /// will execute successfully.
    pub fn transfer(&mut self, sender: &Address, receiver: &Address, value: &U256) -> ExitReason {
        let balance = Self::get_balance(sender);
        if balance < *value {
            return ExitReason::Error(ExitError::OutOfFund);
        }

        let new_receiver_balance = match Self::check_increase_balance(receiver, value) {
            Ok(b) => b,
            Err(e) => return ExitReason::Error(e),
        };
        let new_sender_balance = match Self::check_decrease_balance(sender, value) {
            Ok(b) => b,
            Err(e) => return ExitReason::Error(e),
        };

        Self::set_balance(sender, &new_sender_balance);
        Self::set_balance(receiver, &new_receiver_balance);

        ExitReason::Succeed(ExitSucceed::Returned)
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
            executor.transact_create(origin, value, Vec::from(input), u64::MAX),
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
        let (status, result) = executor.transact_call(origin, contract, value, input, u64::MAX);
        let (values, logs) = executor.into_state().deconstruct();
        self.apply(values, logs, true);
        (status, result)
    }

    #[cfg(feature = "testnet")]
    /// Credits the address with 10 coins from the faucet.
    pub fn credit(&mut self, address: &Address) -> ExitReason {
        if let Err(e) = Self::increase_balance(address, &U256::from(10)) {
            return ExitReason::Error(e);
        }
        ExitReason::Succeed(ExitSucceed::Returned)
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
        executor.transact_call(origin, contract, value, input, u64::MAX)
    }

    fn make_executor(&self) -> StackExecutor<MemoryStackState<Engine>> {
        let metadata = StackSubstateMetadata::new(u64::MAX, &CONFIG);
        let state = MemoryStackState::new(metadata, self);
        StackExecutor::new_with_precompile(state, &CONFIG, precompiles::istanbul_precompiles)
    }
}

impl evm::backend::Backend for Engine {
    /// Returns the gas price.
    ///
    /// This is currently zero, but may be changed in the future. This is mainly
    /// because there already is another cost for transactions.
    fn gas_price(&self) -> U256 {
        U256::zero()
    }

    /// Returns the origin address that created the contract.
    fn origin(&self) -> Address {
        self.origin
    }

    /// Returns a block hash from a given index.
    ///
    /// Currently this returns zero, but may be changed in the future.
    fn block_hash(&self, _number: U256) -> H256 {
        H256::zero() // TODO: https://github.com/near/nearcore/issues/3456
    }

    /// Returns the current block index number.
    fn block_number(&self) -> U256 {
        U256::from(sdk::block_index())
    }

    /// Returns a mocked coinbase which is the first 160 bits of
    /// `keccak256(b"aurora")`.
    ///
    /// It is not possible to return the address of the current block's miner in
    /// NEAR as it isn't relevant.
    fn block_coinbase(&self) -> Address {
        Address([
            0x2b, 0x0b, 0xf3, 0xb8, 0xff, 0xaa, 0x4f, 0x3d, 0x1f, 0x97, 0x76, 0x0d, 0x44, 0x44,
            0x58, 0x84, 0x43, 0xc3, 0xa9, 0x12,
        ])
    }

    /// Returns the current block timestamp.
    fn block_timestamp(&self) -> U256 {
        U256::from(sdk::block_timestamp())
    }

    /// Returns the current block difficulty.
    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    /// Returns the current block's gas limit.
    ///
    /// Currently, this returns 0 as there is no concept of a gas limit.
    fn block_gas_limit(&self) -> U256 {
        U256::max_value()
    }

    /// Returns the states chain ID.
    fn chain_id(&self) -> U256 {
        U256::from(self.state.chain_id)
    }

    /// Checks if an address exists.
    fn exists(&self, address: Address) -> bool {
        !Engine::is_account_empty(&address)
    }

    /// Returns basic account information.
    fn basic(&self, address: Address) -> Basic {
        Basic {
            nonce: Engine::get_nonce(&address),
            balance: Engine::get_balance(&address),
        }
    }

    /// Returns the code of the contract from an address.
    fn code(&self, address: Address) -> Vec<u8> {
        Engine::get_code(&address)
    }

    /// Get storage value of address at index.
    fn storage(&self, address: Address, index: H256) -> H256 {
        Engine::get_storage(&address, &index)
    }

    /// Get original storage value of address at index, if available.
    ///
    /// Currently, this returns `None` for now.
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
