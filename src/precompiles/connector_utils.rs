use crate::precompiles::{Precompile, PrecompileResult};
use crate::prelude::{mem, Borrowed, TryInto};

use evm::{Context, ExitError, ExitSucceed};

pub struct ExitToNear;

impl Precompile for ExitToNear {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        //TODO
        Ok(0)
    }

    #[cfg(feature = "contract")]
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        use alloc::string::String;
        use borsh::BorshSerialize;

        use crate::sdk;
        use crate::types::{AccountId, Gas};
        use crate::parameters::TransferCallArgs;

        const GAS_FOR_FT_TRANSFER: Gas = 50_000;

        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141Account, args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            // Input slice format: [
            // recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens
            // ]

            //TODO: check for possible overflows
            let amount = context.apparent_value.as_u128();

            let receiver_account_id: AccountId = String::from_utf8(
                input.to_vec()
            ).unwrap();

            (
                sdk::current_account_id(),
                TransferCallArgs {
                    receiver_id: receiver_account_id,
                    amount: amount,
                    memo: None,
                }
            )

        } else {
            // This precompile branch is expected to be called from the ERC20 burn function
            // Input slice format: [
            // sender (20 bytes) - Eth address of the address that burned his erc20 tokens
            // amount (U256 le bytes) - the amount that was burned
            // recipient_account_id (bytes) - the NEAR recipient account which will receieve NEP-141 tokens
            // ]

            let mut cursor = 0;
            //TODO: add method in Aurora connector and call promise `get_near_account_for_evm_token(context.caller)`
            let nep141Address = context.caller.to_string();

            let _sender = [0u8; 20];
            _sender.copy_from_slice(&input[cursor..cursor + 20]);
            cursor += 20;

            let mut amount_bytes = [0u8; 32];
            amount_bytes.copy_from_slice(&input[cursor..cursor + 32]);
            //TODO: check for possible overflows
            let amount = U256::from_little_endian(&amount_bytes).as_u128();
            cursor += 32;

            let receiver_account_id: AccountId = String::from_utf8(
                (&input[cursor..]).to_vec()
            ).unwrap();

            (
                nep141Address,
                TransferCallArgs {
                    receiver_id: receiver_account_id,
                    amount: amount,
                    memo: None,
                }
            )
        };

        let promise0 = sdk::promise_create(
            &nep141Account,
            b"ft_transfer",
            BorshSerialize::try_to_vec(&args).ok().unwrap().as_slice(),
            0,
            GAS_FOR_FT_TRANSFER,
        );
        sdk::promise_return(promise0);

        Ok((ExitSucceed::Returned, vec![], 0))
    }

    #[cfg(not(feature = "contract"))]
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        //TODO
        Ok((ExitSucceed::Returned, vec![], 0))
    }
}

pub struct ExitToEthereum;

impl Precompile for ExitToEthereum {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        //TODO
        Ok(0)
    }

    #[cfg(feature = "contract")]
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        use alloc::string::String;
        use borsh::BorshSerialize;

        use crate::sdk;
        use crate::types::{AccountId, Gas};
        use crate::parameters::WithdrawCallArgs;

        const GAS_FOR_WITHDRAW: Gas = 50_000;

        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141Account, args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            // Input slice format: [
            // eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum
            // ]

            //TODO: check for possible overflows
            let amount = context.apparent_value.as_u128();

            let eth_recipient: AccountId = String::from_utf8(
                input.to_vec()
            ).unwrap();

            (
                sdk::current_account_id(),
                WithdrawCallArgs {
                    recipient_id: eth_recipient,
                    amount: amount,
                }
            )

        } else {
            // This precompile branch is expected to be called from the ERC20 withdraw function
            // (or burn function with some flag provided that this is expected to be withdrawn)
            // Input slice format: [
            // sender (20 bytes) - Eth address of the address that burned his erc20 tokens
            // amount (U256 le bytes) - the amount that was burned
            // eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum
            // ]

            let mut cursor = 0;
            //TODO: add method in Aurora connector and call promise `get_near_account_for_evm_token(context.caller)`
            let nep141Address = context.caller.to_string();

            let _sender = [0u8; 20];
            _sender.copy_from_slice(&input[cursor..cursor + 20]);
            cursor += 20;

            let mut amount_bytes = [0u8; 32];
            amount_bytes.copy_from_slice(&input[cursor..cursor + 32]);
            //TODO: check for possible overflows
            let amount = U256::from_little_endian(&amount_bytes).as_u128();
            cursor += 32;

            let eth_recipient: AccountId = String::from_utf8(
                (&input[cursor..]).to_vec()
            ).unwrap();

            (
                nep141Address,
                WithdrawCallArgs {
                    recipient_id: eth_recipient,
                    amount: amount,
                }
            )
        };

        let promise0 = sdk::promise_create(
            &nep141Account,
            b"withdraw",
            BorshSerialize::try_to_vec(&args).ok().unwrap().as_slice(),
            0,
            GAS_FOR_WITHDRAW,
        );
        sdk::promise_return(promise0);

        Ok((ExitSucceed::Returned, vec![], 0))
    }

    #[cfg(not(feature = "contract"))]
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        //TODO
        Ok((ExitSucceed::Returned, vec![], 0))
    }
}
