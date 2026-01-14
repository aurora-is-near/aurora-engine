use aurora_engine_modexp::{AuroraModExp, ModExpAlgorithm};
use aurora_engine_precompiles::PrecompileConstructorContext;
use aurora_engine_sdk::{
    caching::FullCache,
    env::Env,
    io::{IO, StorageIntermediate},
    promise::{PromiseHandler, PromiseId, ReadOnlyPromiseHandler},
};
use aurora_engine_types::parameters::connector::FtTransferMessageData;
use aurora_engine_types::parameters::connector::errors::ParseOnTransferMessageError;
use aurora_engine_types::{
    PhantomData,
    parameters::{
        connector::{Erc20Identifier, Erc20Metadata, FtOnTransferArgs, MirrorErc20TokenArgs},
        engine::FunctionCallArgsV2,
    },
    public_key::PublicKey,
    types::EthGas,
};
use aurora_evm::executor::stack::Authorization;
use aurora_evm::{
    Config, CreateScheme, ExitError, ExitFatal, ExitReason, Opcode,
    backend::{Apply, ApplyBackend, Backend, Basic, Log},
    executor,
};
use core::cell::RefCell;
use core::iter::once;

use crate::contract_methods::{ContractError, silo};
use crate::map::BijectionMap;
use crate::parameters::TransactionStatus;
use crate::parameters::{CallArgs, ResultLog, SubmitArgs, SubmitResult, ViewCallArgs};
use crate::pausables::{
    EngineAuthorizer, EnginePrecompilesPauser, PausedPrecompilesChecker, PrecompileFlags,
};
use crate::prelude::parameters::connector::RefundCallArgs;
use crate::prelude::precompiles::Precompiles;
use crate::prelude::precompiles::native::{exit_to_ethereum, exit_to_near};
use crate::prelude::precompiles::xcc::cross_contract_call;
use crate::prelude::transactions::{EthTransactionKind, NormalizedEthTransaction};
use crate::prelude::{
    AccountId, Address, BTreeMap, BorshDeserialize, ERC20_DECIMALS_SELECTOR, ERC20_MINT_SELECTOR,
    ERC20_NAME_SELECTOR, ERC20_SET_METADATA_SELECTOR, ERC20_SYMBOL_SELECTOR, H160, H256, KeyPrefix,
    PromiseArgs, PromiseCreateArgs, String, U256, Vec, Wei, Yocto, address_to_key, bytes_to_key,
    format, sdk, storage_to_key, u256_to_arr, vec,
};
use crate::state;
use crate::state::EngineState;
use crate::{accounting, errors};

/// Used as the first byte in the concatenation of data used to compute the blockhash.
/// Could be useful in the future as a version byte, or to distinguish different types of blocks.
const BLOCK_HASH_PREFIX: u8 = 0;
const BLOCK_HASH_PREFIX_SIZE: usize = 1;
const BLOCK_HEIGHT_SIZE: usize = 8;
const CHAIN_ID_SIZE: usize = 32;

/// Block height where the bug fix for parsing transactions to the zero address
/// is deployed. The current value is only approximate; will be updated once the
/// fix is actually deployed.
pub const ZERO_ADDRESS_FIX_HEIGHT: u64 = 61_200_152;

#[must_use]
pub fn current_address(current_account_id: &AccountId) -> Address {
    aurora_engine_sdk::types::near_account_to_evm_address(current_account_id.as_bytes())
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize))]
pub struct EngineError {
    pub kind: EngineErrorKind,
    pub gas_used: u64,
}

impl From<EngineErrorKind> for EngineError {
    fn from(kind: EngineErrorKind) -> Self {
        Self { kind, gas_used: 0 }
    }
}

impl AsRef<[u8]> for EngineError {
    fn as_ref(&self) -> &[u8] {
        self.kind.as_bytes()
    }
}

/// Errors with the EVM engine.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize))]
pub enum EngineErrorKind {
    /// Normal EVM errors.
    EvmError(ExitError),
    /// Fatal EVM errors.
    EvmFatal(ExitFatal),
    /// Incorrect nonce.
    IncorrectNonce(String),
    FailedTransactionParse(crate::prelude::transactions::Error),
    InvalidChainId,
    InvalidSignature,
    IntrinsicGasNotMet,
    MaxPriorityGasFeeTooLarge,
    GasPayment(GasPaymentError),
    GasOverflow,
    FixedGasOverflow,
    NotAllowed,
    SameOwner,
    NotOwner,
    NonExistedKey,
    Erc20FromNep141,
    RejectCallerWithCode,
}

impl EngineErrorKind {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::EvmError(ExitError::StackUnderflow) => errors::ERR_STACK_UNDERFLOW,
            Self::EvmError(ExitError::StackOverflow) => errors::ERR_STACK_OVERFLOW,
            Self::EvmError(ExitError::InvalidJump) => errors::ERR_INVALID_JUMP,
            Self::EvmError(ExitError::InvalidRange) => errors::ERR_INVALID_RANGE,
            Self::EvmError(ExitError::DesignatedInvalid) => errors::ERR_DESIGNATED_INVALID,
            Self::EvmError(ExitError::CallTooDeep) => errors::ERR_CALL_TOO_DEEP,
            Self::EvmError(ExitError::CreateCollision) => errors::ERR_CREATE_COLLISION,
            Self::EvmError(ExitError::CreateContractLimit) => errors::ERR_CREATE_CONTRACT_LIMIT,
            Self::EvmError(ExitError::InvalidCode(_)) => errors::ERR_INVALID_CODE,
            Self::EvmError(ExitError::OutOfOffset) => errors::ERR_OUT_OF_OFFSET,
            Self::EvmError(ExitError::OutOfGas) => errors::ERR_OUT_OF_GAS,
            Self::EvmError(ExitError::OutOfFund) => errors::ERR_OUT_OF_FUND,
            Self::EvmError(ExitError::CreateEmpty) => errors::ERR_CREATE_EMPTY,
            Self::EvmError(ExitError::MaxNonce) => errors::ERR_MAX_NONCE,
            Self::EvmFatal(ExitFatal::NotSupported) => errors::ERR_NOT_SUPPORTED,
            Self::EvmFatal(ExitFatal::UnhandledInterrupt) => errors::ERR_UNHANDLED_INTERRUPT,
            Self::EvmError(ExitError::Other(m)) | Self::EvmFatal(ExitFatal::Other(m)) => {
                m.as_bytes()
            }
            Self::IncorrectNonce(msg) => msg.as_bytes(),
            Self::FailedTransactionParse(e) => e.as_ref(),
            Self::InvalidChainId => errors::ERR_INVALID_CHAIN_ID,
            Self::InvalidSignature => errors::ERR_INVALID_ECDSA_SIGNATURE,
            Self::IntrinsicGasNotMet => errors::ERR_INTRINSIC_GAS,
            Self::MaxPriorityGasFeeTooLarge => errors::ERR_MAX_PRIORITY_FEE_GREATER,
            Self::GasPayment(e) => e.as_ref(),
            Self::GasOverflow => errors::ERR_GAS_OVERFLOW,
            Self::FixedGasOverflow => errors::ERR_FIXED_GAS_OVERFLOW,
            Self::NotAllowed => errors::ERR_NOT_ALLOWED,
            Self::SameOwner => errors::ERR_SAME_OWNER,
            Self::NotOwner => errors::ERR_NOT_OWNER,
            Self::NonExistedKey => errors::ERR_FUNCTION_CALL_KEY_NOT_FOUND,
            Self::Erc20FromNep141 => errors::ERR_GETTING_ERC20_FROM_NEP141,
            Self::RejectCallerWithCode => errors::ERR_REJECT_CALL_WITH_CODE,
            Self::EvmFatal(_) | Self::EvmError(_) => unreachable!(), // unused misc
        }
    }
}

impl AsRef<[u8]> for EngineErrorKind {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<ExitError> for EngineErrorKind {
    fn from(e: ExitError) -> Self {
        Self::EvmError(e)
    }
}

impl From<ExitFatal> for EngineErrorKind {
    fn from(e: ExitFatal) -> Self {
        Self::EvmFatal(e)
    }
}

/// An engine result.
pub type EngineResult<T> = Result<T, EngineError>;

trait ExitIntoResult {
    /// Checks if the EVM exit is ok or an error.
    fn into_result(self, data: Vec<u8>) -> Result<TransactionStatus, EngineErrorKind>;
}

impl ExitIntoResult for ExitReason {
    /// We should be aligned to Ethereum's gas charging:
    /// - `Success` | `Revert`
    /// - `ExitError` - Execution errors should charge gas from users
    /// - `ExitFatal` - shouldn't charge user gas
    ///
    /// NOTE: Transactions validation errors should not charge user gas
    fn into_result(self, data: Vec<u8>) -> Result<TransactionStatus, EngineErrorKind> {
        match self {
            Self::Succeed(_) => Ok(TransactionStatus::Succeed(data)),
            Self::Revert(_) => Ok(TransactionStatus::Revert(data)),
            Self::Error(ExitError::OutOfOffset) => Ok(TransactionStatus::OutOfOffset),
            Self::Error(ExitError::OutOfFund) => Ok(TransactionStatus::OutOfFund),
            Self::Error(ExitError::OutOfGas) => Ok(TransactionStatus::OutOfGas),
            Self::Error(ExitError::CallTooDeep) => Ok(TransactionStatus::CallTooDeep),
            Self::Error(ExitError::StackUnderflow) => Ok(TransactionStatus::StackUnderflow),
            Self::Error(ExitError::StackOverflow) => Ok(TransactionStatus::StackOverflow),
            Self::Error(ExitError::InvalidJump) => Ok(TransactionStatus::InvalidJump),
            Self::Error(ExitError::InvalidRange) => Ok(TransactionStatus::InvalidRange),
            Self::Error(ExitError::DesignatedInvalid) => Ok(TransactionStatus::DesignatedInvalid),
            Self::Error(ExitError::CreateCollision) => Ok(TransactionStatus::CreateCollision),
            Self::Error(ExitError::CreateContractLimit) => {
                Ok(TransactionStatus::CreateContractLimit)
            }
            Self::Error(ExitError::InvalidCode(opcode)) => {
                Ok(TransactionStatus::InvalidCode(opcode.0))
            }
            Self::Error(ExitError::PCUnderflow) => Ok(TransactionStatus::PCUnderflow),
            Self::Error(ExitError::CreateEmpty) => Ok(TransactionStatus::CreateEmpty),
            Self::Error(ExitError::MaxNonce) => Ok(TransactionStatus::MaxNonce),
            Self::Error(ExitError::UsizeOverflow) => Ok(TransactionStatus::UsizeOverflow),
            Self::Error(ExitError::CreateContractStartingWithEF) => {
                Ok(TransactionStatus::CreateContractStartingWithEF)
            }
            Self::Error(ExitError::Other(msg)) => Ok(TransactionStatus::Other(msg)),
            Self::Fatal(e) => Err(e.into()),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BalanceOverflow;

impl AsRef<[u8]> for BalanceOverflow {
    fn as_ref(&self) -> &[u8] {
        errors::ERR_BALANCE_OVERFLOW
    }
}

/// Errors resulting from trying to pay for gas
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GasPaymentError {
    /// Overflow adding ETH to an account balance (should never happen)
    BalanceOverflow(BalanceOverflow),
    /// Overflow in `gas * gas_price` calculation
    EthAmountOverflow,
    /// Not enough balance for account to cover the gas cost
    OutOfFund,
}

impl AsRef<[u8]> for GasPaymentError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::BalanceOverflow(overflow) => overflow.as_ref(),
            Self::EthAmountOverflow => errors::ERR_GAS_ETH_AMOUNT_OVERFLOW,
            Self::OutOfFund => errors::ERR_OUT_OF_FUND,
        }
    }
}

impl From<BalanceOverflow> for GasPaymentError {
    fn from(overflow: BalanceOverflow) -> Self {
        Self::BalanceOverflow(overflow)
    }
}

#[derive(Debug)]
pub enum DeployErc20Error {
    State(state::EngineStateError),
    Failed(TransactionStatus),
    Engine(EngineError),
    Register(RegisterTokenError),
}

impl AsRef<[u8]> for DeployErc20Error {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::State(e) => e.as_ref(),
            Self::Failed(e) => e.as_ref(),
            Self::Engine(e) => e.as_ref(),
            Self::Register(e) => e.as_ref(),
        }
    }
}

pub struct ERC20Address(pub Address);

impl AsRef<[u8]> for ERC20Address {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl TryFrom<Vec<u8>> for ERC20Address {
    type Error = AddressParseError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() == 20 {
            Ok(Self(
                Address::try_from_slice(&bytes).map_err(|_| AddressParseError)?,
            ))
        } else {
            Err(AddressParseError)
        }
    }
}

pub struct AddressParseError;

impl AsRef<[u8]> for AddressParseError {
    fn as_ref(&self) -> &[u8] {
        errors::ERR_PARSE_ADDRESS
    }
}

pub struct NEP141Account(pub AccountId);

impl AsRef<[u8]> for NEP141Account {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl TryFrom<Vec<u8>> for NEP141Account {
    type Error = aurora_engine_types::account_id::ParseAccountError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        AccountId::try_from(bytes).map(Self)
    }
}

#[derive(Debug)]
pub enum GetErc20FromNep141Error {
    InvalidNep141AccountId,
    Nep141NotFound,
    InvalidAddress,
}

impl AsRef<[u8]> for GetErc20FromNep141Error {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::InvalidNep141AccountId => errors::ERR_INVALID_NEP141_ACCOUNT_ID,
            Self::Nep141NotFound => errors::ERR_NEP141_NOT_FOUND,
            Self::InvalidAddress => errors::ERR_PARSE_ADDRESS,
        }
    }
}

#[derive(Debug)]
pub enum RegisterTokenError {
    InvalidNep141AccountId,
    TokenAlreadyRegistered,
    InvalidAddress,
}

impl AsRef<[u8]> for RegisterTokenError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::InvalidNep141AccountId => errors::ERR_INVALID_NEP141_ACCOUNT_ID,
            Self::TokenAlreadyRegistered => errors::ERR_NEP141_TOKEN_ALREADY_REGISTERED,
            Self::InvalidAddress => errors::ERR_PARSE_ADDRESS,
        }
    }
}

#[derive(Debug)]
pub enum ReadMetadataError {
    DecodeError,
    WrongType,
    NoValue,
    Nep141NotFound,
    EngineError(EngineErrorKind),
}

impl AsRef<[u8]> for ReadMetadataError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::DecodeError => errors::ERR_DECODING_TOKEN,
            Self::WrongType => errors::ERR_WRONG_TOKEN_TYPE,
            Self::NoValue => errors::ERR_TOKEN_NO_VALUE,
            Self::Nep141NotFound => errors::ERR_NEP141_NOT_FOUND,
            Self::EngineError(e) => e.as_ref(),
        }
    }
}

pub struct StackExecutorParams<'a, I, E, H> {
    precompiles: Precompiles<'a, I, E, H>,
    gas_limit: u64,
}

impl<'env, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> StackExecutorParams<'env, I, E, H> {
    const fn new(gas_limit: u64, precompiles: Precompiles<'env, I, E, H>) -> Self {
        Self {
            precompiles,
            gas_limit,
        }
    }

    #[allow(clippy::type_complexity)]
    fn make_executor<'a, M: ModExpAlgorithm>(
        &'a self,
        engine: &'a Engine<'env, I, E, M>,
    ) -> executor::stack::StackExecutor<
        'static,
        'a,
        executor::stack::MemoryStackState<'a, 'static, Engine<'env, I, E, M>>,
        Precompiles<'env, I, E, H>,
    > {
        let metadata = executor::stack::StackSubstateMetadata::new(self.gas_limit, CONFIG);
        let state = executor::stack::MemoryStackState::new(metadata, engine);
        executor::stack::StackExecutor::new_with_precompiles(state, CONFIG, &self.precompiles)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GasPaymentResult {
    pub prepaid_amount: Wei,
    pub effective_gas_price: U256,
    pub priority_fee_per_gas: U256,
}

pub struct Engine<'env, I: IO, E: Env, M = AuroraModExp> {
    state: EngineState,
    origin: Address,
    gas_price: U256,
    current_account_id: AccountId,
    io: I,
    env: &'env E,
    generation_cache: RefCell<BTreeMap<Address, u32>>,
    account_info_cache: RefCell<FullCache<Address, Basic>>,
    contract_code_cache: RefCell<FullCache<Address, Vec<u8>>>,
    contract_storage_cache: RefCell<FullCache<(Address, H256), H256>>,
    modexp_algorithm: PhantomData<M>,
}

const CONFIG: &Config = &Config::prague();

impl<'env, I: IO + Copy, E: Env, M: ModExpAlgorithm> Engine<'env, I, E, M> {
    pub fn new(
        origin: Address,
        current_account_id: AccountId,
        io: I,
        env: &'env E,
    ) -> Result<Self, state::EngineStateError> {
        state::get_state(&io)
            .map(|state| Self::new_with_state(state, origin, current_account_id, io, env))
    }

    pub fn new_with_state(
        state: EngineState,
        origin: Address,
        current_account_id: AccountId,
        io: I,
        env: &'env E,
    ) -> Self {
        Self {
            state,
            origin,
            gas_price: U256::zero(),
            current_account_id,
            io,
            env,
            generation_cache: RefCell::new(BTreeMap::new()),
            account_info_cache: RefCell::new(FullCache::default()),
            contract_code_cache: RefCell::new(FullCache::default()),
            contract_storage_cache: RefCell::new(FullCache::default()),
            modexp_algorithm: PhantomData,
        }
    }

    pub fn charge_gas(
        &mut self,
        sender: &Address,
        transaction: &NormalizedEthTransaction,
        max_gas_price: Option<U256>,
        fixed_gas: Option<EthGas>,
    ) -> Result<GasPaymentResult, GasPaymentError> {
        if transaction.max_fee_per_gas.is_zero() && fixed_gas.is_none() {
            return Ok(GasPaymentResult::default());
        }

        let priority_fee_per_gas = transaction
            .max_priority_fee_per_gas
            .min(transaction.max_fee_per_gas - self.block_base_fee_per_gas());
        let priority_fee_per_gas = max_gas_price.map_or(priority_fee_per_gas, |price| {
            price.min(priority_fee_per_gas)
        });
        let effective_gas_price = priority_fee_per_gas + self.block_base_fee_per_gas();
        // First we try to use `fixed_gas`. At this point we already know that the `fixed_gas` is
        // less than the `gas_limit`. It allows to avoid refund unused gas to the sender later.
        let prepaid_amount = fixed_gas
            .map_or(transaction.gas_limit, EthGas::as_u256)
            .checked_mul(effective_gas_price)
            .map(Wei::new)
            .ok_or(GasPaymentError::EthAmountOverflow)?;

        let new_balance = get_balance(&self.io, sender)
            .checked_sub(prepaid_amount)
            .ok_or(GasPaymentError::OutOfFund)?;

        set_balance(&mut self.io, sender, &new_balance);

        self.gas_price = effective_gas_price;

        Ok(GasPaymentResult {
            prepaid_amount,
            effective_gas_price,
            priority_fee_per_gas,
        })
    }

    pub fn deploy_code_with_input<P: PromiseHandler>(
        &mut self,
        input: Vec<u8>,
        address: Option<Address>,
        handler: &mut P,
    ) -> EngineResult<SubmitResult> {
        let origin = Address::new(self.origin());
        let value = Wei::zero();
        self.deploy_code(origin, value, input, address, u64::MAX, Vec::new(), handler)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn deploy_code<P: PromiseHandler>(
        &mut self,
        origin: Address,
        value: Wei,
        input: Vec<u8>,
        address: Option<Address>,
        gas_limit: u64,
        access_list: Vec<(H160, Vec<H256>)>, // See EIP-2930
        handler: &mut P,
    ) -> EngineResult<SubmitResult> {
        let pause_flags = EnginePrecompilesPauser::from_io(self.io).paused();
        let precompiles = self.create_precompiles(pause_flags, handler);

        let executor_params = StackExecutorParams::new(gas_limit, precompiles);
        let mut executor = executor_params.make_executor(self);
        let scheme = address.map_or_else(
            || CreateScheme::Legacy {
                caller: origin.raw(),
            },
            |address| CreateScheme::Fixed(address.raw()),
        );
        let address = executor.create_address(scheme);
        let (exit_reason, return_value) = match scheme {
            CreateScheme::Legacy { caller } => {
                executor.transact_create(caller, value.raw(), input, gas_limit, access_list)
            }
            CreateScheme::Fixed(address) => executor.transact_create_fixed(
                origin.raw(),
                address,
                value.raw(),
                input,
                gas_limit,
                access_list,
            ),
            CreateScheme::Create2 { .. } => unreachable!(),
        };
        let result = if exit_reason.is_succeed() {
            address.0.to_vec()
        } else {
            return_value
        };

        let used_gas = executor.used_gas();
        let status = exit_reason.into_result(result)?;

        let (values, logs) = executor.into_state().deconstruct();
        let logs = filter_promises_from_logs(&self.io, handler, logs, &self.current_account_id);

        self.apply(values, Vec::<Log>::new(), true);

        Ok(SubmitResult::new(status, used_gas, logs))
    }

    /// Call the EVM contract with arguments
    pub fn call_with_args<P: PromiseHandler>(
        &mut self,
        args: CallArgs,
        handler: &mut P,
    ) -> EngineResult<SubmitResult> {
        let origin = Address::new(self.origin());
        match args {
            CallArgs::V2(call_args) => {
                let contract = call_args.contract;
                let value = call_args.value.into();
                let input = call_args.input;
                self.call(
                    &origin,
                    &contract,
                    value,
                    input,
                    u64::MAX,
                    Vec::new(),
                    Vec::new(),
                    handler,
                )
            }
            CallArgs::V1(call_args) => {
                let contract = call_args.contract;
                let value = Wei::zero();
                let input = call_args.input;
                self.call(
                    &origin,
                    &contract,
                    value,
                    input,
                    u64::MAX,
                    Vec::new(),
                    Vec::new(),
                    handler,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn call<P: PromiseHandler>(
        &mut self,
        origin: &Address,
        contract: &Address,
        value: Wei,
        input: Vec<u8>,
        gas_limit: u64,
        access_list: Vec<(H160, Vec<H256>)>,    // See EIP-2930
        authorization_list: Vec<Authorization>, // See EIP-7702
        handler: &mut P,
    ) -> EngineResult<SubmitResult> {
        let pause_flags = EnginePrecompilesPauser::from_io(self.io).paused();
        let precompiles = self.create_precompiles(pause_flags, handler);

        let executor_params = StackExecutorParams::new(gas_limit, precompiles);
        let mut executor = executor_params.make_executor(self);
        let (exit_reason, result) = executor.transact_call(
            origin.raw(),
            contract.raw(),
            value.raw(),
            input,
            gas_limit,
            access_list,
            authorization_list,
        );

        let used_gas = executor.used_gas();
        let status = exit_reason.into_result(result)?;

        let (values, logs) = executor.into_state().deconstruct();
        let logs = filter_promises_from_logs(&self.io, handler, logs, &self.current_account_id);
        // The logs could be encoded as base64 or hex string.
        self.apply(values, Vec::<Log>::new(), true);

        Ok(SubmitResult::new(status, used_gas, logs))
    }

    pub fn view_with_args(&self, args: ViewCallArgs) -> Result<TransactionStatus, EngineErrorKind> {
        let origin = &args.sender;
        let contract = &args.address;
        let value = U256::from_big_endian(&args.amount);
        // View calls cannot interact with promises
        let handler = aurora_engine_sdk::promise::Noop;
        let pause_flags = EnginePrecompilesPauser::from_io(self.io).paused();
        let precompiles = self.create_precompiles(pause_flags, &handler);

        let executor_params = StackExecutorParams::new(u64::MAX, precompiles);
        self.view(
            origin,
            contract,
            Wei::new(value),
            args.input,
            &executor_params,
        )
    }

    pub fn view(
        &self,
        origin: &Address,
        contract: &Address,
        value: Wei,
        input: Vec<u8>,
        executor_params: &StackExecutorParams<I, E, aurora_engine_sdk::promise::Noop>,
    ) -> Result<TransactionStatus, EngineErrorKind> {
        let mut executor = executor_params.make_executor(self);
        let (status, result) = executor.transact_call(
            origin.raw(),
            contract.raw(),
            value.raw(),
            input,
            executor_params.gas_limit,
            Vec::new(),
            Vec::new(),
        );
        status.into_result(result)
    }

    fn relayer_key(account_id: &[u8]) -> Vec<u8> {
        bytes_to_key(KeyPrefix::RelayerEvmAddressMap, account_id)
    }

    pub fn register_relayer(&mut self, account_id: &[u8], evm_address: Address) {
        let key = Self::relayer_key(account_id);
        self.io.write_storage(&key, evm_address.as_bytes());
    }

    pub fn get_relayer(&self, account_id: &[u8]) -> Option<Address> {
        let key = Self::relayer_key(account_id);

        self.io.read_storage(&key).map(|v| {
            let mut buf = [0; 20];

            v.copy_to_slice(&mut buf);
            Address::from_array(buf)
        })
    }

    pub fn register_token(
        &mut self,
        erc20_token: Address,
        nep141_token: AccountId,
    ) -> Result<(), RegisterTokenError> {
        match get_erc20_from_nep141(&self.io, &nep141_token) {
            Err(GetErc20FromNep141Error::Nep141NotFound) => (),
            Err(GetErc20FromNep141Error::InvalidNep141AccountId) => {
                return Err(RegisterTokenError::InvalidNep141AccountId);
            }
            Err(GetErc20FromNep141Error::InvalidAddress) => {
                return Err(RegisterTokenError::InvalidAddress);
            }
            Ok(_) => return Err(RegisterTokenError::TokenAlreadyRegistered),
        }

        let erc20_token = ERC20Address(erc20_token);
        let nep141_token = NEP141Account(nep141_token);
        nep141_erc20_map(self.io).insert(&nep141_token, &erc20_token);
        Ok(())
    }

    /// Transfers an amount from a given sender to a receiver, provided that
    /// they have enough in their balance.
    ///
    /// If the sender can send, and the receiver can receive, then the transfer
    /// will execute successfully.
    pub fn transfer<P: PromiseHandler>(
        &mut self,
        sender: Address,
        receiver: Address,
        value: Wei,
        gas_limit: u64,
        handler: &mut P,
    ) -> EngineResult<SubmitResult> {
        self.call(
            &sender,
            &receiver,
            value,
            Vec::new(),
            gas_limit,
            Vec::new(),
            Vec::new(),
            handler,
        )
    }

    /// Mint base tokens for the recipient.
    ///
    /// IMPORTANT: This function should not panic, otherwise it won't
    /// be possible to return the tokens to the sender.
    pub fn receive_base_tokens(
        &mut self,
        args: &FtOnTransferArgs,
    ) -> Result<Option<SubmitResult>, ContractError> {
        let message_data = FtTransferMessageData::try_from(args.msg.as_str())?;
        let amount = Wei::new_u128(args.amount.as_u128());
        let receipient = message_data.recipient;
        let balance = get_balance(&self.io, &receipient);
        let new_balance = balance
            .checked_add(amount)
            .ok_or(errors::ERR_BALANCE_OVERFLOW)?;

        set_balance(&mut self.io, &receipient, &new_balance);

        sdk::log!("Mint {amount} base tokens for: {}", receipient.encode());

        Ok(None)
    }

    /// Mint tokens for recipient on a particular ERC-20 token.
    ///
    /// IMPORTANT: This function should not panic, otherwise it won't
    /// be possible to return the tokens to the sender.
    pub fn receive_erc20_tokens<P: PromiseHandler>(
        &mut self,
        token: &AccountId,
        args: &FtOnTransferArgs,
        current_account_id: &AccountId,
        handler: &mut P,
    ) -> Result<Option<SubmitResult>, ContractError> {
        let amount = args.amount.as_u128();
        // Parse message to determine recipient
        let mut recipient = {
            // The message should contain the recipient EOA address.
            let message = args.msg.strip_prefix("0x").unwrap_or(&args.msg);
            // Recipient - 40 characters (Address in hex without '0x' prefix)
            if message.len() < 40 {
                return Err(ParseOnTransferMessageError::WrongMessageFormat.into());
            }
            let mut address_bytes = [0; 20];
            hex::decode_to_slice(&message[..40], &mut address_bytes)
                .map_err(|_| ParseOnTransferMessageError::WrongMessageFormat)?;
            Address::from_array(address_bytes)
        };

        if let Some(fallback_address) = silo::get_erc20_fallback_address(&self.io) {
            if !silo::is_allow_receive_erc20_tokens(&self.io, &recipient) {
                recipient = fallback_address;
            }
        }

        let erc20_token = get_erc20_from_nep141(&self.io, token)?;
        let erc20_admin_address = current_address(current_account_id);
        let result = self
            .call(
                &erc20_admin_address,
                &erc20_token,
                Wei::zero(),
                setup_receive_erc20_tokens_input(&recipient, amount),
                u64::MAX,
                Vec::new(), // TODO: are there values we should put here?
                Vec::new(),
                handler,
            )
            .and_then(submit_result_or_err)?;

        sdk::log!("Mint {amount} ERC-20 tokens for: {}", recipient.encode());

        // Return SubmitResult so that it can be accessed in standalone engine.
        // This is used to help with the indexing of bridge transactions.
        Ok(Some(result))
    }

    /// Read metadata of ERC-20 contract.
    pub fn get_erc20_metadata(
        &self,
        erc20_identifier: &Erc20Identifier,
    ) -> Result<Erc20Metadata, ReadMetadataError> {
        let erc20_address = self
            .identifier_to_address(erc20_identifier)
            .map_err(|_| ReadMetadataError::Nep141NotFound)?;
        let name = self
            .view_with_selector(
                erc20_address,
                ERC20_NAME_SELECTOR,
                &[ethabi::ParamType::String],
            )?
            .into_string()
            .ok_or(ReadMetadataError::WrongType)?;
        let symbol = self
            .view_with_selector(
                erc20_address,
                ERC20_SYMBOL_SELECTOR,
                &[ethabi::ParamType::String],
            )?
            .into_string()
            .ok_or(ReadMetadataError::WrongType)?;
        let decimals = self
            .view_with_selector(
                erc20_address,
                ERC20_DECIMALS_SELECTOR,
                &[ethabi::ParamType::Uint(8)],
            )?
            .into_uint()
            .ok_or(ReadMetadataError::WrongType)?
            .try_into()
            .map_err(|_| ReadMetadataError::WrongType)?;

        Ok(Erc20Metadata {
            name,
            symbol,
            decimals,
        })
    }

    /// Set metadata of ERC-20 contract.
    pub fn set_erc20_metadata<P: PromiseHandler>(
        &mut self,
        erc20_identifier: &Erc20Identifier,
        erc20_metadata: Erc20Metadata,
        handler: &mut P,
    ) -> EngineResult<SubmitResult> {
        let erc20_address = self
            .identifier_to_address(erc20_identifier)
            .map_err(|_| EngineErrorKind::Erc20FromNep141)?;
        let args = ethabi::encode(&[
            ethabi::Token::String(erc20_metadata.name),
            ethabi::Token::String(erc20_metadata.symbol),
            ethabi::Token::Uint(erc20_metadata.decimals.into()),
        ]);
        let input = [ERC20_SET_METADATA_SELECTOR, &args].concat();

        self.call_with_args(
            CallArgs::V2(FunctionCallArgsV2 {
                contract: erc20_address,
                value: [0; 32],
                input,
            }),
            handler,
        )
    }

    fn create_precompiles<P: PromiseHandler>(
        &self,
        pause_flags: PrecompileFlags,
        handler: &P,
    ) -> Precompiles<'env, I, E, P::ReadOnly> {
        let current_account_id = self.current_account_id.clone();
        let random_seed = self.env.random_seed();
        let io = self.io;
        let env = self.env;
        let ro_promise_handler = handler.read_only();

        let precompiles = Precompiles::new_prague(PrecompileConstructorContext {
            current_account_id,
            random_seed,
            io,
            env,
            promise_handler: ro_promise_handler,
            mod_exp_algorithm: self.modexp_algorithm,
        });

        Self::apply_pause_flags_to_precompiles(precompiles, pause_flags)
    }

    fn apply_pause_flags_to_precompiles<H: ReadOnlyPromiseHandler>(
        precompiles: Precompiles<'env, I, E, H>,
        pause_flags: PrecompileFlags,
    ) -> Precompiles<'env, I, E, H> {
        Precompiles {
            paused_precompiles: precompiles
                .all_precompiles
                .keys()
                .filter(|address| pause_flags.is_paused_by_address(address))
                .copied()
                .collect(),
            all_precompiles: precompiles.all_precompiles,
        }
    }

    fn view_with_selector(
        &self,
        contract_address: Address,
        selector: &[u8],
        output_types: &[ethabi::ParamType],
    ) -> Result<ethabi::Token, ReadMetadataError> {
        let result = self.view_with_args(ViewCallArgs {
            sender: self.origin,
            address: contract_address,
            amount: [0; 32],
            input: selector.to_vec(),
        });

        let output = match result.map_err(ReadMetadataError::EngineError)? {
            TransactionStatus::Succeed(bytes) => bytes,
            _ => Vec::new(),
        };

        ethabi::decode(output_types, &output)
            .map_err(|_| ReadMetadataError::DecodeError)?
            .pop()
            .ok_or(ReadMetadataError::NoValue)
    }

    fn identifier_to_address(
        &self,
        identifier: &Erc20Identifier,
    ) -> Result<Address, GetErc20FromNep141Error> {
        match identifier {
            Erc20Identifier::Erc20 { address } => Ok(*address),
            Erc20Identifier::Nep141 { account_id } => get_erc20_from_nep141(&self.io, account_id),
        }
    }
}

pub fn submit<I: IO + Copy, E: Env, P: PromiseHandler>(
    io: I,
    env: &E,
    args: &SubmitArgs,
    state: EngineState,
    current_account_id: AccountId,
    relayer_address: Address,
    handler: &mut P,
) -> EngineResult<SubmitResult> {
    submit_with_alt_modexp::<_, _, _, AuroraModExp>(
        io,
        env,
        args,
        state,
        current_account_id,
        relayer_address,
        handler,
    )
}

#[allow(clippy::too_many_lines)]
pub fn submit_with_alt_modexp<
    I: IO + Copy,
    E: Env,
    P: PromiseHandler,
    M: ModExpAlgorithm + 'static,
>(
    mut io: I,
    env: &E,
    args: &SubmitArgs,
    state: EngineState,
    current_account_id: AccountId,
    relayer_address: Address,
    handler: &mut P,
) -> EngineResult<SubmitResult> {
    #[cfg(feature = "contract")]
    let transaction = NormalizedEthTransaction::try_from(
        EthTransactionKind::try_from(args.tx_data.as_slice())
            .map_err(EngineErrorKind::FailedTransactionParse)?,
    )
    .map_err(|_e| EngineErrorKind::InvalidSignature)?;

    #[cfg(not(feature = "contract"))]
    // The standalone engine must use the backwards compatible parser to reproduce the NEAR state,
    // but the contract itself does not need to make such checks because it never executes historical
    // transactions.
    let transaction: NormalizedEthTransaction = {
        let adapter =
            aurora_engine_transactions::backwards_compatibility::EthTransactionKindAdapter::new(
                ZERO_ADDRESS_FIX_HEIGHT,
            );
        let block_height = env.block_height();
        let tx: EthTransactionKind = adapter
            .try_parse_bytes(args.tx_data.as_slice(), block_height)
            .map_err(EngineErrorKind::FailedTransactionParse)?;
        tx.try_into()
            .map_err(|_e| EngineErrorKind::InvalidSignature)?
    };
    // Retrieve the signer of the transaction:
    let sender = transaction.address;

    let fixed_gas = silo::get_fixed_gas(&io);

    // Check if the sender has rights to submit transactions or deploy code.
    assert_access(&io, env, &transaction)?;

    // Validate the chain ID, if provided inside the signature:
    if let Some(chain_id) = transaction.chain_id {
        if U256::from(chain_id) != U256::from_big_endian(&state.chain_id) {
            return Err(EngineErrorKind::InvalidChainId.into());
        }
    }

    sdk::log!("signer_address {:?}", sender);

    check_nonce(&io, &sender, &transaction.nonce)?;

    // Check that fixed gas is not greater than gasLimit from the transaction.
    if fixed_gas.is_some_and(|gas| gas.as_u256() > transaction.gas_limit) {
        return Err(EngineErrorKind::FixedGasOverflow.into());
    }

    // Check intrinsic gas is covered by transaction gas limit
    match transaction.intrinsic_gas(CONFIG) {
        Err(_e) => {
            return Err(EngineErrorKind::GasOverflow.into());
        }
        Ok(intrinsic_gas) => {
            if transaction.gas_limit < intrinsic_gas.into() {
                return Err(EngineErrorKind::IntrinsicGasNotMet.into());
            }
        }
    }

    if transaction.max_priority_fee_per_gas > transaction.max_fee_per_gas {
        return Err(EngineErrorKind::MaxPriorityGasFeeTooLarge.into());
    }

    let mut engine: Engine<_, _, M> =
        Engine::new_with_state(state, sender, current_account_id, io, env);
    // EIP-3607
    if !engine.code(sender.raw()).is_empty() {
        return Err(EngineErrorKind::RejectCallerWithCode.into());
    }
    let max_gas_price = args.max_gas_price.map(Into::into);
    let prepaid_amount = match engine.charge_gas(&sender, &transaction, max_gas_price, fixed_gas) {
        Ok(gas_result) => gas_result,
        Err(err) => {
            return Err(EngineErrorKind::GasPayment(err).into());
        }
    };
    let gas_limit = transaction
        .gas_limit
        .try_into()
        .map_err(|_| EngineErrorKind::GasOverflow)?;
    let access_list = transaction
        .access_list
        .into_iter()
        .map(|a| (a.address, a.storage_keys))
        .collect();
    let result = if let Some(receiver) = transaction.to {
        engine.call(
            &sender,
            &receiver,
            transaction.value,
            transaction.data,
            gas_limit,
            access_list,
            transaction.authorization_list,
            handler,
        )
        // TODO: charge for storage
    } else {
        // Execute a contract deployment:
        engine.deploy_code(
            sender,
            transaction.value,
            transaction.data,
            None,
            gas_limit,
            access_list,
            handler,
        )
        // TODO: charge for storage
    };

    // Give refund.
    let gas_used = match &result {
        Ok(submit_result) => submit_result.gas_used,
        Err(engine_err) => engine_err.gas_used,
    };

    refund_unused_gas(
        &mut io,
        &sender,
        gas_used,
        &prepaid_amount,
        &relayer_address,
        fixed_gas,
    )
    .map_err(|e| EngineError {
        gas_used,
        kind: EngineErrorKind::GasPayment(e),
    })?;

    // return result to user
    result
}

#[must_use]
pub fn setup_refund_on_error_input(amount: U256, refund_address: Address) -> Vec<u8> {
    let selector = ERC20_MINT_SELECTOR;
    let mint_args = ethabi::encode(&[
        ethabi::Token::Address(refund_address.raw().0.into()),
        ethabi::Token::Uint(amount.to_big_endian().into()),
    ]);

    [selector, mint_args.as_slice()].concat()
}

pub fn refund_on_error<I: IO + Copy, E: Env, P: PromiseHandler>(
    io: I,
    env: &E,
    state: EngineState,
    args: &RefundCallArgs,
    handler: &mut P,
) -> EngineResult<SubmitResult> {
    let current_account_id = env.current_account_id();
    if let Some(erc20_address) = args.erc20_address {
        // ERC-20 exit; re-mint burned tokens
        let erc20_admin_address = current_address(&current_account_id);
        let mut engine: Engine<_, _> =
            Engine::new_with_state(state, erc20_admin_address, current_account_id, io, env);

        let refund_address = args.recipient_address;
        let amount = U256::from_big_endian(&args.amount);
        let input = setup_refund_on_error_input(amount, refund_address);

        engine.call(
            &erc20_admin_address,
            &erc20_address,
            Wei::zero(),
            input,
            u64::MAX,
            Vec::new(),
            Vec::new(),
            handler,
        )
    } else {
        // ETH exit; transfer ETH back from precompile address
        let exit_address = exit_to_near::ADDRESS;
        let mut engine: Engine<_, _> =
            Engine::new_with_state(state, exit_address, current_account_id, io, env);
        let refund_address = args.recipient_address;
        let amount = Wei::new(U256::from_big_endian(&args.amount));
        engine.call(
            &exit_address,
            &refund_address,
            amount,
            Vec::new(),
            u64::MAX,
            vec![
                (exit_address.raw(), Vec::new()),
                (refund_address.raw(), Vec::new()),
            ],
            Vec::new(),
            handler,
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
#[must_use]
pub fn compute_block_hash(chain_id: [u8; 32], block_height: u64, account_id: &[u8]) -> H256 {
    debug_assert_eq!(BLOCK_HASH_PREFIX_SIZE, size_of_val(&BLOCK_HASH_PREFIX));
    debug_assert_eq!(BLOCK_HEIGHT_SIZE, size_of_val(&block_height));
    debug_assert_eq!(CHAIN_ID_SIZE, size_of_val(&chain_id));
    let mut data = Vec::with_capacity(
        BLOCK_HASH_PREFIX_SIZE + BLOCK_HEIGHT_SIZE + CHAIN_ID_SIZE + account_id.len(),
    );
    data.push(BLOCK_HASH_PREFIX);
    data.extend_from_slice(&chain_id);
    data.extend_from_slice(account_id);
    data.extend_from_slice(&block_height.to_be_bytes());

    sdk::sha256(&data)
}

#[must_use]
pub fn get_authorizer<I: IO + Copy>(io: &I) -> EngineAuthorizer {
    // TODO: a temporary use the owner account only until the engine adapts std with near-plugins
    state::get_state(io)
        .map(|state| EngineAuthorizer::from_accounts(once(state.owner_id)))
        .unwrap_or_default()
}

pub fn refund_unused_gas<I: IO>(
    io: &mut I,
    sender: &Address,
    gas_used: u64,
    gas_result: &GasPaymentResult,
    relayer: &Address,
    fixed_gas: Option<EthGas>,
) -> Result<(), GasPaymentError> {
    if gas_result.effective_gas_price.is_zero() {
        return Ok(());
    }

    let (refund, relayer_reward) = {
        let gas_to_wei = |price: U256| {
            fixed_gas
                .map_or_else(|| gas_used.into(), EthGas::as_u256)
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

        (refund, reward_amount)
    };

    if !refund.is_zero() {
        add_balance(io, sender, refund)?;
    }

    if !relayer_reward.is_zero() {
        add_balance(io, relayer, relayer_reward)?;
    }

    Ok(())
}

#[must_use]
pub fn setup_receive_erc20_tokens_input(recipient: &Address, amount: u128) -> Vec<u8> {
    let selector = ERC20_MINT_SELECTOR;
    let tail = ethabi::encode(&[
        ethabi::Token::Address(recipient.raw().0.into()),
        ethabi::Token::Uint(amount.into()),
    ]);

    [selector, tail.as_slice()].concat()
}

#[must_use]
pub fn setup_deploy_erc20_input(
    current_account_id: &AccountId,
    erc20_metadata: Option<Erc20Metadata>,
) -> Vec<u8> {
    #[cfg(feature = "error_refund")]
    let erc20_contract = include_bytes!("../../etc/eth-contracts/res/EvmErc20V2.bin");
    #[cfg(not(feature = "error_refund"))]
    let erc20_contract = include_bytes!("../../etc/eth-contracts/res/EvmErc20.bin");

    let erc20_admin_address = current_address(current_account_id);
    let erc20_metadata = erc20_metadata.unwrap_or_default();

    let deploy_args = ethabi::encode(&[
        ethabi::Token::String(erc20_metadata.name),
        ethabi::Token::String(erc20_metadata.symbol),
        ethabi::Token::Uint(erc20_metadata.decimals.into()),
        ethabi::Token::Address(erc20_admin_address.raw().0.into()),
    ]);

    [erc20_contract, deploy_args.as_slice()].concat()
}

/// Used to bridge NEP-141 tokens from NEAR to Aurora. On Aurora the NEP-141 becomes an ERC-20.
pub fn deploy_erc20_token<I: IO + Copy, E: Env, P: PromiseHandler>(
    nep141: AccountId,
    metadata: Option<Erc20Metadata>,
    io: I,
    env: &E,
    handler: &mut P,
) -> Result<Address, DeployErc20Error> {
    let current_account_id = env.current_account_id();
    let input = setup_deploy_erc20_input(&current_account_id, metadata);
    let mut engine: Engine<_, _> = Engine::new(
        aurora_engine_sdk::types::near_account_to_evm_address(
            env.predecessor_account_id().as_bytes(),
        ),
        current_account_id,
        io,
        env,
    )
    .map_err(DeployErc20Error::State)?;

    let address = match engine.deploy_code_with_input(input, None, handler) {
        Ok(result) => match result.status {
            TransactionStatus::Succeed(ret) => {
                Address::new(H160(ret.as_slice().try_into().unwrap()))
            }
            other => return Err(DeployErc20Error::Failed(other)),
        },
        Err(e) => return Err(DeployErc20Error::Engine(e)),
    };

    sdk::log!("Deployed ERC-20 in Aurora at: {:#?}", address);
    engine
        .register_token(address, nep141)
        .map_err(DeployErc20Error::Register)?;

    Ok(address)
}

/// Used to mirror deployed ERC-20 contract on main contract to silo.
pub fn mirror_erc20_token<I: IO + Copy, E: Env, P: PromiseHandler>(
    args: MirrorErc20TokenArgs,
    erc20_address: Address,
    erc20_metadata: Erc20Metadata,
    io: I,
    env: &E,
    handler: &mut P,
) -> Result<Address, DeployErc20Error> {
    let current_account_id = env.current_account_id();
    let input = setup_deploy_erc20_input(&current_account_id, Some(erc20_metadata));
    let mut engine: Engine<_, _> = Engine::new(
        aurora_engine_sdk::types::near_account_to_evm_address(
            env.predecessor_account_id().as_bytes(),
        ),
        current_account_id,
        io,
        env,
    )
    .map_err(DeployErc20Error::State)?;

    let address = match engine.deploy_code_with_input(input, Some(erc20_address), handler) {
        Ok(result) => match result.status {
            TransactionStatus::Succeed(ret) => {
                Address::new(H160(ret.as_slice().try_into().unwrap()))
            }
            other => return Err(DeployErc20Error::Failed(other)),
        },
        Err(e) => return Err(DeployErc20Error::Engine(e)),
    };

    assert_eq!(address, erc20_address);

    sdk::log!(
        "ERC-20 on: {} at address: {} has been mirrored",
        args.contract_id.as_ref(),
        address.encode()
    );
    engine
        .register_token(address, args.nep141)
        .map_err(DeployErc20Error::Register)?;

    Ok(address)
}

pub fn set_code<I: IO>(io: &mut I, address: &Address, code: &[u8]) {
    io.write_storage(&address_to_key(KeyPrefix::Code, address), code);
}

pub fn remove_code<I: IO>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Code, address));
}

pub fn get_code<I: IO>(io: &I, address: &Address) -> Vec<u8> {
    io.read_storage(&address_to_key(KeyPrefix::Code, address))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

pub fn get_code_size<I: IO>(io: &I, address: &Address) -> usize {
    io.read_storage_len(&address_to_key(KeyPrefix::Code, address))
        .unwrap_or(0)
}

pub fn set_nonce<I: IO>(io: &mut I, address: &Address, nonce: &U256) {
    io.write_storage(
        &address_to_key(KeyPrefix::Nonce, address),
        &u256_to_arr(nonce),
    );
}

pub fn remove_nonce<I: IO>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Nonce, address));
}

/// Checks the nonce to ensure that the address matches the transaction
/// nonce.
#[inline]
pub fn check_nonce<I: IO>(
    io: &I,
    address: &Address,
    transaction_nonce: &U256,
) -> Result<(), EngineErrorKind> {
    let account_nonce = get_nonce(io, address);

    if transaction_nonce != &account_nonce {
        return Err(EngineErrorKind::IncorrectNonce(format!(
            "ERR_INCORRECT_NONCE: ac: {account_nonce}, tx: {transaction_nonce}"
        )));
    }

    Ok(())
}

pub fn get_nonce<I: IO>(io: &I, address: &Address) -> U256 {
    io.read_u256(&address_to_key(KeyPrefix::Nonce, address))
        .unwrap_or_else(|_| U256::zero())
}

#[cfg(test)]
pub fn increment_nonce<I: IO>(io: &mut I, address: &Address) {
    let account_nonce = get_nonce(io, address);
    let new_nonce = account_nonce.saturating_add(U256::one());
    set_nonce(io, address, &new_nonce);
}

#[must_use]
pub fn create_legacy_address(caller: &Address, nonce: &U256) -> Address {
    let mut stream = rlp::RlpStream::new_list(2);
    stream.append(&caller.raw());
    stream.append(nonce);
    let hash = aurora_engine_sdk::keccak(&stream.out());
    let hash_bytes = hash.as_bytes();
    Address::try_from_slice(&hash_bytes[12..]).unwrap()
}

#[must_use]
pub const fn nep141_erc20_map<I: IO>(io: I) -> BijectionMap<NEP141Account, ERC20Address, I> {
    BijectionMap::new(KeyPrefix::Nep141Erc20Map, KeyPrefix::Erc20Nep141Map, io)
}

pub fn get_erc20_from_nep141<I: IO>(
    io: &I,
    nep141_account_id: &AccountId,
) -> Result<Address, GetErc20FromNep141Error> {
    let key = bytes_to_key(KeyPrefix::Nep141Erc20Map, nep141_account_id.as_bytes());
    io.read_storage(&key)
        .map(|v| {
            let mut buf = [0u8; 20];
            v.copy_to_slice(&mut buf);
            Address::from_array(buf)
        })
        .ok_or(GetErc20FromNep141Error::Nep141NotFound)
}

pub fn add_balance<I: IO>(
    io: &mut I,
    address: &Address,
    amount: Wei,
) -> Result<(), BalanceOverflow> {
    let current_balance = get_balance(io, address);
    let new_balance = current_balance.checked_add(amount).ok_or(BalanceOverflow)?;
    set_balance(io, address, &new_balance);
    Ok(())
}

pub fn set_balance<I: IO>(io: &mut I, address: &Address, balance: &Wei) {
    io.write_storage(
        &address_to_key(KeyPrefix::Balance, address),
        &balance.to_bytes(),
    );
}

pub fn remove_balance<I: IO + Copy>(io: &mut I, address: &Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Balance, address));
}

pub fn get_balance<I: IO>(io: &I, address: &Address) -> Wei {
    let raw = io
        .read_u256(&address_to_key(KeyPrefix::Balance, address))
        .unwrap_or_else(|_| U256::zero());
    Wei::new(raw)
}

pub fn remove_storage<I: IO>(io: &mut I, address: &Address, key: &H256, generation: u32) {
    io.remove_storage(storage_to_key(address, key, generation).as_ref());
}

pub fn set_storage<I: IO>(
    io: &mut I,
    address: &Address,
    key: &H256,
    value: &H256,
    generation: u32,
) {
    io.write_storage(storage_to_key(address, key, generation).as_ref(), &value.0);
}

pub fn get_storage<I: IO>(io: &I, address: &Address, key: &H256, generation: u32) -> H256 {
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
        .unwrap_or_default()
}

pub fn storage_has_key<I: IO>(io: &I, address: &Address, key: &H256, generation: u32) -> bool {
    io.storage_has_key(storage_to_key(address, key, generation).as_ref())
}

/// EIP-7610: balance, nonce, code, storage should be empty
pub fn is_account_empty<I: IO>(io: &I, address: &Address) -> bool {
    get_balance(io, address).is_zero()
        && get_nonce(io, address).is_zero()
        && get_code_size(io, address) == 0
        && !storage_has_key(io, address, &H256::zero(), get_generation(io, address))
}

/// Increments storage generation for a given address.
pub fn set_generation<I: IO>(io: &mut I, address: &Address, generation: u32) {
    io.write_storage(
        &address_to_key(KeyPrefix::Generation, address),
        &generation.to_be_bytes(),
    );
}

pub fn get_generation<I: IO>(io: &I, address: &Address) -> u32 {
    io.read_storage(&address_to_key(KeyPrefix::Generation, address))
        .map_or(0, |value| {
            let mut bytes = [0u8; 4];
            value.copy_to_slice(&mut bytes);
            u32::from_be_bytes(bytes)
        })
}

/// Adds a public function call key for a relayer.
pub fn add_function_call_key<I: IO>(io: &mut I, key: &PublicKey) {
    let prefixed_key = bytes_to_key(KeyPrefix::RelayerFunctionCallKey, key.key_data());
    io.write_storage(&prefixed_key, &[1]);
}

/// Removes a public function call key for a relayer.
pub fn remove_function_call_key<I: IO>(io: &mut I, key: &PublicKey) -> Result<(), EngineError> {
    let prefixed_key = bytes_to_key(KeyPrefix::RelayerFunctionCallKey, key.key_data());
    io.remove_storage(&prefixed_key)
        .ok_or_else(|| EngineError::from(EngineErrorKind::NonExistedKey))?;

    Ok(())
}

/// Removes all storage for the given address.
fn remove_all_storage<I: IO>(io: &mut I, address: &Address, generation: u32) {
    // FIXME: there is presently no way to prefix delete trie state.
    // NOTE: There is not going to be a method on runtime for this.
    //     You may need to store all keys in a list if you want to do this in a contract.
    //     Maybe you can incentivize people to delete dead old keys. They can observe them from
    //     external indexer node and then issue special cleaning transaction.
    //     Either way you may have to store the nonce per storage address root. When the account
    //     has to be deleted the storage nonce needs to be increased, and the old nonce keys
    //     can be deleted over time. That's how TurboGeth does storage.
    set_generation(io, address, generation + 1);
}

/// Removes an account.
fn remove_account<I: IO + Copy>(io: &mut I, address: &Address, generation: u32) {
    remove_nonce(io, address);
    remove_balance(io, address);
    remove_code(io, address);
    remove_all_storage(io, address, generation);
}

fn filter_promises_from_logs<I, T, P>(
    io: &I,
    handler: &mut P,
    logs: T,
    current_account_id: &AccountId,
) -> Vec<ResultLog>
where
    T: IntoIterator<Item = Log>,
    P: PromiseHandler,
    I: IO + Copy,
{
    let mut previous_promise: Option<PromiseId> = None;
    logs.into_iter()
        .filter_map(|log| {
            if log.address == exit_to_near::ADDRESS.raw()
                || log.address == exit_to_ethereum::ADDRESS.raw()
            {
                if log.topics.is_empty() {
                    if let Ok(promise) = PromiseArgs::try_from_slice(&log.data) {
                        match promise {
                            PromiseArgs::Create(promise) => {
                                // Safety: this promise creation is safe because it does not come from
                                // users directly. The exit precompile only create promises which we
                                // are able to execute without violating any security invariants.
                                let id = match previous_promise {
                                    Some(base_id) => {
                                        schedule_promise_callback(handler, base_id, &promise)
                                    }
                                    None => schedule_promise(handler, &promise),
                                };
                                previous_promise = Some(id);
                            }
                            PromiseArgs::Callback(promise) => {
                                // Safety: This is safe because the promise data comes from our own
                                // exit precompiles. See note above.
                                let base_id = match previous_promise {
                                    Some(base_id) => {
                                        schedule_promise_callback(handler, base_id, &promise.base)
                                    }
                                    None => schedule_promise(handler, &promise.base),
                                };
                                let id =
                                    schedule_promise_callback(handler, base_id, &promise.callback);
                                previous_promise = Some(id);
                            }
                            PromiseArgs::Recursive(_) => {
                                unreachable!("Exit precompiles do not produce recursive promises")
                            }
                        }
                    }
                    // do not pass on these "internal logs" to caller
                    None
                } else {
                    // The exit precompiles do produce externally consumable logs in
                    // addition to the promises. The external logs have a non-empty
                    // `topics` field.
                    Some(evm_log_to_result_log(log))
                }
            } else if log.address == cross_contract_call::ADDRESS.raw() {
                if log.topics[0] == cross_contract_call::AMOUNT_TOPIC {
                    // NEAR balances are 128-bit, so the leading 16 bytes of the 256-bit topic
                    // value should always be zero.
                    assert_eq!(&log.topics[1].as_bytes()[0..16], &[0; 16]);
                    let required_near =
                        Yocto::new(U256::from_big_endian(log.topics[1].as_bytes()).low_u128());
                    if let Ok(promise) = PromiseCreateArgs::try_from_slice(&log.data) {
                        let id = crate::xcc::handle_precompile_promise(
                            io,
                            handler,
                            previous_promise,
                            &promise,
                            required_near,
                            current_account_id,
                        );
                        previous_promise = Some(id);
                    }
                }
                // do not pass on these "internal logs" to caller
                None
            } else {
                Some(evm_log_to_result_log(log))
            }
        })
        .collect()
}

fn evm_log_to_result_log(log: Log) -> ResultLog {
    let topics = log
        .topics
        .into_iter()
        .map(|topic| topic.0)
        .collect::<Vec<_>>();
    ResultLog {
        address: Address::new(log.address),
        topics,
        data: log.data,
    }
}

fn schedule_promise<P: PromiseHandler>(handler: &mut P, promise: &PromiseCreateArgs) -> PromiseId {
    sdk::log!(
        "call_contract {}.{}",
        promise.target_account_id,
        promise.method
    );
    handler.promise_create_call(promise)
}

fn schedule_promise_callback<P: PromiseHandler>(
    handler: &mut P,
    base_id: PromiseId,
    promise: &PromiseCreateArgs,
) -> PromiseId {
    sdk::log!(
        "callback_call_contract {}.{}",
        promise.target_account_id,
        promise.method
    );

    handler.promise_attach_callback(base_id, promise)
}

fn assert_access<I: IO + Copy, E: Env>(
    io: &I,
    env: &E,
    transaction: &NormalizedEthTransaction,
) -> Result<(), EngineError> {
    let allowed = if transaction.to.is_some() {
        silo::is_allow_submit(io, &env.predecessor_account_id(), &transaction.address)
    } else {
        silo::is_allow_deploy(io, &env.predecessor_account_id(), &transaction.address)
    };

    if !allowed {
        return Err(EngineError {
            kind: EngineErrorKind::NotAllowed,
            gas_used: 0,
        });
    }

    Ok(())
}

impl<I: IO + Copy, E: Env, M: ModExpAlgorithm> Backend for Engine<'_, I, E, M> {
    /// Returns the "effective" gas price (as defined by EIP-1559)
    fn gas_price(&self) -> U256 {
        self.gas_price
    }

    /// Returns the origin address that created the contract.
    fn origin(&self) -> H160 {
        self.origin.raw()
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
        U256::from_big_endian(&self.state.chain_id)
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

    /// Check if the storage of the address is empty.
    /// Related to EIP-7610: non-empty storage
    fn is_empty_storage(&self, address: H160) -> bool {
        // As we can't read all storage data for account we assuming that if storage exists
        // `index = 0` always true
        let index = H256::zero();
        // First we're checking cache to not produce read-storage operation
        let address = Address::new(address);
        if self
            .contract_storage_cache
            .borrow()
            .contains_key(&(address, index))
        {
            return false;
        }
        let generation = *self
            .generation_cache
            .borrow_mut()
            .entry(address)
            .or_insert_with(|| get_generation(&self.io, &address));
        !storage_has_key(&self.io, &address, &index, generation)
    }

    /// Get original storage value of address at index, if available.
    ///
    /// Since `SputnikVM` collects storage changes in memory until the transaction is over,
    /// the "original storage" will always be the same as the storage because no values
    /// are written to storage until after the transaction is complete.
    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        Some(self.storage(address, index))
    }

    fn get_blob_hash(&self, _index: usize) -> Option<U256> {
        None
    }

    fn blob_gas_price(&self) -> Option<u128> {
        None
    }
}

impl<J: IO + Copy, E: Env, M: ModExpAlgorithm> ApplyBackend for Engine<'_, J, E, M> {
    fn apply<A, I, L>(&mut self, values: A, _logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
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
            accounting::Net::Zero | accounting::Net::Lost(_) => (),
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
    }
}

fn submit_result_or_err(submit_result: SubmitResult) -> Result<SubmitResult, EngineError> {
    match submit_result.status {
        TransactionStatus::Succeed(_) => Ok(submit_result),
        TransactionStatus::Revert(bytes) => {
            let error_message =
                format!("Reverted with message: {}", String::from_utf8_lossy(&bytes));
            Err(engine_error(
                ExitError::Other(error_message.into()),
                submit_result.gas_used,
            ))
        }
        TransactionStatus::OutOfFund => {
            Err(engine_error(ExitError::OutOfFund, submit_result.gas_used))
        }
        TransactionStatus::OutOfOffset => {
            Err(engine_error(ExitError::OutOfOffset, submit_result.gas_used))
        }
        TransactionStatus::OutOfGas => {
            Err(engine_error(ExitError::OutOfGas, submit_result.gas_used))
        }
        TransactionStatus::CallTooDeep => {
            Err(engine_error(ExitError::CallTooDeep, submit_result.gas_used))
        }
        TransactionStatus::StackUnderflow => Err(engine_error(
            ExitError::StackUnderflow,
            submit_result.gas_used,
        )),
        TransactionStatus::StackOverflow => Err(engine_error(
            ExitError::StackOverflow,
            submit_result.gas_used,
        )),
        TransactionStatus::InvalidJump => {
            Err(engine_error(ExitError::InvalidJump, submit_result.gas_used))
        }
        TransactionStatus::InvalidRange => Err(engine_error(
            ExitError::InvalidRange,
            submit_result.gas_used,
        )),
        TransactionStatus::DesignatedInvalid => Err(engine_error(
            ExitError::DesignatedInvalid,
            submit_result.gas_used,
        )),
        TransactionStatus::CreateCollision => Err(engine_error(
            ExitError::CreateCollision,
            submit_result.gas_used,
        )),
        TransactionStatus::CreateContractLimit => Err(engine_error(
            ExitError::CreateContractLimit,
            submit_result.gas_used,
        )),
        TransactionStatus::InvalidCode(code) => Err(engine_error(
            ExitError::InvalidCode(Opcode(code)),
            submit_result.gas_used,
        )),
        TransactionStatus::PCUnderflow => {
            Err(engine_error(ExitError::PCUnderflow, submit_result.gas_used))
        }
        TransactionStatus::CreateEmpty => {
            Err(engine_error(ExitError::CreateEmpty, submit_result.gas_used))
        }
        TransactionStatus::MaxNonce => {
            Err(engine_error(ExitError::MaxNonce, submit_result.gas_used))
        }
        TransactionStatus::UsizeOverflow => Err(engine_error(
            ExitError::UsizeOverflow,
            submit_result.gas_used,
        )),
        TransactionStatus::CreateContractStartingWithEF => Err(engine_error(
            ExitError::CreateContractStartingWithEF,
            submit_result.gas_used,
        )),
        TransactionStatus::Other(e) => {
            Err(engine_error(ExitError::Other(e), submit_result.gas_used))
        }
    }
}

const fn engine_error(exit_error: ExitError, gas_used: u64) -> EngineError {
    EngineError {
        kind: EngineErrorKind::EvmError(exit_error),
        gas_used,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters::{FunctionCallArgsV1, FunctionCallArgsV2};
    use aurora_engine_sdk::env::Fixed;
    use aurora_engine_sdk::promise::Noop;
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use aurora_engine_test_doubles::promise::PromiseTracker;
    use aurora_engine_types::parameters::connector::FtOnTransferArgs;
    use aurora_engine_types::parameters::engine::RelayerKeyArgs;
    use aurora_engine_types::types::{Balance, NearGas, RawU256, make_address};
    use std::cell::RefCell;

    #[test]
    fn test_view_call_to_empty_contract_without_input_returns_empty_data() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let contract = make_address(1, 1);
        let value = Wei::new_u64(1000);
        let input = vec![];
        let args = ViewCallArgs {
            sender: origin,
            address: contract,
            amount: RawU256::from(value.raw().to_big_endian()),
            input,
        };
        let actual_status = engine.view_with_args(args).unwrap();
        let expected_status = TransactionStatus::Succeed(Vec::new());

        assert_eq!(expected_status, actual_status);
    }

    #[test]
    fn test_deploying_code_with_empty_input_succeeds() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let input = vec![];
        let mut handler = Noop;

        let actual_result = engine
            .deploy_code_with_input(input, None, &mut handler)
            .unwrap();

        let nonce = U256::zero();
        let expected_address = create_legacy_address(&origin, &nonce).as_bytes().to_vec();
        let expected_status = TransactionStatus::Succeed(expected_address);
        let expected_gas_used = 53000;
        let expected_logs = Vec::new();
        let expected_result = SubmitResult::new(expected_status, expected_gas_used, expected_logs);

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_deploying_code_with_address_succeeds() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let input = vec![];
        let mut handler = Noop;

        let address = Address::from_array([1; 20]);
        let actual_result = engine
            .deploy_code_with_input(input, Some(address), &mut handler)
            .unwrap();

        let expected_status = TransactionStatus::Succeed(address.as_bytes().to_vec());
        let expected_gas_used = 53000;
        let expected_logs = Vec::new();
        let expected_result = SubmitResult::new(expected_status, expected_gas_used, expected_logs);

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_call_to_empty_contract_returns_empty_data() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let input = Vec::<u8>::new();
        let mut handler = Noop;
        let contract = make_address(1, 1);
        let value = Wei::new_u64(1000);
        let args = CallArgs::V2(FunctionCallArgsV2 {
            contract,
            value: RawU256::from(value.raw().to_big_endian()),
            input,
        });
        let actual_result = engine.call_with_args(args, &mut handler).unwrap();

        let expected_data = Vec::new();
        let expected_status = TransactionStatus::Succeed(expected_data);
        let expected_gas_used = 21000;
        let expected_logs = Vec::new();
        let expected_result = SubmitResult::new(expected_status, expected_gas_used, expected_logs);

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_call_with_empty_balance_fails_with_out_of_funds_error() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let input = Vec::<u8>::new();
        let mut handler = Noop;
        let contract = make_address(1, 1);
        let value = Wei::new_u64(1000);
        let args = CallArgs::V2(FunctionCallArgsV2 {
            contract,
            value: RawU256::from(value.raw().to_big_endian()),
            input,
        });
        let actual_result = engine.call_with_args(args, &mut handler).unwrap();

        let expected_status = TransactionStatus::OutOfFund;
        let expected_gas_used = 21000;
        let expected_logs = Vec::new();
        let expected_result = SubmitResult::new(expected_status, expected_gas_used, expected_logs);

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_transfer_moves_balance_from_sender_to_recipient() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let gas_limit = u64::MAX;
        let mut handler = Noop;
        let receiver = make_address(1, 1);
        let value = Wei::new_u64(1000);
        let actual_result = engine
            .transfer(origin, receiver, value, gas_limit, &mut handler)
            .unwrap();

        let expected_data = Vec::new();
        let expected_status = TransactionStatus::Succeed(expected_data);
        let expected_gas_used = 21000;
        let expected_logs = Vec::new();
        let expected_result = SubmitResult::new(expected_status, expected_gas_used, expected_logs);

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_call_with_v1_args_to_empty_contract_returns_empty_data() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let input = Vec::<u8>::new();
        let mut handler = Noop;
        let contract = make_address(1, 1);
        let args = CallArgs::V1(FunctionCallArgsV1 { contract, input });
        let actual_result = engine.call_with_args(args, &mut handler).unwrap();

        let expected_data = Vec::new();
        let expected_status = TransactionStatus::Succeed(expected_data);
        let expected_gas_used = 21000;
        let expected_logs = Vec::new();
        let expected_result = SubmitResult::new(expected_status, expected_gas_used, expected_logs);

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_registering_relayer_succeeds() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let account_id = AccountId::new("relayer").unwrap();
        let expected_relayer_address = make_address(1, 1);
        engine.register_relayer(account_id.as_bytes(), expected_relayer_address);
        let actual_relayer_address = engine.get_relayer(account_id.as_bytes()).unwrap();

        assert_eq!(expected_relayer_address, actual_relayer_address);
    }

    #[test]
    fn test_registering_token_succeeds() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        set_balance(&mut io, &origin, &Wei::new_u64(22000));
        let mut engine: Engine<_, _> = Engine::new_with_state(
            EngineState::default(),
            origin,
            current_account_id.clone(),
            io,
            &env,
        );

        let receiver = make_address(6, 6);
        let erc20_token = make_address(4, 5);
        let nep141_token = AccountId::new("testcoin").unwrap();
        let args = FtOnTransferArgs {
            sender_id: AccountId::default(),
            amount: Balance::default(),
            msg: receiver.encode(),
        };
        let mut handler = Noop;
        engine
            .register_token(erc20_token, nep141_token.clone())
            .unwrap();
        let result = engine
            .receive_erc20_tokens(&nep141_token, &args, &current_account_id, &mut handler)
            .unwrap()
            .unwrap();

        assert!(matches!(result.status, TransactionStatus::Succeed(_)));
    }

    #[test]
    fn test_deploying_token_succeeds() {
        let env = Fixed::default();
        let origin = aurora_engine_sdk::types::near_account_to_evm_address(
            env.predecessor_account_id().as_bytes(),
        );
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        state::set_state(&mut io, &EngineState::default()).unwrap();

        let nep141_token = AccountId::new("testcoin").unwrap();
        let mut handler = Noop;
        let nonce = U256::zero();
        let expected_address = create_legacy_address(&origin, &nonce);
        let actual_address =
            deploy_erc20_token(nep141_token, None, io, &env, &mut handler).unwrap();

        assert_eq!(expected_address, actual_address);
    }

    #[test]
    fn test_get_erc20_metadata() {
        let env = Fixed::default();
        let origin = aurora_engine_sdk::types::near_account_to_evm_address(
            env.predecessor_account_id().as_bytes(),
        );
        let current_account_id = AccountId::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let state = EngineState::default();
        state::set_state(&mut io, &state).unwrap();

        let engine: Engine<_, _> =
            Engine::new_with_state(state, origin, current_account_id, io, &env);
        let nep141 = AccountId::new("testcoin").unwrap();
        let mut handler = Noop;
        let erc20_address = deploy_erc20_token(nep141, None, io, &env, &mut handler).unwrap();
        let metadata = engine
            .get_erc20_metadata(&Erc20Identifier::Erc20 {
                address: erc20_address,
            })
            .unwrap();

        assert_eq!(metadata, Erc20Metadata::default());
    }

    #[test]
    fn test_gas_charge_for_empty_transaction_is_zero() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(22000)).unwrap();
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let transaction = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: None,
            nonce: U256::default(),
            gas_limit: U256::MAX,
            max_priority_fee_per_gas: U256::default(),
            max_fee_per_gas: U256::MAX,
            to: None,
            value: Wei::default(),
            data: vec![],
            access_list: vec![],
            authorization_list: vec![],
        };
        let actual_result = engine
            .charge_gas(&origin, &transaction, None, None)
            .unwrap();

        let expected_result = GasPaymentResult {
            prepaid_amount: Wei::zero(),
            effective_gas_price: U256::zero(),
            priority_fee_per_gas: U256::zero(),
        };

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_gas_charge_for_non_empty_transaction() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        add_balance(&mut io, &origin, Wei::new_u64(2_000_000)).unwrap();
        let mut engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let transaction = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: None,
            nonce: U256::default(),
            gas_limit: 67_000.into(),
            max_priority_fee_per_gas: 20.into(),
            max_fee_per_gas: 10.into(),
            to: None,
            value: Wei::default(),
            data: vec![],
            access_list: vec![],
            authorization_list: vec![],
        };
        let actual_result = engine
            .charge_gas(&origin, &transaction, None, None)
            .unwrap();

        let expected_result = GasPaymentResult {
            prepaid_amount: Wei::new_u64(67_000 * 10),
            effective_gas_price: 10.into(),
            priority_fee_per_gas: 10.into(),
        };

        assert_eq!(expected_result, actual_result);

        let actual_result = engine
            .charge_gas(&origin, &transaction, None, Some(EthGas::new(50_000)))
            .unwrap();

        let expected_result = GasPaymentResult {
            prepaid_amount: Wei::new_u64(50_000 * 10),
            effective_gas_price: 10.into(),
            priority_fee_per_gas: 10.into(),
        };

        assert_eq!(expected_result, actual_result);

        let actual_result = engine
            .charge_gas(&origin, &transaction, Some(5.into()), None)
            .unwrap();

        let expected_result = GasPaymentResult {
            prepaid_amount: Wei::new_u64(67_000 * 5),
            effective_gas_price: 5.into(),
            priority_fee_per_gas: 5.into(),
        };

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_scheduling_promise_creates_it() {
        use aurora_engine_test_doubles::promise::PromiseArgs;
        use std::collections::HashMap;

        let mut promise_tracker = PromiseTracker::default();
        let args = PromiseCreateArgs {
            target_account_id: AccountId::default(),
            method: String::new(),
            args: vec![],
            attached_balance: Yocto::default(),
            attached_gas: NearGas::default(),
        };
        let actual_id = schedule_promise(&mut promise_tracker, &args);
        let actual_scheduled_promises = promise_tracker.scheduled_promises;
        let expected_scheduled_promises = {
            let mut map = HashMap::new();
            map.insert(actual_id.raw(), PromiseArgs::Create(args));
            map
        };

        assert_eq!(expected_scheduled_promises, actual_scheduled_promises);
    }

    #[test]
    fn test_scheduling_promise_callback_adds_it() {
        use aurora_engine_test_doubles::promise::PromiseArgs;
        use std::collections::HashMap;

        let mut promise_tracker = PromiseTracker::default();
        let args = PromiseCreateArgs {
            target_account_id: AccountId::default(),
            method: String::new(),
            args: vec![],
            attached_balance: Yocto::default(),
            attached_gas: NearGas::default(),
        };
        let base_id = PromiseId::new(6);
        let actual_id = schedule_promise_callback(&mut promise_tracker, base_id, &args);
        let actual_scheduled_promises = promise_tracker.scheduled_promises;
        let expected_scheduled_promises = {
            let mut map = HashMap::new();
            map.insert(
                actual_id.raw(),
                PromiseArgs::Callback {
                    base: base_id,
                    callback: args,
                },
            );
            map
        };

        assert_eq!(expected_scheduled_promises, actual_scheduled_promises);
    }

    #[test]
    fn test_loading_original_storage_loads_stored_value() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let expected_value = H256::from_low_u64_le(64);
        let index = H256::zero();
        let generation = get_generation(&io, &origin);
        set_storage(&mut io, &origin, &index, &expected_value, generation);
        let actual_value = engine.original_storage(origin.raw(), index).unwrap();

        assert_eq!(expected_value, actual_value);
    }

    #[test]
    fn test_storage_is_empty_with_cache() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let engine: Engine<_, _> =
            Engine::new_with_state(EngineState::default(), origin, current_account_id, io, &env);

        let expected_value = H256::from_low_u64_le(64);
        let index = H256::zero();
        // Check that storage is empty
        assert!(engine.is_empty_storage(origin.raw()));
        let generation = get_generation(&io, &origin);
        set_storage(&mut io, &origin, &index, &expected_value, generation);
        // It will read without cache
        assert!(!engine.is_empty_storage(origin.raw()));
        // Cache should be empty
        let cache_val = engine
            .contract_storage_cache
            .borrow()
            .contains_key(&(origin, index));
        assert!(!cache_val);
        // Check the storage value and hit the cache
        let actual_value = engine.storage(origin.raw(), index);
        assert_eq!(expected_value, actual_value);
        // Cache should exist
        let cache_val = engine
            .contract_storage_cache
            .borrow()
            .contains_key(&(origin, index));
        assert!(cache_val);
        remove_storage(&mut io, &origin, &index, generation);
        // Value should still be in the cache
        let actual_value = engine.storage(origin.raw(), index);
        assert_eq!(expected_value, actual_value);
        assert!(!engine.is_empty_storage(origin.raw()));
    }

    #[test]
    fn test_loading_engine_from_storage_loads_stored_state() {
        let origin = Address::zero();
        let current_account_id = AccountId::default();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let expected_state = EngineState::default();
        state::set_state(&mut io, &expected_state).unwrap();
        let engine: Engine<_, _> = Engine::new(origin, current_account_id, io, &env).unwrap();
        let actual_state = engine.state;

        assert_eq!(expected_state, actual_state);
    }

    #[test]
    fn test_refund_transfer_eth_back_from_precompile_address() {
        let recipient_address = make_address(1, 1);
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let expected_state = EngineState::default();
        let refund_amount = Wei::new_u64(1000);
        add_balance(&mut io, &exit_to_near::ADDRESS, refund_amount).unwrap();
        state::set_state(&mut io, &expected_state).unwrap();
        let args = RefundCallArgs {
            recipient_address,
            erc20_address: None,
            amount: RawU256::from(refund_amount.raw().to_big_endian()),
        };
        let mut handler = Noop;
        let actual_result = refund_on_error(io, &env, expected_state, &args, &mut handler).unwrap();
        let expected_result =
            SubmitResult::new(TransactionStatus::Succeed(Vec::new()), 25800, Vec::new());

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_refund_remint_burned_erc20_tokens() {
        let origin = Address::zero();
        let env = Fixed::default();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let expected_state = EngineState::default();
        state::set_state(&mut io, &expected_state).unwrap();
        let value = Wei::new_u64(1000);
        let args = RefundCallArgs {
            recipient_address: Address::default(),
            erc20_address: Some(origin),
            amount: RawU256::from(value.raw().to_big_endian()),
        };
        let mut handler = Noop;
        let actual_result = refund_on_error(io, &env, expected_state, &args, &mut handler).unwrap();
        let expected_result =
            SubmitResult::new(TransactionStatus::Succeed(Vec::new()), 21860, Vec::new());

        assert_eq!(expected_result, actual_result);
    }

    #[test]
    fn test_refund_free_effective_gas_does_nothing() {
        let origin = Address::zero();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let expected_state = EngineState::default();
        state::set_state(&mut io, &expected_state).unwrap();
        let relayer = make_address(1, 1);
        let gas_result = GasPaymentResult {
            prepaid_amount: Wei::default(),
            effective_gas_price: U256::zero(),
            priority_fee_per_gas: U256::zero(),
        };

        refund_unused_gas(&mut io, &origin, 1000, &gas_result, &relayer, None).unwrap();
    }

    #[test]
    fn test_refund_gas_pays_expected_amount() {
        let origin = Address::zero();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let expected_state = EngineState::default();
        state::set_state(&mut io, &expected_state).unwrap();
        let relayer = make_address(1, 1);
        let gas_result = GasPaymentResult {
            prepaid_amount: Wei::new_u64(8000),
            effective_gas_price: 1.into(),
            priority_fee_per_gas: 2.into(),
        };
        let gas_used = 4000;

        refund_unused_gas(&mut io, &origin, gas_used, &gas_result, &relayer, None).unwrap();

        let actual_refund = get_balance(&io, &origin);
        let expected_refund = Wei::new_u64(gas_used);
        assert_eq!(expected_refund, actual_refund);

        let actual_refund = get_balance(&io, &relayer);
        let expected_refund = Wei::new_u64(gas_used * 2);
        assert_eq!(expected_refund, actual_refund);
    }

    #[test]
    fn test_refund_fixed_gas_pays_expected_amount() {
        let origin = Address::zero();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let expected_state = EngineState::default();
        state::set_state(&mut io, &expected_state).unwrap();
        let relayer = make_address(1, 1);
        let gas_result = GasPaymentResult {
            prepaid_amount: Wei::new_u64(8000),
            effective_gas_price: 1.into(),
            priority_fee_per_gas: 2.into(),
        };
        let gas_used = 4000;
        let fixed_gas = Some(EthGas::new(7000));

        refund_unused_gas(&mut io, &origin, gas_used, &gas_result, &relayer, fixed_gas).unwrap();

        let actual_refund = get_balance(&io, &origin);
        let expected_refund = Wei::new_u64(1000);
        assert_eq!(expected_refund, actual_refund);

        let actual_refund = get_balance(&io, &relayer);
        let expected_refund = Wei::new_u64(7000 * 2);
        assert_eq!(expected_refund, actual_refund);
    }

    #[test]
    fn test_check_nonce_with_increment_succeeds() {
        let origin = Address::zero();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);

        increment_nonce(&mut io, &origin);
        check_nonce(&io, &origin, &U256::from(1u64)).unwrap();
    }

    #[test]
    fn test_check_nonce_without_increment_fails() {
        let origin = Address::zero();
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);

        increment_nonce(&mut io, &origin);
        let actual_error_kind = check_nonce(&io, &origin, &U256::from(0u64)).unwrap_err();

        assert_eq!(
            actual_error_kind.as_bytes(),
            b"ERR_INCORRECT_NONCE: ac: 1, tx: 0"
        );
    }

    #[test]
    fn test_create_legacy_address() {
        // Aurora transaction hash (aurorascan.dev): 0xfc94bb484a9b144b1588a2d7238a497b425db343f0217ab66eb6e5171b3b4645
        let caller = Address::decode("3160f7328df59c14d85dfd09addad4ef18ae3e2c").unwrap();
        let nonce = U256::from_dec_str("109438").unwrap();
        let created_address = create_legacy_address(&caller, &nonce);

        assert_eq!(
            created_address.encode(),
            "140e8a21d08cbb530929b012581a7c7e696145ef"
        );
    }

    #[test]
    fn test_filtering_promises_from_logs_with_none_keeps_all() {
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let current_account_id = AccountId::default();
        let mut handler = Noop;
        let logs = vec![Log {
            address: H160::default(),
            topics: vec![],
            data: vec![],
        }];

        let actual_logs = filter_promises_from_logs(&io, &mut handler, logs, &current_account_id);
        let expected_logs = vec![ResultLog {
            address: Address::default(),
            topics: vec![],
            data: vec![],
        }];

        assert_eq!(expected_logs, actual_logs);
    }

    #[test]
    fn test_add_remove_function_call_key() {
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);
        let public_key = serde_json::from_str::<RelayerKeyArgs>(
            r#"{"public_key":"ed25519:DcA2MzgpJbrUATQLLceocVckhhAqrkingax4oJ9kZ847"}"#,
        )
        .map(|args| args.public_key)
        .unwrap();

        let result = remove_function_call_key(&mut io, &public_key);
        assert!(result.is_err()); // should fail because the key doesn't exist yet.

        add_function_call_key(&mut io, &public_key);

        let result = remove_function_call_key(&mut io, &public_key);
        assert!(result.is_ok());
        let result = remove_function_call_key(&mut io, &public_key);
        assert!(result.is_err()); // should fail because the key doesn't exist anymore.
    }
}
