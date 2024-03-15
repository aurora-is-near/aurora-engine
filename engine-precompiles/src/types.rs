use crate::account_ids::PredecessorAccount;

use aurora_engine_types::types::EthGas;
use aurora_engine_types::{Cow, Vec, H160, H256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log {
    pub address: H160,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize))]
pub enum ExitError {
    StackUnderflow,
    StackOverflow,
    InvalidJump,
    InvalidRange,
    DesignatedInvalid,
    CallTooDeep,
    CreateCollision,
    CreateContractLimit,
    OutOfOffset,
    OutOfGas,
    OutOfFund,
    #[allow(clippy::upper_case_acronyms)]
    PCUnderflow,
    CreateEmpty,
    Other(Cow<'static, str>),
    MaxNonce,
    InvalidCode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize))]
pub enum ExitFatal {
    NotSupported,
    UnhandledInterrupt,
    CallErrorAsFatal(ExitError),
    Other(Cow<'static, str>),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PrecompileOutput {
    pub cost: EthGas,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
}

impl PrecompileOutput {
    #[must_use]
    pub fn without_logs(cost: EthGas, output: Vec<u8>) -> Self {
        Self {
            cost,
            output,
            logs: Vec::new(),
        }
    }
}

pub enum AllPrecompiles<'a, I, E, H> {
    PredecessorAccount(PredecessorAccount<'a, E>),
}
