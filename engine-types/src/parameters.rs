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

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct PromiseCreateArgs {
    pub target_account_id: AccountId,
    pub method: String,
    pub args: Vec<u8>,
    pub attached_balance: Yocto,
    pub attached_gas: NearGas,
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PromiseWithCallbackArgs {
    pub base: PromiseCreateArgs,
    pub callback: PromiseCreateArgs,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub enum PromiseAction {
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
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct WithdrawCallArgs {
    pub recipient_address: Address,
    pub amount: NEP141Wei,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RefundCallArgs {
    pub recipient_address: Address,
    pub erc20_address: Option<Address>,
    pub amount: RawU256,
}
