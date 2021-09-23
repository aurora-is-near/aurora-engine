use crate::types::*;
use crate::*;
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PromiseCreateArgs {
    pub target_account_id: AccountId,
    pub method: String,
    pub args: Vec<u8>,
    pub attached_balance: u128,
    pub attached_gas: u64,
}

/// withdraw NEAR eth-connector call args
#[derive(BorshSerialize, BorshDeserialize)]
pub struct WithdrawCallArgs {
    pub recipient_address: EthAddress,
    pub amount: Balance,
}
