use borsh::BorshSerialize;
use evm::{Context, ExitError, ExitSucceed};

use super::{Precompile, PrecompileResult};
use crate::parameters::{
    BridgedTokenWithdrawArgs, BridgedTokenWithdrawEthConnectorArgs, NEP41TransferCallArgs,
};
use crate::prelude::{String, ToString, Vec, U256};
use crate::sdk;
use crate::types::{AccountId, Gas};

mod costs {
    use crate::types::Gas;

    // TODO: Determine the correct amount of gas
    pub(super) const TRANSFER_ETH_TO_NEAR: Gas = 0;

    // TODO: Determine the correct amount of gas
    pub(super) const TRANSFER_NEAR_TO_ETH: Gas = 0;

    pub(super) const GAS_FOR_FT_TRANSFER: Gas = 50_000;
}

pub struct TransferEthToNear;

impl Precompile for TransferEthToNear {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        //TODO
        Ok(costs::TRANSFER_ETH_TO_NEAR)
    }

    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141account, args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            //
            // Input slice format:
            //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens

            (
                String::from_utf8(sdk::current_account_id()).unwrap(),
                NEP41TransferCallArgs {
                    receiver_id: String::from_utf8(input.to_vec()).unwrap(),
                    amount: context.apparent_value.as_u128(),
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

            //TODO: add method in Aurora connector and call promise `get_near_account_for_evm_token(context.caller)`
            let nep141address = context.caller.to_string();

            let mut input = input;

            let mut sender = [0u8; 20];
            sender.copy_from_slice(&input[..20]);
            input = &input[20..];

            let amount = U256::from_little_endian(&input[..32]).as_u128();
            input = &input[32..];

            let receiver_account_id: AccountId = String::from_utf8(input.to_vec()).unwrap();

            (
                nep141address,
                NEP41TransferCallArgs {
                    receiver_id: receiver_account_id,
                    amount,
                    memo: None,
                },
            )
        };

        let promise0 = sdk::promise_create(
            nep141account,
            b"ft_transfer",
            BorshSerialize::try_to_vec(&args).ok().unwrap().as_slice(),
            0,
            costs::GAS_FOR_FT_TRANSFER,
        );

        sdk::promise_return(promise0);

        Ok((ExitSucceed::Returned, Vec::new(), 0))
    }
}

pub struct TransferNearToEth;

impl Precompile for TransferNearToEth {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::TRANSFER_NEAR_TO_ETH)
    }

    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        // TODO: Determine the correct amount of gas
        const GAS_FOR_WITHDRAW: Gas = 50_000;
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141account, serialized_args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            //
            // Input slice format:
            //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

            let eth_recipient: AccountId = String::from_utf8(input.to_vec()).unwrap();
            let args = BridgedTokenWithdrawEthConnectorArgs {
                amount: context.apparent_value.as_u128(),
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

            //TODO: add method in Aurora connector and call promise `get_near_account_for_evm_token(context.caller)`
            let nep141address = context.caller.to_string();

            let mut input = input;

            let mut sender = [0u8; 20];
            sender.copy_from_slice(&input[..20]);
            input = &input[20..];

            let amount = U256::from_little_endian(&input[..32]).as_u128();
            input = &input[32..];

            let eth_recipient: AccountId = String::from_utf8(input.to_vec()).unwrap();
            let args = BridgedTokenWithdrawArgs {
                recipient: eth_recipient,
                amount,
            };

            (
                nep141address,
                BorshSerialize::try_to_vec(&args).ok().unwrap(),
            )
        };

        let promise0 = sdk::promise_create(
            nep141account,
            b"withdraw",
            serialized_args.as_slice(),
            0,
            GAS_FOR_WITHDRAW,
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
