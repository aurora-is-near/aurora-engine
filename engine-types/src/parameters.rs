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

/// Borsh-encoded parameters for the engine `call` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub struct FunctionCallArgsV2 {
    pub contract: Address,
    /// Wei compatible Borsh-encoded value field to attach an ETH balance to the transaction
    pub value: WeiU256,
    pub input: Vec<u8>,
}

/// Legacy Borsh-encoded parameters for the engine `call` function, to provide backward type compatibility
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub struct FunctionCallArgsV1 {
    pub contract: Address,
    pub input: Vec<u8>,
}

/// Deserialized values from bytes to current or legacy Borsh-encoded parameters
/// for passing to the engine `call` function, and to provide backward type compatibility
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub enum CallArgs {
    V2(FunctionCallArgsV2),
    V1(FunctionCallArgsV1),
}

impl CallArgs {
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        // For handling new input format (wrapped into call args enum) - for data structures with new arguments,
        // made for flexibility and extensibility.
        if let Ok(value) = Self::try_from_slice(bytes) {
            Some(value)
            // Fallback, for handling old input format,
            // i.e. input, formed as a raw (not wrapped into call args enum) data structure with legacy arguments,
            // made for backward compatibility.
        } else if let Ok(value) = FunctionCallArgsV1::try_from_slice(bytes) {
            Some(Self::V1(value))
            // Dealing with unrecognized input should be handled and result as an exception in a call site.
        } else {
            None
        }
    }
}
