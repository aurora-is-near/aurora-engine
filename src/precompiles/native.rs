use evm::{Context, ExitError, ExitSucceed};

use super::{Precompile, PrecompileResult};
use crate::precompiles::PrecompileOutput;
use crate::prelude::Vec;
#[cfg(feature = "exit-precompiles")]
use crate::{
    prelude::{is_valid_account_id, Cow, String, U256},
    types::AccountId,
};

#[cfg(feature = "exit-precompiles")]
mod costs {
    use crate::types::Gas;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const EXIT_TO_NEAR_GAS: Gas = 0;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const EXIT_TO_ETHEREUM_GAS: Gas = 0;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const FT_TRANSFER_GAS: Gas = 100_000_000_000_000;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const WITHDRAWAL_GAS: Gas = 100_000_000_000_000;
}

/// Get the current nep141 token associated with the current erc20 token.
/// This will fail is none is associated.
#[cfg(feature = "exit-precompiles")]
fn get_nep141_from_erc20(_erc20_token: &[u8]) -> Vec<u8> {
    // TODO(#51): Already implemented
    Vec::new()
}

pub struct ExitToNear; //TransferEthToNear

impl ExitToNear {
    /// Exit to NEAR precompile address
    ///
    /// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
    /// This address is computed as: `&keccak("exitToNear")[12..]`
    pub(super) const ADDRESS: [u8; 20] =
        super::make_address(0xe9217bc7, 0x0b7ed1f598ddd3199e80b093fa71124f);
}

impl Precompile for ExitToNear {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::EXIT_TO_NEAR_GAS)
    }

    #[cfg(not(feature = "exit-precompiles"))]
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        Ok((ExitSucceed::Returned, Vec::new(), 0))
    }

    #[cfg(feature = "exit-precompiles")]
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141_address, args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            //
            // Input slice format:
            //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens

            if is_valid_account_id(input) {
                (
                    crate::sdk::current_account_id(),
                    crate::prelude::format!(
                        r#"{{"receiver_id": "{}", "amount": "{}"}}"#,
                        String::from_utf8(input.to_vec()).unwrap(),
                        context.apparent_value.as_u128()
                    ),
                )
            } else {
                return Err(ExitError::Other(Cow::from(
                    "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                )));
            }
        } else {
            // ERC20 transfer
            //
            // This precompile branch is expected to be called from the ERC20 burn function\
            //
            // Input slice format:
            //      amount (U256 le bytes) - the amount that was burned
            //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 tokens

            let nep141_address = get_nep141_from_erc20(context.caller.as_bytes());

            let mut input_mut = input;
            let amount = U256::from_big_endian(&input_mut[..32]).as_u128();
            input_mut = &input_mut[32..];

            // TODO: You have to charge caller's account balance for this transfer.

            if is_valid_account_id(input_mut) {
                let receiver_account_id: AccountId = String::from_utf8(input_mut.to_vec()).unwrap();
                (
                    nep141_address,
                    crate::prelude::format!(
                        r#"{{"receiver_id": "{}", "amount": "{}"}}"#,
                        receiver_account_id,
                        amount
                    ),
                )
            } else {
                return Err(ExitError::Other(Cow::from(
                    "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                )));
            }
        };

        let promise0 = crate::sdk::promise_create(
            &nep141_address,
            b"ft_transfer",
            args.as_bytes(),
            1,
            costs::FT_TRANSFER_GAS,
        );

        crate::sdk::promise_return(promise0);

        Ok(PrecompileOutput::default())
    }
}

pub struct ExitToEthereum;

impl ExitToEthereum {
    /// Exit to Ethereum precompile address
    ///
    /// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
    /// This address is computed as: `&keccak("exitToEthereum")[12..]`
    pub(super) const ADDRESS: [u8; 20] =
        super::make_address(0xb0bd02f6, 0xa392af548bdf1cfaee5dfa0eefcc8eab);
}

impl Precompile for ExitToEthereum {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::EXIT_TO_ETHEREUM_GAS)
    }

    #[cfg(not(feature = "exit-precompiles"))]
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        Ok((ExitSucceed::Returned, Vec::new(), 0))
    }

    #[cfg(feature = "exit-precompiles")]
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let (nep141_address, serialized_args) = if context.apparent_value != U256::from(0) {
            // ETH transfer
            //
            // Input slice format:
            //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

            let eth_recipient: String = hex::encode(input);

            if eth_recipient.len() == 20 {
                (
                    crate::sdk::current_account_id(),
                    crate::prelude::format!(
                        r#"{{"amount": "{}", "recipient": "{}"}}"#,
                        context.apparent_value.as_u128(),
                        eth_recipient
                    ),
                )
            } else {
                return Err(ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")));
            }
        } else {
            // ERC-20 transfer
            //
            // This precompile branch is expected to be called from the ERC20 withdraw function
            // (or burn function with some flag provided that this is expected to be withdrawn)
            //
            // Input slice format:
            //      amount (U256 le bytes) - the amount that was burned
            //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

            let nep141_address = get_nep141_from_erc20(context.caller.as_bytes());

            let mut input_mut = input;

            let amount = U256::from_big_endian(&input_mut[..32]).as_u128();
            input_mut = &input_mut[32..];

            // TODO: Charge the caller's account balance?

            if input_mut.len() == 20 {
                // Parse ethereum address in hex
                let eth_recipient: String = hex::encode(input_mut.to_vec());

                (
                    nep141_address,
                    crate::prelude::format!(
                        r#"{{"amount": "{}", "recipient": "{}"}}"#,
                        amount,
                        eth_recipient
                    ),
                )
            } else {
                return Err(ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")));
            }
        };

        let promise0 = crate::sdk::promise_create(
            &nep141_address,
            b"withdraw",
            serialized_args.as_bytes(),
            1,
            costs::WITHDRAWAL_GAS,
        );

        crate::sdk::promise_return(promise0);

        Ok(PrecompileOutput::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{ExitToEthereum, ExitToNear};
    use crate::types::near_account_to_evm_address;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            ExitToEthereum::ADDRESS,
            near_account_to_evm_address("exitToEthereum".as_bytes()).0
        );
        assert_eq!(
            ExitToNear::ADDRESS,
            near_account_to_evm_address("exitToNear".as_bytes()).0
        );
    }
}
