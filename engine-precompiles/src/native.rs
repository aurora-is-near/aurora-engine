use super::{EvmPrecompileResult, Precompile};
#[cfg(feature = "contract")]
use crate::prelude::{
    format, is_valid_account_id,
    parameters::{PromiseCreateArgs, WithdrawCallArgs},
    sdk,
    storage::{bytes_to_key, KeyPrefix},
    types::AccountId,
    vec, BorshSerialize, Cow, String, ToString, TryInto, Vec, U256,
};

use crate::prelude::Address;
use crate::PrecompileOutput;
#[cfg(feature = "contract")]
use evm::backend::Log;
use evm::{Context, ExitError};

const ERR_TARGET_TOKEN_NOT_FOUND: &str = "Target token not found";

mod costs {
    use crate::prelude::types::Gas;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const EXIT_TO_NEAR_GAS: Gas = 0;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const EXIT_TO_ETHEREUM_GAS: Gas = 0;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const FT_TRANSFER_GAS: Gas = 100_000_000_000_000;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const WITHDRAWAL_GAS: Gas = 100_000_000_000_000;
}

pub struct ExitToNear; //TransferEthToNear

impl ExitToNear {
    /// Exit to NEAR precompile address
    ///
    /// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
    /// This address is computed as: `&keccak("exitToNear")[12..]`
    pub const ADDRESS: Address =
        super::make_address(0xe9217bc7, 0x0b7ed1f598ddd3199e80b093fa71124f);
}

#[cfg(feature = "contract")]
fn get_nep141_from_erc20(erc20_token: &[u8]) -> AccountId {
    AccountId::from_utf8(
        sdk::read_storage(bytes_to_key(KeyPrefix::Erc20Nep141Map, erc20_token).as_slice())
            .expect(ERR_TARGET_TOKEN_NOT_FOUND),
    )
    .unwrap()
}

impl Precompile for ExitToNear {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::EXIT_TO_NEAR_GAS)
    }

    #[cfg(not(feature = "contract"))]
    fn run(
        input: &[u8],
        target_gas: Option<u64>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        Ok(PrecompileOutput::default().into())
    }

    #[cfg(feature = "contract")]
    fn run(
        input: &[u8],
        target_gas: Option<u64>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // It's not allowed to call exit precompiles in static mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        }

        // First byte of the input is a flag, selecting the behavior to be triggered:
        //      0x0 -> Eth transfer
        //      0x1 -> Erc20 transfer
        let mut input = input;
        let flag = input[0];
        input = &input[1..];

        let (nep141_address, args) = match flag {
            0x0 => {
                // ETH transfer
                //
                // Input slice format:
                //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens

                if is_valid_account_id(input) {
                    (
                        String::from_utf8(sdk::current_account_id()).unwrap(),
                        // There is no way to inject json, given the encoding of both arguments
                        // as decimal and valid account id respectively.
                        format!(
                            r#"{{"receiver_id": "{}", "amount": "{}", "memo": null}}"#,
                            String::from_utf8(input.to_vec()).unwrap(),
                            context.apparent_value.as_u128()
                        ),
                    )
                } else {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                    )));
                }
            }
            0x1 => {
                // ERC20 transfer
                //
                // This precompile branch is expected to be called from the ERC20 burn function\
                //
                // Input slice format:
                //      amount (U256 big-endian bytes) - the amount that was burned
                //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 tokens

                if context.apparent_value != U256::from(0) {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_ETH_ATTACHED_FOR_ERC20_EXIT",
                    )));
                }

                let nep141_address = get_nep141_from_erc20(context.caller.as_bytes());

                let amount = U256::from_big_endian(&input[..32]).as_u128();
                input = &input[32..];

                if is_valid_account_id(input) {
                    let receiver_account_id: AccountId = String::from_utf8(input.to_vec()).unwrap();
                    (
                        nep141_address,
                        // There is no way to inject json, given the encoding of both arguments
                        // as decimal and valid account id respectively.
                        format!(
                            r#"{{"receiver_id": "{}", "amount": "{}", "memo": null}}"#,
                            receiver_account_id, amount
                        ),
                    )
                } else {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                    )));
                }
            }
            _ => return Err(ExitError::Other(Cow::from("ERR_INVALID_FLAG"))),
        };

        let promise: Vec<u8> = PromiseCreateArgs {
            target_account_id: nep141_address,
            method: "ft_transfer".to_string(),
            args: args.as_bytes().to_vec(),
            attached_balance: 1,
            attached_gas: costs::FT_TRANSFER_GAS,
        }
        .try_to_vec()
        .unwrap();
        let log = Log {
            address: Self::ADDRESS,
            topics: Vec::new(),
            data: promise,
        };

        Ok(PrecompileOutput {
            logs: vec![log],
            ..Default::default()
        }
        .into())
    }
}

pub struct ExitToEthereum;

impl ExitToEthereum {
    /// Exit to Ethereum precompile address
    ///
    /// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
    /// This address is computed as: `&keccak("exitToEthereum")[12..]`
    pub const ADDRESS: Address =
        super::make_address(0xb0bd02f6, 0xa392af548bdf1cfaee5dfa0eefcc8eab);
}

impl Precompile for ExitToEthereum {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        Ok(costs::EXIT_TO_ETHEREUM_GAS)
    }

    #[cfg(not(feature = "contract"))]
    fn run(
        input: &[u8],
        target_gas: Option<u64>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        Ok(PrecompileOutput::default().into())
    }

    #[cfg(feature = "contract")]
    fn run(
        input: &[u8],
        target_gas: Option<u64>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // It's not allowed to call exit precompiles in static mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        }

        // First byte of the input is a flag, selecting the behavior to be triggered:
        //      0x0 -> Eth transfer
        //      0x1 -> Erc20 transfer
        let mut input = input;
        let flag = input[0];
        input = &input[1..];

        let (nep141_address, serialized_args) = match flag {
            0x0 => {
                // ETH transfer
                //
                // Input slice format:
                //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum
                (
                    String::from_utf8(sdk::current_account_id()).unwrap(),
                    // There is no way to inject json, given the encoding of both arguments
                    // as decimal and hexadecimal respectively.
                    WithdrawCallArgs {
                        recipient_address: input.try_into().map_err(|_| {
                            ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS"))
                        })?,
                        amount: context.apparent_value.as_u128(),
                    }
                    .try_to_vec()
                    .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")))?,
                )
            }
            0x1 => {
                // ERC-20 transfer
                //
                // This precompile branch is expected to be called from the ERC20 withdraw function
                // (or burn function with some flag provided that this is expected to be withdrawn)
                //
                // Input slice format:
                //      amount (U256 big-endian bytes) - the amount that was burned
                //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

                if context.apparent_value != U256::from(0) {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_ETH_ATTACHED_FOR_ERC20_EXIT",
                    )));
                }

                let nep141_address = get_nep141_from_erc20(context.caller.as_bytes());

                let amount = U256::from_big_endian(&input[..32]).as_u128();
                input = &input[32..];

                if input.len() == 20 {
                    // Parse ethereum address in hex
                    let eth_recipient: String = hex::encode(input.to_vec());

                    (
                        nep141_address,
                        // There is no way to inject json, given the encoding of both arguments
                        // as decimal and hexadecimal respectively.
                        format!(
                            r#"{{"amount": "{}", "recipient": "{}"}}"#,
                            amount, eth_recipient
                        )
                        .as_bytes()
                        .to_vec(),
                    )
                } else {
                    return Err(ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")));
                }
            }
            _ => {
                return Err(ExitError::Other(Cow::from(
                    "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                )));
            }
        };

        let promise = PromiseCreateArgs {
            target_account_id: nep141_address,
            method: "withdraw".to_string(),
            args: serialized_args,
            attached_balance: 1,
            attached_gas: costs::WITHDRAWAL_GAS,
        }
        .try_to_vec()
        .unwrap();
        let log = Log {
            address: Self::ADDRESS,
            topics: Vec::new(),
            data: promise,
        };

        Ok(PrecompileOutput {
            logs: vec![log],
            ..Default::default()
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::{ExitToEthereum, ExitToNear};
    use crate::prelude::sdk::types::near_account_to_evm_address;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            ExitToEthereum::ADDRESS,
            near_account_to_evm_address("exitToEthereum".as_bytes())
        );
        assert_eq!(
            ExitToNear::ADDRESS,
            near_account_to_evm_address("exitToNear".as_bytes())
        );
    }
}
