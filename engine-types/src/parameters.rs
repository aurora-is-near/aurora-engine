use crate::account_id::*;
use crate::types::*;
use crate::types_new::Address;
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
    pub attached_balance: u128,
    pub attached_gas: u64,
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
        amount: u128,
    },
    DeployConotract {
        code: Vec<u8>,
    },
    FunctionCall {
        name: String,
        args: Vec<u8>,
        attached_yocto: u128,
        gas: u64,
    },
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct PromiseBatchAction {
    pub target_account_id: AccountId,
    pub actions: Vec<PromiseAction>,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct WithdrawCallArgs {
    pub recipient_address: Address,
    pub amount: Balance,
}

/// withdraw NEAR eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct RefundCallArgs {
    pub recipient_address: EthAddress,
    pub erc20_address: Option<EthAddress>,
    pub amount: RawU256,
}
