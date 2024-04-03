use aurora_engine_types::{
    types::address::error::AddressError, types::balance::error::BalanceOverflowError,
};

use crate::errors;

#[derive(Debug)]
pub enum StorageReadError {
    KeyNotFound,
    BorshDeserialize,
}

impl AsRef<[u8]> for StorageReadError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::KeyNotFound => errors::ERR_CONNECTOR_STORAGE_KEY_NOT_FOUND,
            Self::BorshDeserialize => errors::ERR_FAILED_DESERIALIZE_CONNECTOR_DATA,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum FinishDepositError {
    TransferCall(FtTransferCallError),
    ProofUsed,
}

impl From<ProofUsed> for FinishDepositError {
    fn from(_: ProofUsed) -> Self {
        Self::ProofUsed
    }
}

impl From<FtTransferCallError> for FinishDepositError {
    fn from(e: FtTransferCallError) -> Self {
        Self::TransferCall(e)
    }
}

impl From<DepositError> for FinishDepositError {
    fn from(e: DepositError) -> Self {
        Self::TransferCall(FtTransferCallError::Transfer(e.into()))
    }
}

impl AsRef<[u8]> for FinishDepositError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::ProofUsed => errors::ERR_PROOF_EXIST,
            Self::TransferCall(e) => e.as_ref(),
        }
    }
}

#[derive(Debug)]
pub enum WithdrawError {
    FT(WithdrawFtError),
    Paused,
    ParseArgs,
    TotalSupplyUnderflow,
    InsufficientFunds,
}

impl From<WithdrawFtError> for WithdrawError {
    fn from(e: WithdrawFtError) -> Self {
        Self::FT(e)
    }
}

impl AsRef<[u8]> for WithdrawError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::FT(e) => e.as_ref(),
            Self::ParseArgs => errors::ERR_PARSE_WITHDRAW_EVENT,
            Self::Paused => errors::ERR_PAUSED,
            Self::TotalSupplyUnderflow => errors::ERR_TOTAL_SUPPLY_UNDERFLOW,
            Self::InsufficientFunds => errors::ERR_NOT_ENOUGH_BALANCE,
        }
    }
}

impl From<WithdrawError> for TransferError {
    fn from(err: WithdrawError) -> Self {
        Self::Withdraw(err)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum FtTransferCallError {
    BalanceOverflow(BalanceOverflowError),
    MessageParseFailed(ParseOnTransferMessageError),
    InsufficientAmountForFee,
    Transfer(TransferError),
    Paused,
}

impl From<TransferError> for FtTransferCallError {
    fn from(e: TransferError) -> Self {
        Self::Transfer(e)
    }
}

impl From<DepositError> for FtTransferCallError {
    fn from(e: DepositError) -> Self {
        Self::Transfer(e.into())
    }
}

impl From<ParseOnTransferMessageError> for FtTransferCallError {
    fn from(e: ParseOnTransferMessageError) -> Self {
        Self::MessageParseFailed(e)
    }
}

impl AsRef<[u8]> for FtTransferCallError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::MessageParseFailed(e) => e.as_ref(),
            Self::InsufficientAmountForFee => errors::ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
            Self::Transfer(e) => e.as_ref(),
            Self::BalanceOverflow(e) => e.as_ref(),
            Self::Paused => errors::ERR_FT_PAUSED,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum InitContractError {
    AlreadyInitialized,
    InvalidCustodianAddress(AddressError),
}

impl AsRef<[u8]> for InitContractError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::AlreadyInitialized => errors::ERR_CONTRACT_INITIALIZED,
            Self::InvalidCustodianAddress(e) => e.as_ref(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub struct ProofUsed;

impl AsRef<[u8]> for ProofUsed {
    fn as_ref(&self) -> &[u8] {
        errors::ERR_PROOF_EXIST
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum DepositError {
    TotalSupplyOverflow,
    BalanceOverflow,
    Paused,
    ProofParseFailed,
    EventParseFailed(ParseError),
    CustodianAddressMismatch,
    InsufficientAmountForFee,
    InvalidAddress(AddressError),
}

impl AsRef<[u8]> for DepositError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::TotalSupplyOverflow => errors::ERR_TOTAL_SUPPLY_OVERFLOW,
            Self::BalanceOverflow => errors::ERR_BALANCE_OVERFLOW,
            Self::Paused => errors::ERR_PAUSED,
            Self::ProofParseFailed => errors::ERR_BORSH_DESERIALIZE.as_bytes(),
            Self::EventParseFailed(e) => e.as_ref(),
            Self::CustodianAddressMismatch => errors::ERR_WRONG_EVENT_ADDRESS,
            Self::InsufficientAmountForFee => errors::ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
            Self::InvalidAddress(e) => e.as_ref(),
        }
    }
}

#[derive(Debug)]
pub enum WithdrawFtError {
    TotalSupplyUnderflow,
    InsufficientFunds,
    BalanceOverflow(BalanceOverflowError),
}

impl AsRef<[u8]> for WithdrawFtError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::TotalSupplyUnderflow => errors::ERR_TOTAL_SUPPLY_UNDERFLOW,
            Self::InsufficientFunds => errors::ERR_NOT_ENOUGH_BALANCE,
            Self::BalanceOverflow(e) => e.as_ref(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum TransferError {
    TotalSupplyUnderflow,
    TotalSupplyOverflow,
    InsufficientFunds,
    BalanceOverflow,
    ZeroAmount,
    SelfTransfer,
    Deposit(DepositError),
    Withdraw(WithdrawError),
    Paused,
}

impl AsRef<[u8]> for TransferError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::TotalSupplyUnderflow => errors::ERR_TOTAL_SUPPLY_UNDERFLOW,
            Self::TotalSupplyOverflow => errors::ERR_TOTAL_SUPPLY_OVERFLOW,
            Self::InsufficientFunds => errors::ERR_NOT_ENOUGH_BALANCE,
            Self::BalanceOverflow => errors::ERR_BALANCE_OVERFLOW,
            Self::ZeroAmount => errors::ERR_ZERO_AMOUNT,
            Self::SelfTransfer => errors::ERR_SENDER_EQUALS_RECEIVER,
            Self::Deposit(e) => e.as_ref(),
            Self::Withdraw(e) => e.as_ref(),
            Self::Paused => errors::ERR_FT_PAUSED,
        }
    }
}

impl From<WithdrawFtError> for TransferError {
    fn from(err: WithdrawFtError) -> Self {
        match err {
            WithdrawFtError::InsufficientFunds => Self::InsufficientFunds,
            WithdrawFtError::TotalSupplyUnderflow => Self::TotalSupplyUnderflow,
            WithdrawFtError::BalanceOverflow(_) => Self::BalanceOverflow,
        }
    }
}

impl From<DepositError> for TransferError {
    fn from(err: DepositError) -> Self {
        Self::Deposit(err)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum StorageFundingError {
    NotRegistered,
    NoAvailableBalance,
    InsufficientDeposit,
    UnRegisterPositiveBalance,
    Paused,
}

impl AsRef<[u8]> for StorageFundingError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::NotRegistered => errors::ERR_ACCOUNT_NOT_REGISTERED,
            Self::NoAvailableBalance => errors::ERR_NO_AVAILABLE_BALANCE,
            Self::InsufficientDeposit => errors::ERR_ATTACHED_DEPOSIT_NOT_ENOUGH,
            Self::UnRegisterPositiveBalance => {
                errors::ERR_FAILED_UNREGISTER_ACCOUNT_POSITIVE_BALANCE
            }
            Self::Paused => errors::ERR_FT_PAUSED,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum DecodeError {
    RlpFailed,
    SchemaMismatch,
}
impl AsRef<[u8]> for DecodeError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::RlpFailed => errors::ERR_RLP_FAILED,
            Self::SchemaMismatch => errors::ERR_PARSE_DEPOSIT_EVENT,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum ParseEventMessageError {
    TooManyParts,
    InvalidAccount,
    EthAddressValidationError(AddressError),
}

impl AsRef<[u8]> for ParseEventMessageError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::TooManyParts => errors::ERR_INVALID_EVENT_MESSAGE_FORMAT,
            Self::InvalidAccount => errors::ERR_INVALID_ACCOUNT_ID,
            Self::EthAddressValidationError(e) => e.as_ref(),
        }
    }
}

impl From<ParseEventMessageError> for ParseError {
    fn from(e: ParseEventMessageError) -> Self {
        Self::MessageParseFailed(e)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum ParseError {
    LogParseFailed(DecodeError),
    InvalidSender,
    InvalidAmount,
    InvalidFee,
    MessageParseFailed(ParseEventMessageError),
    OverflowNumber,
}

impl AsRef<[u8]> for ParseError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::LogParseFailed(e) => e.as_ref(),
            Self::InvalidSender => errors::ERR_INVALID_SENDER,
            Self::InvalidAmount => errors::ERR_INVALID_AMOUNT,
            Self::InvalidFee => errors::ERR_INVALID_FEE,
            Self::MessageParseFailed(e) => e.as_ref(),
            Self::OverflowNumber => errors::ERR_OVERFLOW_NUMBER,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum ParseOnTransferMessageError {
    TooManyParts,
    InvalidHexData,
    WrongMessageFormat,
    InvalidAccount,
    OverflowNumber,
}

impl AsRef<[u8]> for ParseOnTransferMessageError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::TooManyParts => errors::ERR_INVALID_ON_TRANSFER_MESSAGE_FORMAT,
            Self::InvalidHexData => errors::ERR_INVALID_ON_TRANSFER_MESSAGE_HEX,
            Self::WrongMessageFormat => errors::ERR_INVALID_ON_TRANSFER_MESSAGE_DATA,
            Self::InvalidAccount => errors::ERR_INVALID_ACCOUNT_ID,
            Self::OverflowNumber => errors::ERR_OVERFLOW_NUMBER,
        }
    }
}
