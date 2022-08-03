use crate::account_id::*;
use crate::types::*;
use crate::*;
use borsh::{BorshDeserialize, BorshSerialize};

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum PromiseArgs {
    Create(PromiseCreateArgs),
    Callback(PromiseWithCallbackArgs),
}

impl PromiseArgs {
    pub fn total_gas(&self) -> NearGas {
        match self {
            Self::Create(call) => call.attached_gas,
            Self::Callback(cb) => cb.base.attached_gas + cb.callback.attached_gas,
        }
    }
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub struct PromiseCreateArgs {
    pub target_account_id: AccountId,
    pub method: String,
    pub args: Vec<u8>,
    pub attached_balance: Yocto,
    pub attached_gas: NearGas,
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub struct PromiseWithCallbackArgs {
    pub base: PromiseCreateArgs,
    pub callback: PromiseCreateArgs,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub enum PromiseAction {
    CreateAccount,
    Transfer {
        amount: Yocto,
    },
    DeployConotract {
        code: Vec<u8>,
    },
    FunctionCall {
        name: String,
        args: Vec<u8>,
        attached_yocto: Yocto,
        gas: NearGas,
    },
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct PromiseBatchAction {
    pub target_account_id: AccountId,
    pub actions: Vec<PromiseAction>,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct WithdrawCallArgs {
    pub recipient_address: Address,
    pub amount: NEP141Wei,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct RefundCallArgs {
    pub recipient_address: Address,
    pub erc20_address: Option<Address>,
    pub amount: RawU256,
}

/// Args passed to the the cross contract call precompile.
/// That precompile is used by Aurora contracts to make calls to the broader NEAR ecosystem.
/// See https://github.com/aurora-is-near/AIPs/pull/2 for design details.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum CrossContractCallArgs {
    /// The promise is to be executed immediately (as part of the same NEAR transaction as the EVM call).
    Eager(PromiseArgs),
    /// The promise is to be stored in the router contract, and can be executed in a future transaction.
    /// The purpose of this is to expand how much NEAR gas can be made available to a cross contract call.
    /// For example, if an expensive EVM call ends with a NEAR cross contract call, then there may not be
    /// much gas left to perform it. In this case, the promise could be `Delayed` (stored in the router)
    /// and executed in a separate transaction with a fresh 300 Tgas available for it.
    Delayed(PromiseArgs),
}
