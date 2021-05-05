use borsh::BorshSerialize;
use evm::{Context, ExitError, ExitSucceed};

use super::{Precompile, PrecompileResult};
use crate::engine::ERC20_NEP141_PREFIX;
use crate::map::append_slice;
use crate::parameters::{
    BridgedTokenWithdrawArgs, BridgedTokenWithdrawEthConnectorArgs, NEP141TransferCallArgs,
};
use crate::prelude::{String, Vec, U256};
use crate::sdk;
use crate::types::{AccountId, U128};

const ERR_TARGET_TOKEN_NOT_FOUND: &str = "Target token not found";

mod costs {
    use crate::types::Gas;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const EXIT_TO_NEAR_GAS: Gas = 0;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const EXIT_TO_ETHEREUM_GAS: Gas = 0;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const FT_TRANSFER_GAS: Gas = 50_000;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const WITHDRAWAL_GAS: Gas = 50_000;
}

/// Get the current nep141 token associated with the current erc20 token.
/// This will fail is none is associated.
fn get_nep141_from_erc20(erc20_token: &[u8]) -> String {
    String::from_utf8(
        sdk::read_storage(append_slice(ERC20_NEP141_PREFIX, erc20_token).as_slice())
            .expect(ERR_TARGET_TOKEN_NOT_FOUND),
    )
    .unwrap()
}

pub struct ExitToNear; //TransferEthToNear

impl Precompile for ExitToNear {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::EXIT_TO_NEAR_GAS)
    }

    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141_account, args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            //
            // Input slice format:
            //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens

            (
                String::from_utf8(sdk::current_account_id()).unwrap(),
                NEP141TransferCallArgs {
                    receiver_id: String::from_utf8(input.to_vec()).unwrap(),
                    amount: U128(context.apparent_value.as_u128()),
                    memo: None,
                },
            )
        } else {
            // ERC20 transfer
            //
            // This precompile branch is expected to be called from the ERC20 burn function\
            //
            // Input slice format:
            //      sender (20 bytes) - Eth address of the address that burned his erc20 tokens
            //      amount (U256 le bytes) - the amount that was burned
            //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 tokens

            let nep141_address = get_nep141_from_erc20(context.caller.as_bytes());

            let mut input_mut = input;

            let mut sender = [0u8; 20];
            sender.copy_from_slice(&input_mut[..20]);
            input_mut = &input_mut[20..];

            let amount = U128(U256::from_little_endian(&input_mut[..32]).as_u128());
            input_mut = &input_mut[32..];

            let receiver_account_id: AccountId = String::from_utf8(input_mut.to_vec()).unwrap();

            (
                nep141_address,
                NEP141TransferCallArgs {
                    receiver_id: receiver_account_id,
                    amount,
                    memo: None,
                },
            )
        };

        let promise0 = sdk::promise_create(
            nep141_account,
            b"ft_transfer",
            BorshSerialize::try_to_vec(&args).ok().unwrap().as_slice(),
            0,
            costs::FT_TRANSFER_GAS,
        );

        sdk::promise_return(promise0);

        Ok((ExitSucceed::Returned, Vec::new(), 0))
    }
}

pub struct ExitToEthereum;

impl Precompile for ExitToEthereum {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::EXIT_TO_ETHEREUM_GAS)
    }

    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141_account, serialized_args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            //
            // Input slice format:
            //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

            let eth_recipient: AccountId = String::from_utf8(input.to_vec()).unwrap();
            let args = BridgedTokenWithdrawEthConnectorArgs {
                amount: U128(context.apparent_value.as_u128()),
                recipient: eth_recipient,
            };

            (
                String::from_utf8(sdk::current_account_id()).unwrap(),
                BorshSerialize::try_to_vec(&args).ok().unwrap(),
            )
        } else {
            // ERC-20 transfer
            //
            // This precompile branch is expected to be called from the ERC20 withdraw function
            // (or burn function with some flag provided that this is expected to be withdrawn)
            //
            // Input slice format:
            //      sender (20 bytes) - Eth address of the address that burned his erc20 tokens
            //      amount (U256 le bytes) - the amount that was burned
            //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

            let nep141_address = get_nep141_from_erc20(context.caller.as_bytes());

            let mut input_mut = input;

            let mut sender = [0u8; 20];
            sender.copy_from_slice(&input_mut[..20]);
            input_mut = &input_mut[20..];

            let amount = U128(U256::from_little_endian(&input_mut[..32]).as_u128());
            input_mut = &input_mut[32..];

            let eth_recipient: AccountId = String::from_utf8(input_mut.to_vec()).unwrap();
            let args = BridgedTokenWithdrawArgs {
                recipient: eth_recipient,
                amount,
            };

            (
                nep141_address,
                BorshSerialize::try_to_vec(&args).ok().unwrap(),
            )
        };

        let promise0 = sdk::promise_create(
            nep141_account,
            b"withdraw",
            serialized_args.as_slice(),
            0,
            costs::WITHDRAWAL_GAS,
        );

        sdk::promise_return(promise0);

        Ok((ExitSucceed::Returned, Vec::new(), 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precompiles::{EXIT_TO_ETHEREUM_ID, EXIT_TO_NEAR_ID};
    use crate::types::near_account_to_evm_address;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            EXIT_TO_ETHEREUM_ID,
            near_account_to_evm_address("exitToEthereum".as_bytes()).to_low_u64_be()
        );
        assert_eq!(
            EXIT_TO_NEAR_ID,
            near_account_to_evm_address("exitToNear".as_bytes()).to_low_u64_be()
        );
    }
}
