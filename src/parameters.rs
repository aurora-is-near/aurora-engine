use borsh::{BorshDeserialize, BorshSerialize};

use crate::prelude::{String, Vec};
#[cfg(feature = "contract")]
use crate::prover::Proof;
use crate::types::{AccountId, RawAddress, RawH256, RawU256};
#[cfg(feature = "contract")]
use crate::types::{Balance, EthAddress};

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

/// Borsh-encoded parameters for the `begin_chain` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BeginChainArgs {
    pub chain_id: RawU256,
}

/// Borsh-encoded parameters for the `begin_block` function.
#[cfg(feature = "evm_bully")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BeginBlockArgs {
    /// The current block's hash (for replayer use).
    pub hash: RawU256,
    /// The current block's beneficiary address.
    pub coinbase: RawU256,
    /// The current block's timestamp (in seconds since the Unix epoch).
    pub timestamp: RawU256,
    /// The current block's number (the genesis block is number zero).
    pub number: RawU256,
    /// The current block's difficulty.
    pub difficulty: RawU256,
    /// The current block's gas limit.
    pub gaslimit: RawU256,
}

/// Eth-connector deposit arguments
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct DepositCallArgs {
    /// Proof data
    pub proof: Proof,
    /// Optional relayer address
    pub relayer_eth_account: Option<EthAddress>,
}

/// withdraw result for eth-connector
#[cfg(feature = "contract")]
#[derive(BorshSerialize)]
pub struct WithdrawResult {
    pub amount: Balance,
    pub recipient_id: RawAddress,
    pub eth_custodian_address: RawAddress,
}

/// ft_on_transfer eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FtOnTransfer {
    pub amount: Balance,
    pub msg: String,
    pub receiver_id: AccountId,
}

/// ft_resolve_transfer eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize)]
pub struct FtResolveTransfer {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub current_account_id: AccountId,
}

/// Fungible token storage balance
#[cfg(feature = "contract")]
#[derive(BorshSerialize)]
pub struct StorageBalance {
    pub total: Balance,
    pub available: Balance,
}

/// resolve_transfer eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct ResolveTransferCallArgs {
    pub sender_id: AccountId,
    pub amount: Balance,
    pub receiver_id: AccountId,
}

/// Finish deposit NEAR eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinishDepositCallArgs {
    pub new_owner_id: AccountId,
    pub amount: Balance,
    pub proof_key: String,
    pub relayer_id: AccountId,
    pub fee: Balance,
    pub msg: Option<Vec<u8>>,
}

/// Deposit ETH args
#[cfg(feature = "contract")]
#[derive(Default, BorshDeserialize, BorshSerialize, Clone)]
pub struct DepositEthCallArgs {
    pub proof: Proof,
    pub relayer_eth_account: EthAddress,
}

/// Finish deposit NEAR eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinishDepositEthCallArgs {
    pub new_owner_id: EthAddress,
    pub amount: Balance,
    pub fee: Balance,
    pub relayer_eth_account: AccountId,
    pub proof: Proof,
}

/// eth-connector initial args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitCallArgs {
    pub prover_account: AccountId,
    pub eth_custodian_address: AccountId,
}

/// transfer eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransferCallCallArgs {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub memo: Option<String>,
    pub msg: String,
}

/// Deploy EVM token args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct DeployEvmTokenCallArgs {
    pub near_account_id: AccountId,
    pub erc20_contract: Vec<u8>,
}

/// storage_balance_of eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StorageBalanceOfCallArgs {
    pub account_id: AccountId,
}

/// storage_deposit eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StorageDepositCallArgs {
    pub account_id: Option<AccountId>,
    pub registration_only: Option<bool>,
}

/// storage_withdraw eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StorageWithdrawCallArgs {
    pub amount: Option<u128>,
}

/// transfer args for json invocation
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct TransferCallArgs {
    pub receiver_id: AccountId,
    pub amount: Balance,
    pub memo: Option<String>,
}

/// withdraw NEAR eth-connector call args
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct WithdrawCallArgs {
    pub recipient_id: AccountId,
    pub amount: Balance,
}

/// balance_of args for json invocation
#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BalanceOfCallArgs {
    pub account_id: AccountId,
}

#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BalanceOfEthCallArgs {
    pub address: EthAddress,
}

#[cfg(feature = "contract")]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct RegisterRelayerCallArgs {
    pub address: EthAddress,
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
