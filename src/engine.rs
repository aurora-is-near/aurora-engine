use borsh::{BorshDeserialize, BorshSerialize};
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::executor::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use evm::ExitFatal;
use evm::{Config, CreateScheme, ExitError, ExitReason};

use crate::map::LookupMap;
use crate::parameters::{FunctionCallArgs, NewCallArgs, SubmitResult, ViewCallArgs};
use crate::precompiles;
use crate::prelude::{Address, TryInto, Vec, H256, U256};
use crate::sdk;
use crate::storage::{address_to_key, storage_to_key, KeyPrefix, KeyPrefixU8};
use crate::types::{u256_to_arr, AccountId};

/// Errors with the EVM engine.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EngineError {
    /// Normal EVM errors.
    EvmError(ExitError),
    /// Fatal EVM errors.
    EvmFatal(ExitFatal),
    /// Incorrect nonce.
    IncorrectNonce,
}

impl EngineError {
    pub fn to_str(&self) -> &str {
        use EngineError::*;
        match self {
            EvmError(ExitError::StackUnderflow) => "ERR_STACK_UNDERFLOW",
            EvmError(ExitError::StackOverflow) => "ERR_STACK_OVERFLOW",
            EvmError(ExitError::InvalidJump) => "ERR_INVALID_JUMP",
            EvmError(ExitError::InvalidRange) => "ERR_INVALID_RANGE",
            EvmError(ExitError::DesignatedInvalid) => "ERR_DESIGNATED_INVALID",
            EvmError(ExitError::CallTooDeep) => "ERR_CALL_TOO_DEEP",
            EvmError(ExitError::CreateCollision) => "ERR_CREATE_COLLISION",
            EvmError(ExitError::CreateContractLimit) => "ERR_CREATE_CONTRACT_LIMIT",
            EvmError(ExitError::OutOfOffset) => "ERR_OUT_OF_OFFSET",
            EvmError(ExitError::OutOfGas) => "ERR_OUT_OF_GAS",
            EvmError(ExitError::OutOfFund) => "ERR_OUT_OF_FUND",
            EvmError(ExitError::Other(m)) => m,
            EvmError(_) => unreachable!(), // unused misc
            EvmFatal(ExitFatal::NotSupported) => "ERR_NOT_SUPPORTED",
            EvmFatal(ExitFatal::UnhandledInterrupt) => "ERR_UNHANDLED_INTERRUPT",
            EvmFatal(ExitFatal::Other(m)) => m,
            EvmFatal(_) => unreachable!(), // unused misc
            IncorrectNonce => "ERR_NONCE_INCORRECT",
        }
    }
}

impl AsRef<str> for EngineError {
    fn as_ref(&self) -> &str {
        self.to_str()
    }
}

impl AsRef<[u8]> for EngineError {
    fn as_ref(&self) -> &[u8] {
        self.to_str().as_bytes()
    }
}

impl From<ExitError> for EngineError {
    fn from(e: ExitError) -> Self {
        EngineError::EvmError(e)
    }
}

impl From<ExitFatal> for EngineError {
    fn from(e: ExitFatal) -> Self {
        EngineError::EvmFatal(e)
    }
}

/// An engine result.
pub type EngineResult<T> = Result<T, EngineError>;

trait ExitIntoResult {
    /// Checks if the EVM exit is ok or an error.
    fn into_result(self) -> EngineResult<()>;
}

impl ExitIntoResult for ExitReason {
    fn into_result(self) -> EngineResult<()> {
        use ExitReason::*;
        match self {
            Succeed(_) | Revert(_) => Ok(()),
            Error(e) => Err(e.into()),
            Fatal(e) => Err(e.into()),
        }
    }
}

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
    /// Mapping between relayer account id and relayer evm address
    pub relayers_evm_addresses: LookupMap<{ KeyPrefix::RelayerEvmAddressMap as KeyPrefixU8 }>,
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        EngineState {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            bridge_prover_id: args.bridge_prover_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
            relayers_evm_addresses: LookupMap::new(),
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

    /// Checks the nonce to ensure that the address matches the transaction
    /// nonce.
    #[inline]
    pub fn check_nonce(address: &Address, transaction_nonce: &U256) -> EngineResult<()> {
        let account_nonce = Self::get_nonce(address);

        if transaction_nonce != &account_nonce {
            return Err(EngineError::IncorrectNonce);
        }

        Ok(())
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

    pub fn deploy_code_with_input(&mut self, input: &[u8]) -> EngineResult<SubmitResult> {
        let origin = self.origin();
        let value = U256::zero();
        self.deploy_code(origin, value, input)
    }

    pub fn deploy_code(
        &mut self,
        origin: Address,
        value: U256,
        input: &[u8],
    ) -> EngineResult<SubmitResult> {
        let mut executor = self.make_executor();
        let address = executor.create_address(CreateScheme::Legacy { caller: origin });
        let (status, result) = (
            executor.transact_create(origin, value, Vec::from(input), u64::MAX),
            address,
        );

        let is_succeed = status.is_succeed();
        status.into_result()?;
        let used_gas = executor.used_gas();
        let (values, logs) = executor.into_state().deconstruct();
        self.apply(values, Vec::<Log>::new(), true);

        Ok(SubmitResult {
            status: is_succeed,
            gas_used: used_gas,
            result: result.0.to_vec(),
            logs: logs.into_iter().map(Into::into).collect(),
        })
    }

    pub fn call_with_args(&mut self, args: FunctionCallArgs) -> EngineResult<SubmitResult> {
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
    ) -> EngineResult<SubmitResult> {
        let mut executor = self.make_executor();
        let (status, result) = executor.transact_call(origin, contract, value, input, u64::MAX);

        let used_gas = executor.used_gas();
        let (values, logs) = executor.into_state().deconstruct();
        let is_succeed = status.is_succeed();
        status.into_result()?;
        // There is no way to return the logs to the NEAR log method as it only
        // allows a return of UTF-8 strings.
        self.apply(values, Vec::<Log>::new(), true);

        Ok(SubmitResult {
            status: is_succeed,
            gas_used: used_gas,
            result,
            logs: logs.into_iter().map(Into::into).collect(),
        })
    }

    #[cfg(feature = "testnet")]
    pub fn increment_nonce(&self, address: &Address) {
        let account_nonce = Self::get_nonce(address);
        account_nonce.saturating_add(U256::one());
        Self::set_nonce(address, &account_nonce);
    }

    #[cfg(feature = "testnet")]
    /// Credits the address with 10 coins from the faucet.
    pub fn credit(&self, address: &Address) -> EngineResult<()> {
        use crate::prelude::Add;

        let balance = Self::get_balance(address);
        // Saturating adds are intentional
        let new_balance = balance.saturating_add(U256::one());

        Self::set_balance(address, &new_balance);
        Ok(())
    }

    pub fn view_with_args(&self, args: ViewCallArgs) -> EngineResult<Vec<u8>> {
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
    ) -> EngineResult<Vec<u8>> {
        let mut executor = self.make_executor();
        let (status, result) = executor.transact_call(origin, contract, value, input, u64::MAX);
        status.into_result()?;
        Ok(result)
    }

    fn make_executor(&self) -> StackExecutor<MemoryStackState<Engine>> {
        let metadata = StackSubstateMetadata::new(u64::MAX, &CONFIG);
        let state = MemoryStackState::new(metadata, self);
        StackExecutor::new_with_precompile(state, &CONFIG, precompiles::istanbul_precompiles)
    }

    pub fn register_relayer(&mut self, account_id: &[u8], evm_address: Address) {
        self.state
            .relayers_evm_addresses
            .insert_raw(account_id, evm_address.as_bytes());
    }

    #[allow(dead_code)]
    pub fn get_relayer(&self, account_id: &[u8]) -> Option<Address> {
        self.state
            .relayers_evm_addresses
            .get_raw(account_id)
            .map(|result| Address(result.as_slice().try_into().unwrap()))
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
    ///
    /// See: https://doc.aurora.dev/develop/compat/evm#blockhash
    fn block_hash(&self, _number: U256) -> H256 {
        H256::zero() // TODO: https://github.com/near/nearcore/issues/3456
    }

    /// Returns the current block index number.
    fn block_number(&self) -> U256 {
        U256::from(sdk::block_index())
    }

    /// Returns a mocked coinbase which is the EVM address for the Aurora
    /// account, being 0x4444588443C3a91288c5002483449Aba1054192b.
    ///
    /// See: https://doc.aurora.dev/develop/compat/evm#coinbase
    fn block_coinbase(&self) -> Address {
        Address([
            0x44, 0x44, 0x58, 0x84, 0x43, 0xC3, 0xa9, 0x12, 0x88, 0xc5, 0x00, 0x24, 0x83, 0x44,
            0x9A, 0xba, 0x10, 0x54, 0x19, 0x2b,
        ])
    }

    /// Returns the current block timestamp.
    fn block_timestamp(&self) -> U256 {
        U256::from(sdk::block_timestamp())
    }

    /// Returns the current block difficulty.
    ///
    /// See: https://doc.aurora.dev/develop/compat/evm#difficulty
    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    /// Returns the current block gas limit.
    ///
    /// Currently, this returns 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
    /// as there isn't a gas limit alternative right now but this may change in
    /// the future.
    ///
    /// See: https://doc.aurora.dev/develop/compat/evm#gaslimit
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
    fn apply<A, I, L>(&mut self, values: A, _logs: L, delete_empty: bool)
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
    }
}

#[cfg(test)]
mod tests {}
