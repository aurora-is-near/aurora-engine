use borsh::{BorshDeserialize, BorshSerialize};

use crate::prelude::{String, Vec};
use crate::types::{AccountId, RawAddress, RawH256, RawU256, U128};
use evm::backend::Log;

/// Borsh-encoded parameters for the `new` function.    
#[derive(BorshSerialize, BorshDeserialize)]
pub struct NewCallArgs {
    /// Chain id, according to the EIP-115 / ethereum-lists spec.
    pub chain_id: RawU256,
    /// Account which can upgrade this contract.
    /// Use empty to disable updatability.
    pub owner_id: AccountId,
    /// Account of the bridge prover.
    /// Use empty to not use base token as bridged asset.
    pub bridge_prover_id: AccountId,
    /// How many blocks after staging upgrade can deploy it.
    pub upgrade_delay_blocks: u64,
}

/// Borsh-encoded parameters for the `meta_call` function.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct MetaCallArgs {
    pub signature: [u8; 64],
    pub v: u8,
    pub nonce: RawU256,
    pub fee_amount: RawU256,
    pub fee_address: RawAddress,
    pub contract_address: RawAddress,
    pub value: RawU256,
    pub method_def: String,
    pub args: Vec<u8>,
}

/// Borsh-encoded log for use in a `SubmitResult`.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ResultLog {
    pub topics: Vec<RawU256>,
    pub data: Vec<u8>,
}

impl From<Log> for ResultLog {
    fn from(log: Log) -> Self {
        let topics = log
            .topics
            .into_iter()
            .map(|topic| topic.0)
            .collect::<Vec<_>>();
        ResultLog {
            topics,
            data: log.data,
        }
    }
}

/// Borsh-encoded parameters for the `call`, `call_with_args`, `deploy_code`,
/// and `deploy_with_input` methods.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct SubmitResult {
    pub status: bool,
    pub gas_used: u64,
    pub result: Vec<u8>,
    pub logs: Vec<ResultLog>,
}

/// Borsh-encoded parameters for the `call` function.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FunctionCallArgs {
    pub contract: RawAddress,
    pub input: Vec<u8>,
}

/// Borsh-encoded parameters for the `view` function.
#[derive(BorshSerialize, BorshDeserialize, Debug, Eq, PartialEq)]
pub struct ViewCallArgs {
    pub sender: RawAddress,
    pub address: RawAddress,
    pub amount: RawU256,
    pub input: Vec<u8>,
}

/// Borsh-encoded parameters for the `get_storage_at` function.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct GetStorageAtArgs {
    pub address: RawAddress,
    pub key: RawH256,
}

/// Borsh-encoded (genesis) account balance used by the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountBalance {
    pub address: RawAddress,
    pub balance: RawU256,
}

/// Borsh-encoded parameters for the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BeginChainArgs {
    pub chain_id: RawU256,
    pub genesis_alloc: Vec<AccountBalance>,
}

/// Borsh-encoded parameters for the `begin_block` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BeginBlockArgs {
    /// The current block's hash (for replayer use).
    pub hash: RawU256,
    /// The current block's beneficiary address.
    pub coinbase: RawAddress,
    /// The current block's timestamp (in seconds since the Unix epoch).
    pub timestamp: RawU256,
    /// The current block's number (the genesis block is number zero).
    pub number: RawU256,
    /// The current block's difficulty.
    pub difficulty: RawU256,
    /// The current block's gas limit.
    pub gaslimit: RawU256,
}

/// Borsh-encoded parameters for the `ft_transfer_call` function
/// for regular NEP-141 tokens.
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct NEP141TransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: U128,
    pub memo: Option<String>,
}

/// Borsh-encoded parameters for the `withdraw` function for
/// NEP-141 tokens that were bridged from ERC-20 tokens.
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BridgedTokenWithdrawArgs {
    pub amount: U128,
    pub recipient: String,
}

/// Borsh-encoded parameters for the `withdraw` function.
/// This is a function from Bridged ERC-20 in NEAR
///
/// TODO(#51): Flip arguments in Eth-Connector for consistency:
///     https://github.com/aurora-is-near/eth-connector/blob/master/evm-fungible-token/src/lib.rs#L190-L194
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BridgedTokenWithdrawEthConnectorArgs {
    pub recipient: String,
    pub amount: U128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_call_fail() {
        let bytes = [0; 71];
        let _ = ViewCallArgs::try_from_slice(&bytes).unwrap_err();
    }

    #[test]
    fn test_roundtrip_view_call() {
        let x = ViewCallArgs {
            sender: [1; 20],
            address: [2; 20],
            amount: [3; 32],
            input: vec![1, 2, 3],
        };
        let bytes = x.try_to_vec().unwrap();
        let res = ViewCallArgs::try_from_slice(&bytes).unwrap();
        assert_eq!(x, res);
    }
}
