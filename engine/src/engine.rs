use crate::parameters::{
    CallArgsType, NEP141FtOnTransferArgs, ResultLog, SubmitResult, ViewCallArgs,
};
use core::mem;
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use evm::executor;
use evm::{Config, CreateScheme, ExitError, ExitFatal, ExitReason};

use crate::connector::EthConnectorContract;
#[cfg(feature = "contract")]
use crate::contract::current_address;
use crate::map::{BijectionMap, LookupMap};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::near_runtime::Runtime;

use crate::parameters::{NewCallArgs, TransactionStatus};
use crate::prelude::precompiles::native::{ExitToEthereum, ExitToNear};
use crate::prelude::precompiles::Precompiles;
use crate::prelude::{
    address_to_key, bytes_to_key, sdk, storage_to_key, u256_to_arr, AccountId, Address,
    BorshDeserialize, BorshSerialize, KeyPrefix, KeyPrefixU8, PromiseArgs, PromiseCreateArgs,
    TryFrom, TryInto, Vec, Wei, ERC20_MINT_SELECTOR, H256, U256,
};
use crate::transaction::NormalizedEthTransaction;

/// Used as the first byte in the concatenation of data used to compute the blockhash.
/// Could be useful in the future as a version byte, or to distinguish different types of blocks.
const BLOCK_HASH_PREFIX: u8 = 0;
const BLOCK_HASH_PREFIX_SIZE: usize = 1;
const BLOCK_HEIGHT_SIZE: usize = 8;
const CHAIN_ID_SIZE: usize = 32;

#[cfg(not(feature = "contract"))]
pub fn current_address() -> Address {
    sdk::types::near_account_to_evm_address("engine".as_bytes())
}

macro_rules! unwrap_res_or_finish {
    ($e:expr, $output:expr, $io:expr) => {
        match $e {
            Ok(v) => v,
            Err(_e) => {
                #[cfg(feature = "log")]
                sdk::log(crate::prelude::format!("{:?}", _e).as_str());
                $io.return_output($output);
                return;
            }
        }
    };
}

macro_rules! assert_or_finish {
    ($e:expr, $output:expr, $io:expr) => {
        if !$e {
            $io.return_output($output);
            return;
        }
    };
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EngineError {
    pub kind: EngineErrorKind,
    pub gas_used: u64,
}

impl AsRef<str> for EngineError {
    fn as_ref(&self) -> &str {
        self.kind.to_str()
    }
}

impl AsRef<[u8]> for EngineError {
    fn as_ref(&self) -> &[u8] {
        self.kind.to_str().as_bytes()
    }
}

/// Errors with the EVM engine.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EngineErrorKind {
    /// Normal EVM errors.
    EvmError(ExitError),
    /// Fatal EVM errors.
    EvmFatal(ExitFatal),
    /// Incorrect nonce.
    IncorrectNonce,
}

impl EngineErrorKind {
    pub fn with_gas_used(self, gas_used: u64) -> EngineError {
        EngineError {
            kind: self,
            gas_used,
        }
    }

    pub fn to_str(&self) -> &str {
        use EngineErrorKind::*;
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
            IncorrectNonce => "ERR_INCORRECT_NONCE",
        }
    }
}

impl AsRef<str> for EngineErrorKind {
    fn as_ref(&self) -> &str {
        self.to_str()
    }
}

impl AsRef<[u8]> for EngineErrorKind {
    fn as_ref(&self) -> &[u8] {
        self.to_str().as_bytes()
    }
}

impl From<ExitError> for EngineErrorKind {
    fn from(e: ExitError) -> Self {
        EngineErrorKind::EvmError(e)
    }
}

impl From<ExitFatal> for EngineErrorKind {
    fn from(e: ExitFatal) -> Self {
        EngineErrorKind::EvmFatal(e)
    }
}

/// An engine result.
pub type EngineResult<T> = Result<T, EngineError>;

trait ExitIntoResult {
    /// Checks if the EVM exit is ok or an error.
    fn into_result(self, data: Vec<u8>) -> Result<TransactionStatus, EngineErrorKind>;
}

impl ExitIntoResult for ExitReason {
    fn into_result(self, data: Vec<u8>) -> Result<TransactionStatus, EngineErrorKind> {
        use ExitReason::*;
        match self {
            Succeed(_) => Ok(TransactionStatus::Succeed(data)),
            Revert(_) => Ok(TransactionStatus::Revert(data)),
            Error(ExitError::OutOfOffset) => Ok(TransactionStatus::OutOfOffset),
            Error(ExitError::OutOfFund) => Ok(TransactionStatus::OutOfFund),
            Error(ExitError::OutOfGas) => Ok(TransactionStatus::OutOfGas),
            Error(e) => Err(e.into()),
            Fatal(e) => Err(e.into()),
        }
    }
}

pub struct BalanceOverflow;

impl AsRef<[u8]> for BalanceOverflow {
    fn as_ref(&self) -> &[u8] {
        b"ERR_BALANCE_OVERFLOW"
    }
}

/// Errors resulting from trying to pay for gas
pub enum GasPaymentError {
    /// Overflow adding ETH to an account balance (should never happen)
    BalanceOverflow(BalanceOverflow),
    /// Overflow in gas * gas_price calculation
    EthAmountOverflow,
    /// Not enough balance for account to cover the gas cost
    OutOfFund,
}

impl AsRef<[u8]> for GasPaymentError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::BalanceOverflow(overflow) => overflow.as_ref(),
            Self::EthAmountOverflow => b"ERR_GAS_ETH_AMOUNT_OVERFLOW",
            Self::OutOfFund => b"ERR_OUT_OF_FUND",
        }
    }
}

impl From<BalanceOverflow> for GasPaymentError {
    fn from(overflow: BalanceOverflow) -> Self {
        Self::BalanceOverflow(overflow)
    }
}

pub const ERR_INVALID_NEP141_ACCOUNT_ID: &str = "ERR_INVALID_NEP141_ACCOUNT_ID";

#[derive(Debug)]
pub enum GetErc20FromNep141Error {
    InvalidNep141AccountId,
    Nep141NotFound,
}

impl GetErc20FromNep141Error {
    pub fn to_str(&self) -> &str {
        match self {
            Self::InvalidNep141AccountId => ERR_INVALID_NEP141_ACCOUNT_ID,
            Self::Nep141NotFound => "ERR_NEP141_NOT_FOUND",
        }
    }
}

impl AsRef<[u8]> for GetErc20FromNep141Error {
    fn as_ref(&self) -> &[u8] {
        self.to_str().as_bytes()
    }
}

#[derive(Debug)]
pub enum RegisterTokenError {
    InvalidNep141AccountId,
    TokenAlreadyRegistered,
}

impl RegisterTokenError {
    pub fn to_str(&self) -> &str {
        match self {
            Self::InvalidNep141AccountId => ERR_INVALID_NEP141_ACCOUNT_ID,
            Self::TokenAlreadyRegistered => "ERR_NEP141_TOKEN_ALREADY_REGISTERED",
        }
    }
}

impl AsRef<[u8]> for RegisterTokenError {
    fn as_ref(&self) -> &[u8] {
        self.to_str().as_bytes()
    }
}

#[derive(Debug)]
pub enum EngineStateError {
    NotFound,
    DeserializationFailed,
}

impl AsRef<[u8]> for EngineStateError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::NotFound => b"ERR_STATE_NOT_FOUND",
            Self::DeserializationFailed => b"ERR_STATE_CORRUPTED",
        }
    }
}

struct StackExecutorParams {
    precompiles: Precompiles,
    gas_limit: u64,
}

impl StackExecutorParams {
    fn new(gas_limit: u64) -> Self {
        Self {
            precompiles: Precompiles::new_london(),
            gas_limit,
        }
    }

    fn make_executor<'a, I: IO + Default + Copy>(
        &'a self,
        engine: &'a Engine<I>,
    ) -> executor::StackExecutor<'static, 'a, executor::MemoryStackState<Engine<I>>, Precompiles>
    {
        let metadata = executor::StackSubstateMetadata::new(self.gas_limit, CONFIG);
        let state = executor::MemoryStackState::new(metadata, engine);
        executor::StackExecutor::new_with_precompiles(state, CONFIG, &self.precompiles)
    }
}

#[derive(Debug, Default)]
pub struct GasPaymentResult {
    pub prepaid_amount: Wei,
    pub effective_gas_price: U256,
    pub priority_fee_per_gas: U256,
}

/// Engine internal state, mostly configuration.
/// Should not contain anything large or enumerable.
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct EngineState {
    /// Chain id, according to the EIP-155 / ethereum-lists spec.
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
    pub relayers_evm_addresses:
        LookupMap<Runtime, { KeyPrefix::RelayerEvmAddressMap as KeyPrefixU8 }>,
}

impl From<NewCallArgs> for EngineState {
    fn from(args: NewCallArgs) -> Self {
        EngineState {
            chain_id: args.chain_id,
            owner_id: args.owner_id,
            bridge_prover_id: args.bridge_prover_id,
            upgrade_delay_blocks: args.upgrade_delay_blocks,
            relayers_evm_addresses: LookupMap::default(),
        }
    }
}

pub struct Engine<I: IO> {
    state: EngineState,
    origin: Address,
    gas_price: U256,
    io: I,
}

// TODO: upgrade to Berlin HF
pub(crate) const CONFIG: &Config = &Config::london();

/// Key for storing the state of the engine.
const STATE_KEY: &[u8; 5] = b"STATE";

impl<I: IO + Copy + Default> Engine<I> {
    pub fn new(origin: Address, io: I) -> Result<Self, EngineStateError> {
        Engine::get_state(&io).map(|state| Self::new_with_state(state, origin, io))
    }

    pub fn new_with_state(state: EngineState, origin: Address, io: I) -> Self {
        Self {
            state,
            origin,
            gas_price: U256::zero(),
            io,
        }
    }

    /// Saves state into the storage.
    pub fn set_state(io: &mut I, state: EngineState) {
        io.write_storage(
            &bytes_to_key(KeyPrefix::Config, STATE_KEY),
            &state.try_to_vec().expect("ERR_SER"),
        );
    }

    pub fn charge_gas(
        &mut self,
        sender: &Address,
        transaction: &NormalizedEthTransaction,
    ) -> Result<GasPaymentResult, GasPaymentError> {
        if transaction.max_fee_per_gas.is_zero() {
            return Ok(GasPaymentResult::default());
        }

        let priority_fee_per_gas = transaction
            .max_priority_fee_per_gas
            .min(transaction.max_fee_per_gas - self.block_base_fee_per_gas());
        let effective_gas_price = priority_fee_per_gas + self.block_base_fee_per_gas();
        let gas_limit = transaction.gas_limit;
        let prepaid_amount = gas_limit
            .checked_mul(effective_gas_price)
            .map(Wei::new)
            .ok_or(GasPaymentError::EthAmountOverflow)?;

        let new_balance = Self::get_balance(&self.io, sender)
            .checked_sub(prepaid_amount)
            .ok_or(GasPaymentError::OutOfFund)?;

        Self::set_balance(&mut self.io, sender, &new_balance);

        self.gas_price = effective_gas_price;

        Ok(GasPaymentResult {
            prepaid_amount,
            effective_gas_price,
            priority_fee_per_gas,
        })
    }

    pub fn refund_unused_gas(
        io: &mut I,
        sender: &Address,
        gas_used: u64,
        gas_result: GasPaymentResult,
        relayer: &Address,
    ) -> Result<(), GasPaymentError> {
        if gas_result.effective_gas_price.is_zero() {
            return Ok(());
        }

        let gas_to_wei = |price: U256| {
            U256::from(gas_used)
                .checked_mul(price)
                .map(Wei::new)
                .ok_or(GasPaymentError::EthAmountOverflow)
        };

        let spent_amount = gas_to_wei(gas_result.effective_gas_price)?;
        let reward_amount = gas_to_wei(gas_result.priority_fee_per_gas)?;

        let refund = gas_result
            .prepaid_amount
            .checked_sub(spent_amount)
            .ok_or(GasPaymentError::EthAmountOverflow)?;

        Self::add_balance(io, sender, refund)?;
        Self::add_balance(io, relayer, reward_amount)?;

        Ok(())
    }

    /// Fails if state is not found.
    pub fn get_state(io: &I) -> Result<EngineState, EngineStateError> {
        match io.read_storage(&bytes_to_key(KeyPrefix::Config, STATE_KEY)) {
            None => Err(EngineStateError::NotFound),
            Some(bytes) => EngineState::try_from_slice(&bytes.to_vec())
                .map_err(|_| EngineStateError::DeserializationFailed),
        }
    }

    pub fn set_code(io: &mut I, address: &Address, code: &[u8]) {
        io.write_storage(&address_to_key(KeyPrefix::Code, address), code);
    }

    pub fn remove_code(io: &mut I, address: &Address) {
        io.remove_storage(&address_to_key(KeyPrefix::Code, address));
    }

    pub fn get_code(io: &I, address: &Address) -> Vec<u8> {
        io.read_storage(&address_to_key(KeyPrefix::Code, address))
            .map(|s| s.to_vec())
            .unwrap_or_else(Vec::new)
    }

    pub fn get_code_size(io: &I, address: &Address) -> usize {
        io.read_storage_len(&address_to_key(KeyPrefix::Code, address))
            .unwrap_or(0)
    }

    pub fn set_nonce(io: &mut I, address: &Address, nonce: &U256) {
        io.write_storage(
            &address_to_key(KeyPrefix::Nonce, address),
            &u256_to_arr(nonce),
        );
    }

    pub fn remove_nonce(io: &mut I, address: &Address) {
        io.remove_storage(&address_to_key(KeyPrefix::Nonce, address));
    }

    /// Checks the nonce to ensure that the address matches the transaction
    /// nonce.
    #[inline]
    pub fn check_nonce(
        io: &I,
        address: &Address,
        transaction_nonce: &U256,
    ) -> Result<(), EngineErrorKind> {
        let account_nonce = Self::get_nonce(io, address);

        if transaction_nonce != &account_nonce {
            return Err(EngineErrorKind::IncorrectNonce);
        }

        Ok(())
    }

    pub fn get_nonce(io: &I, address: &Address) -> U256 {
        io.read_u256(&address_to_key(KeyPrefix::Nonce, address))
            .unwrap_or_else(|_| U256::zero())
    }

    pub fn add_balance(io: &mut I, address: &Address, amount: Wei) -> Result<(), BalanceOverflow> {
        let current_balance = Self::get_balance(io, address);
        let new_balance = current_balance.checked_add(amount).ok_or(BalanceOverflow)?;
        Self::set_balance(io, address, &new_balance);
        Ok(())
    }

    pub fn set_balance(io: &mut I, address: &Address, balance: &Wei) {
        io.write_storage(
            &address_to_key(KeyPrefix::Balance, address),
            &balance.to_bytes(),
        );
    }

    pub fn remove_balance(io: &mut I, address: &Address) {
        let balance = Self::get_balance(io, address);
        // Apply changes for eth-conenctor
        EthConnectorContract::get_instance(*io).internal_remove_eth(address, &balance.raw());
        io.remove_storage(&address_to_key(KeyPrefix::Balance, address));
    }

    pub fn get_balance(io: &I, address: &Address) -> Wei {
        let raw = io
            .read_u256(&address_to_key(KeyPrefix::Balance, address))
            .unwrap_or_else(|_| U256::zero());
        Wei::new(raw)
    }

    pub fn remove_storage(io: &mut I, address: &Address, key: &H256, generation: u32) {
        io.remove_storage(storage_to_key(address, key, generation).as_ref());
    }

    pub fn set_storage(io: &mut I, address: &Address, key: &H256, value: &H256, generation: u32) {
        io.write_storage(storage_to_key(address, key, generation).as_ref(), &value.0);
    }

    pub fn get_storage(io: &I, address: &Address, key: &H256, generation: u32) -> H256 {
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
            .unwrap_or_else(H256::default)
    }

    pub fn is_account_empty(io: &I, address: &Address) -> bool {
        let balance = Self::get_balance(io, address);
        let nonce = Self::get_nonce(io, address);
        let code_len = Self::get_code_size(io, address);
        balance.is_zero() && nonce.is_zero() && code_len == 0
    }

    /// Increments storage generation for a given address.
    pub fn set_generation(io: &mut I, address: &Address, generation: u32) {
        io.write_storage(
            &address_to_key(KeyPrefix::Generation, address),
            &generation.to_be_bytes(),
        );
    }

    pub fn get_generation(io: &I, address: &Address) -> u32 {
        io.read_storage(&address_to_key(KeyPrefix::Generation, address))
            .map(|value| {
                let mut bytes = [0u8; 4];
                value.copy_to_slice(&mut bytes);
                u32::from_be_bytes(bytes)
            })
            .unwrap_or(0)
    }

    /// Removes all storage for the given address.
    fn remove_all_storage(io: &mut I, address: &Address, generation: u32) {
        // FIXME: there is presently no way to prefix delete trie state.
        // NOTE: There is not going to be a method on runtime for this.
        //     You may need to store all keys in a list if you want to do this in a contract.
        //     Maybe you can incentivize people to delete dead old keys. They can observe them from
        //     external indexer node and then issue special cleaning transaction.
        //     Either way you may have to store the nonce per storage address root. When the account
        //     has to be deleted the storage nonce needs to be increased, and the old nonce keys
        //     can be deleted over time. That's how TurboGeth does storage.
        Self::set_generation(io, address, generation + 1);
    }

    /// Removes an account.
    fn remove_account(io: &mut I, address: &Address, generation: u32) {
        Self::remove_nonce(io, address);
        Self::remove_balance(io, address);
        Self::remove_code(io, address);
        Self::remove_all_storage(io, address, generation);
    }

    pub fn deploy_code_with_input(&mut self, input: Vec<u8>) -> EngineResult<SubmitResult> {
        let origin = self.origin();
        let value = Wei::zero();
        self.deploy_code(origin, value, input, u64::MAX, Vec::new())
    }

    pub fn deploy_code(
        &mut self,
        origin: Address,
        value: Wei,
        input: Vec<u8>,
        gas_limit: u64,
        access_list: Vec<(Address, Vec<H256>)>, // See EIP-2930
    ) -> EngineResult<SubmitResult> {
        let executor_params = StackExecutorParams::new(gas_limit);
        let mut executor = executor_params.make_executor(self);
        let address = executor.create_address(CreateScheme::Legacy { caller: origin });
        let (exit_reason, result) = (
            executor.transact_create(origin, value.raw(), input, gas_limit, access_list),
            address,
        );

        let used_gas = executor.used_gas();
        let status = match exit_reason.into_result(result.0.to_vec()) {
            Ok(status) => status,
            Err(e) => {
                Engine::increment_nonce(&mut self.io, &origin);
                return Err(e.with_gas_used(used_gas));
            }
        };

        let (values, logs) = executor.into_state().deconstruct();
        let logs = Self::filter_promises_from_logs(logs);

        self.apply(values, Vec::<Log>::new(), true);

        Ok(SubmitResult::new(status, used_gas, logs))
    }

    /// Call the EVM contract with arguments (attached ETH balance, input, gas limit, etc.)
    pub fn call_with_args(&mut self, args: CallArgsType) -> EngineResult<SubmitResult> {
        let origin = self.origin();
        match args {
            CallArgsType::New(call_args) => {
                let contract = Address(call_args.contract);
                let value = call_args.value.into();
                let input = call_args.input;
                self.call(origin, contract, value, input, u64::MAX, Vec::new())
            }
            CallArgsType::Legacy(call_args) => {
                let contract = Address(call_args.contract);
                let value = Wei::zero();
                let input = call_args.input;
                self.call(origin, contract, value, input, u64::MAX, Vec::new())
            }
        }
    }

    pub fn call(
        &mut self,
        origin: Address,
        contract: Address,
        value: Wei,
        input: Vec<u8>,
        gas_limit: u64,
        access_list: Vec<(Address, Vec<H256>)>, // See EIP-2930
    ) -> EngineResult<SubmitResult> {
        let executor_params = StackExecutorParams::new(gas_limit);
        let mut executor = executor_params.make_executor(self);
        let (exit_reason, result) =
            executor.transact_call(origin, contract, value.raw(), input, gas_limit, access_list);

        let used_gas = executor.used_gas();
        let status = match exit_reason.into_result(result) {
            Ok(status) => status,
            Err(e) => {
                Engine::increment_nonce(&mut self.io, &origin);
                return Err(e.with_gas_used(used_gas));
            }
        };

        let (values, logs) = executor.into_state().deconstruct();
        let logs = Self::filter_promises_from_logs(logs);

        // There is no way to return the logs to the NEAR log method as it only
        // allows a return of UTF-8 strings.
        self.apply(values, Vec::<Log>::new(), true);

        Ok(SubmitResult::new(status, used_gas, logs))
    }

    pub fn increment_nonce(io: &mut I, address: &Address) {
        let account_nonce = Self::get_nonce(io, address);
        let new_nonce = account_nonce.saturating_add(U256::one());
        Self::set_nonce(io, address, &new_nonce);
    }

    pub fn view_with_args(&self, args: ViewCallArgs) -> Result<TransactionStatus, EngineErrorKind> {
        let origin = Address::from_slice(&args.sender);
        let contract = Address::from_slice(&args.address);
        let value = U256::from_big_endian(&args.amount);
        self.view(origin, contract, Wei::new(value), args.input, u64::MAX)
    }

    pub fn view(
        &self,
        origin: Address,
        contract: Address,
        value: Wei,
        input: Vec<u8>,
        gas_limit: u64,
    ) -> Result<TransactionStatus, EngineErrorKind> {
        let executor_params = StackExecutorParams::new(gas_limit);
        let mut executor = executor_params.make_executor(self);
        let (status, result) =
            executor.transact_call(origin, contract, value.raw(), input, gas_limit, Vec::new());
        status.into_result(result)
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

    pub fn register_token(
        &mut self,
        erc20_token: &[u8],
        nep141_token: &[u8],
    ) -> Result<(), RegisterTokenError> {
        match Self::get_erc20_from_nep141(self.io, nep141_token) {
            Err(GetErc20FromNep141Error::Nep141NotFound) => (),
            Err(GetErc20FromNep141Error::InvalidNep141AccountId) => {
                return Err(RegisterTokenError::InvalidNep141AccountId);
            }
            Ok(_) => return Err(RegisterTokenError::TokenAlreadyRegistered),
        }

        Self::nep141_erc20_map(self.io).insert(nep141_token, erc20_token);
        Ok(())
    }

    pub fn get_erc20_from_nep141(
        io: I,
        nep141_account_id: &[u8],
    ) -> Result<Vec<u8>, GetErc20FromNep141Error> {
        AccountId::try_from(nep141_account_id)
            .map_err(|_| GetErc20FromNep141Error::InvalidNep141AccountId)?;

        Self::nep141_erc20_map(io)
            .lookup_left(nep141_account_id)
            .ok_or(GetErc20FromNep141Error::Nep141NotFound)
    }

    /// Transfers an amount from a given sender to a receiver, provided that
    /// the have enough in their balance.
    ///
    /// If the sender can send, and the receiver can receive, then the transfer
    /// will execute successfully.
    pub fn transfer(
        &mut self,
        sender: Address,
        receiver: Address,
        value: Wei,
        gas_limit: u64,
    ) -> EngineResult<SubmitResult> {
        self.call(sender, receiver, value, Vec::new(), gas_limit, Vec::new())
    }

    /// Mint tokens for recipient on a particular ERC20 token
    /// This function should return the amount of tokens unused,
    /// which will be always all (<amount>) if there is any problem
    /// with the input, or 0 if tokens were minted successfully.
    ///
    /// The output will be serialized as a String
    /// https://github.com/near/NEPs/discussions/146
    ///
    /// IMPORTANT: This function should not panic, otherwise it won't
    /// be possible to return the tokens to the sender.
    pub fn receive_erc20_tokens(&mut self, args: &NEP141FtOnTransferArgs) {
        let str_amount = crate::prelude::format!("\"{}\"", args.amount);
        let output_on_fail = str_amount.as_bytes();

        // Parse message to determine recipient and fee
        let (recipient, fee) = {
            // Message format:
            //      Recipient of the transaction - 40 characters (Address in hex)
            //      Fee to be paid in ETH (Optional) - 64 characters (Encoded in big endian / hex)
            let mut message = args.msg.as_bytes();
            assert_or_finish!(message.len() >= 40, output_on_fail, self.io);

            let recipient = Address(unwrap_res_or_finish!(
                hex::decode(&message[..40]).unwrap().as_slice().try_into(),
                output_on_fail,
                self.io
            ));
            message = &message[40..];

            let fee = if message.is_empty() {
                U256::from(0)
            } else {
                assert_or_finish!(message.len() == 64, output_on_fail, self.io);
                U256::from_big_endian(
                    unwrap_res_or_finish!(hex::decode(message), output_on_fail, self.io).as_slice(),
                )
            };

            (recipient, fee)
        };

        let token = sdk::predecessor_account_id();
        let erc20_token = Address(unwrap_res_or_finish!(
            unwrap_res_or_finish!(
                Self::get_erc20_from_nep141(self.io, &token),
                output_on_fail,
                self.io
            )
            .as_slice()
            .try_into(),
            output_on_fail,
            self.io
        ));

        if fee != U256::from(0) {
            let relayer_account_id = sdk::signer_account_id();
            let relayer_address = unwrap_res_or_finish!(
                self.get_relayer(relayer_account_id.as_slice()).ok_or(()),
                output_on_fail,
                self.io
            );

            unwrap_res_or_finish!(
                self.transfer(
                    recipient,
                    relayer_address,
                    Wei::new_u64(fee.as_u64()),
                    u64::MAX,
                ),
                output_on_fail,
                self.io
            );
        }

        let selector = ERC20_MINT_SELECTOR;
        let tail = ethabi::encode(&[
            ethabi::Token::Address(recipient),
            ethabi::Token::Uint(args.amount.into()),
        ]);

        let erc20_admin_address = current_address();
        unwrap_res_or_finish!(
            self.call(
                erc20_admin_address,
                erc20_token,
                Wei::zero(),
                [selector, tail.as_slice()].concat(),
                u64::MAX,
                Vec::new(), // TODO: are there values we should put here?
            )
            .and_then(|submit_result| {
                match submit_result.status {
                    TransactionStatus::Succeed(_) => Ok(()),
                    TransactionStatus::Revert(bytes) => {
                        let error_message = crate::prelude::format!(
                            "Reverted with message: {}",
                            crate::prelude::String::from_utf8_lossy(&bytes)
                        );
                        Err(EngineError {
                            kind: EngineErrorKind::EvmError(ExitError::Other(
                                crate::prelude::Cow::from(error_message),
                            )),
                            gas_used: submit_result.gas_used,
                        })
                    }
                    TransactionStatus::OutOfFund => Err(EngineError {
                        kind: EngineErrorKind::EvmError(ExitError::OutOfFund),
                        gas_used: submit_result.gas_used,
                    }),
                    TransactionStatus::OutOfOffset => Err(EngineError {
                        kind: EngineErrorKind::EvmError(ExitError::OutOfOffset),
                        gas_used: submit_result.gas_used,
                    }),
                    TransactionStatus::OutOfGas => Err(EngineError {
                        kind: EngineErrorKind::EvmError(ExitError::OutOfGas),
                        gas_used: submit_result.gas_used,
                    }),
                    TransactionStatus::CallTooDeep => Err(EngineError {
                        kind: EngineErrorKind::EvmError(ExitError::CallTooDeep),
                        gas_used: submit_result.gas_used,
                    }),
                }
            }),
            output_on_fail,
            self.io
        );

        // TODO(marX)
        // Everything succeed so return "0"
        self.io.return_output(b"\"0\"");
    }

    pub fn nep141_erc20_map(
        io: I,
    ) -> BijectionMap<
        I,
        { KeyPrefix::Nep141Erc20Map as KeyPrefixU8 },
        { KeyPrefix::Erc20Nep141Map as KeyPrefixU8 },
    > {
        BijectionMap { io }
    }

    fn filter_promises_from_logs<T: IntoIterator<Item = Log>>(logs: T) -> Vec<ResultLog> {
        logs.into_iter()
            .filter_map(|log| {
                if log.address == ExitToNear::ADDRESS || log.address == ExitToEthereum::ADDRESS {
                    if log.topics.is_empty() {
                        if let Ok(promise) = PromiseArgs::try_from_slice(&log.data) {
                            match promise {
                                PromiseArgs::Create(promise) => Self::schedule_promise(promise),
                                PromiseArgs::Callback(promise) => {
                                    let base_id = Self::schedule_promise(promise.base);
                                    Self::schedule_promise_callback(base_id, promise.callback)
                                }
                            };
                        }
                        // do not pass on these "internal logs" to caller
                        None
                    } else {
                        // The exit precompiles do produce externally consumable logs in
                        // addition to the promises. The external logs have a non-empty
                        // `topics` field.
                        Some(log.into())
                    }
                } else {
                    Some(log.into())
                }
            })
            .collect()
    }

    fn schedule_promise(promise: PromiseCreateArgs) -> u64 {
        sdk::log!(&crate::prelude::format!(
            "call_contract {}.{}",
            promise.target_account_id,
            promise.method
        ));
        sdk::promise_create(
            promise.target_account_id.as_bytes(),
            promise.method.as_bytes(),
            promise.args.as_slice(),
            promise.attached_balance,
            promise.attached_gas,
        )
    }

    fn schedule_promise_callback(base_id: u64, promise: PromiseCreateArgs) -> u64 {
        sdk::log!(&crate::prelude::format!(
            "callback_call_contract {}.{}",
            promise.target_account_id,
            promise.method
        ));
        sdk::promise_then(
            base_id,
            promise.target_account_id.as_bytes(),
            promise.method.as_bytes(),
            promise.args.as_slice(),
            promise.attached_balance,
            promise.attached_gas,
        )
    }
}

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
pub fn compute_block_hash(chain_id: [u8; 32], block_height: u64, account_id: &[u8]) -> H256 {
    debug_assert_eq!(BLOCK_HASH_PREFIX_SIZE, mem::size_of_val(&BLOCK_HASH_PREFIX));
    debug_assert_eq!(BLOCK_HEIGHT_SIZE, mem::size_of_val(&block_height));
    debug_assert_eq!(CHAIN_ID_SIZE, mem::size_of_val(&chain_id));
    let mut data = Vec::with_capacity(
        BLOCK_HASH_PREFIX_SIZE + BLOCK_HEIGHT_SIZE + CHAIN_ID_SIZE + account_id.len(),
    );
    data.push(BLOCK_HASH_PREFIX);
    data.extend_from_slice(&chain_id);
    data.extend_from_slice(account_id);
    data.extend_from_slice(&block_height.to_be_bytes());

    #[cfg(not(feature = "contract"))]
    {
        use sha2::Digest;

        let output = sha2::Sha256::digest(&data);
        H256(output.into())
    }

    #[cfg(feature = "contract")]
    sdk::sha256(&data)
}

impl<I: IO + Default + Copy> evm::backend::Backend for Engine<I> {
    /// Returns the "effective" gas price (as defined by EIP-1559)
    fn gas_price(&self) -> U256 {
        self.gas_price
    }

    /// Returns the origin address that created the contract.
    fn origin(&self) -> Address {
        self.origin
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
    /// See: https://doc.aurora.dev/develop/compat/evm#blockhash
    fn block_hash(&self, number: U256) -> H256 {
        let idx = U256::from(sdk::block_index());
        if idx.saturating_sub(U256::from(256)) <= number && number < idx {
            // since `idx` comes from `u64` it is always safe to downcast `number` from `U256`
            #[cfg(feature = "contract")]
            {
                let account_id = sdk::current_account_id();
                compute_block_hash(self.state.chain_id, number.low_u64(), &account_id)
            }

            #[cfg(not(feature = "contract"))]
            compute_block_hash(self.state.chain_id, number.low_u64(), b"aurora")
        } else {
            H256::zero()
        }
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
    fn exists(&self, address: Address) -> bool {
        !Engine::is_account_empty(&self.io, &address)
    }

    /// Returns basic account information.
    fn basic(&self, address: Address) -> Basic {
        Basic {
            nonce: Engine::get_nonce(&self.io, &address),
            balance: Engine::get_balance(&self.io, &address).raw(),
        }
    }

    /// Returns the code of the contract from an address.
    fn code(&self, address: Address) -> Vec<u8> {
        Engine::get_code(&self.io, &address)
    }

    /// Get storage value of address at index.
    fn storage(&self, address: Address, index: H256) -> H256 {
        let generation = Self::get_generation(&self.io, &address);
        Engine::get_storage(&self.io, &address, &index, generation)
    }

    /// Get original storage value of address at index, if available.
    ///
    /// Currently, this returns `None` for now.
    fn original_storage(&self, _address: Address, _index: H256) -> Option<H256> {
        None
    }
}

impl<J: IO + Default + Copy> ApplyBackend for Engine<J> {
    fn apply<A, I, L>(&mut self, values: A, _logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
        let mut writes_counter: usize = 0;
        let mut code_bytes_written: usize = 0;
        for apply in values {
            match apply {
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage,
                } => {
                    let generation = Self::get_generation(&self.io, &address);
                    Engine::set_nonce(&mut self.io, &address, &basic.nonce);
                    Engine::set_balance(&mut self.io, &address, &Wei::new(basic.balance));
                    writes_counter += 2; // 1 for nonce, 1 for balance

                    if let Some(code) = code {
                        Engine::set_code(&mut self.io, &address, &code);
                        code_bytes_written = code.len();
                        sdk::log!(crate::prelude::format!(
                            "code_write_at_address {:?} {}",
                            address,
                            code_bytes_written,
                        )
                        .as_str());
                    }

                    let next_generation = if reset_storage {
                        Engine::remove_all_storage(&mut self.io, &address, generation);
                        generation + 1
                    } else {
                        generation
                    };

                    for (index, value) in storage {
                        if value == H256::default() {
                            Engine::remove_storage(&mut self.io, &address, &index, next_generation)
                        } else {
                            Engine::set_storage(
                                &mut self.io,
                                &address,
                                &index,
                                &value,
                                next_generation,
                            )
                        }
                        writes_counter += 1;
                    }

                    // We only need to remove the account if:
                    // 1. we are supposed to delete an empty account
                    // 2. the account is empty
                    // 3. we didn't already clear out the storage (because if we did then there is
                    //    nothing to do)
                    if delete_empty
                        && Engine::is_account_empty(&self.io, &address)
                        && generation == next_generation
                    {
                        Engine::remove_account(&mut self.io, &address, generation);
                        writes_counter += 1;
                    }
                }
                Apply::Delete { address } => {
                    let generation = Self::get_generation(&self.io, &address);
                    Engine::remove_account(&mut self.io, &address, generation);
                    writes_counter += 1;
                }
            }
        }
        // These variable are only used if logging feature is enabled.
        // In production logging is always enabled so we can ignore the warnings.
        #[allow(unused_variables)]
        let total_bytes = 32 * writes_counter + code_bytes_written;
        #[allow(unused_assignments)]
        if code_bytes_written > 0 {
            writes_counter += 1;
        }
        sdk::log!(crate::prelude::format!("total_writes_count {}", writes_counter).as_str());
        sdk::log!(crate::prelude::format!("total_written_bytes {}", total_bytes).as_str());
    }
}

#[cfg(test)]
mod tests {}
