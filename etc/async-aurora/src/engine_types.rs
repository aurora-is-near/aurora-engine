use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

pub type EthAddress = [u8; 20];
pub type RawU256 = [u8; 32];

/// The status of a transaction.
#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Succeed(Vec<u8>),
    Revert(Vec<u8>),
    OutOfGas,
    OutOfFund,
    OutOfOffset,
    CallTooDeep,
}

/// Borsh-encoded log for use in a `SubmitResult`.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ResultLog {
    pub address: EthAddress,
    pub topics: Vec<RawU256>,
    pub data: Vec<u8>,
}

/// Borsh-encoded parameters for the `call`, `call_with_args`, `deploy_code`,
/// and `deploy_with_input` methods.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct SubmitResult {
    version: u8,
    pub status: TransactionStatus,
    pub gas_used: u64,
    pub logs: Vec<ResultLog>,
}
