use super::{EvmPrecompileResult, Precompile};
use crate::prelude::{
    format,
    parameters::{PromiseArgs, PromiseCreateArgs, WithdrawCallArgs},
    sdk::io::{StorageIntermediate, IO},
    storage::{bytes_to_key, KeyPrefix},
    types::{Address, Yocto},
    vec, BorshSerialize, Cow, String, ToString, Vec, U256,
};
#[cfg(feature = "error_refund")]
use crate::prelude::{
    parameters::{PromiseWithCallbackArgs, RefundCallArgs},
    types,
};

use crate::prelude::types::EthGas;
use crate::PrecompileOutput;
use aurora_engine_types::account_id::AccountId;
use evm::backend::Log;
use evm::{Context, ExitError};

const ERR_TARGET_TOKEN_NOT_FOUND: &str = "Target token not found";

mod costs {
    use crate::prelude::types::{EthGas, NearGas};

    // TODO(#483): Determine the correct amount of gas
    pub(super) const EXIT_TO_NEAR_GAS: EthGas = EthGas::new(0);

    // TODO(#483): Determine the correct amount of gas
    pub(super) const EXIT_TO_ETHEREUM_GAS: EthGas = EthGas::new(0);

    /// Value determined experimentally based on tests and mainnet data. Example:
    /// https://explorer.mainnet.near.org/transactions/5CD7NrqWpK3H8MAAU4mYEPuuWz9AqR9uJkkZJzw5b8PM#D1b5NVRrAsJKUX2ZGs3poKViu1Rgt4RJZXtTfMgdxH4S
    pub(super) const FT_TRANSFER_GAS: NearGas = NearGas::new(10_000_000_000_000);

    /// Value determined experimentally based on tests.
    /// (No mainnet data available since this feature is not enabled)
    #[cfg(feature = "error_refund")]
    pub(super) const REFUND_ON_ERROR_GAS: NearGas = NearGas::new(5_000_000_000_000);

    // TODO(#332): Determine the correct amount of gas
    pub(super) const WITHDRAWAL_GAS: NearGas = NearGas::new(100_000_000_000_000);
}

pub mod events {
    use crate::prelude::{types::Address, vec, String, ToString, H160, H256, U256};

    /// Derived from event signature (see tests::test_exit_signatures)
    pub const EXIT_TO_NEAR_SIGNATURE: H256 = crate::make_h256(
        0x5a91b8bc9c1981673db8fb226dbd8fcd,
        0xd0c23f45cd28abb31403a5392f6dd0c7,
    );
    /// Derived from event signature (see tests::test_exit_signatures)
    pub const EXIT_TO_ETH_SIGNATURE: H256 = crate::make_h256(
        0xd046c2bb01a5622bc4b9696332391d87,
        0x491373762eeac0831c48400e2d5a5f07,
    );

    /// The exit precompile events have an `erc20_address` field to indicate
    /// which ERC-20 token is being withdrawn. However, ETH is not an ERC-20 token
    /// So we need to have some other address to fill this field. This constant is
    /// used for this purpose.
    pub const ETH_ADDRESS: Address = Address::new(H160([0; 20]));

    /// ExitToNear(
    ///    Address indexed sender,
    ///    Address indexed erc20_address,
    ///    string indexed dest,
    ///    uint amount
    /// )
    /// Note: in the ERC-20 exit case `sender` == `erc20_address` because it is
    /// the ERC-20 contract which calls the exit precompile. However in the case
    /// of ETH exit the sender will give the true sender (and the `erc20_address`
    /// will not be meaningful because ETH is not an ERC-20 token).
    pub struct ExitToNear {
        pub sender: Address,
        pub erc20_address: Address,
        pub dest: String,
        pub amount: U256,
    }

    impl ExitToNear {
        pub fn encode(self) -> ethabi::RawLog {
            let data = ethabi::encode(&[ethabi::Token::Int(self.amount)]);
            let topics = vec![
                EXIT_TO_NEAR_SIGNATURE,
                encode_address(self.sender),
                encode_address(self.erc20_address),
                aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(self.dest)])),
            ];

            ethabi::RawLog { topics, data }
        }
    }

    /// ExitToEth(
    ///    Address indexed sender,
    ///    Address indexed erc20_address,
    ///    string indexed dest,
    ///    uint amount
    /// )
    /// Note: in the ERC-20 exit case `sender` == `erc20_address` because it is
    /// the ERC-20 contract which calls the exit precompile. However in the case
    /// of ETH exit the sender will give the true sender (and the `erc20_address`
    /// will not be meaningful because ETH is not an ERC-20 token).
    pub struct ExitToEth {
        pub sender: Address,
        pub erc20_address: Address,
        pub dest: Address,
        pub amount: U256,
    }

    impl ExitToEth {
        pub fn encode(self) -> ethabi::RawLog {
            let data = ethabi::encode(&[ethabi::Token::Int(self.amount)]);
            let topics = vec![
                EXIT_TO_ETH_SIGNATURE,
                encode_address(self.sender),
                encode_address(self.erc20_address),
                encode_address(self.dest),
            ];

            ethabi::RawLog { topics, data }
        }
    }

    fn encode_address(a: Address) -> H256 {
        let mut result = [0u8; 32];
        result[12..].copy_from_slice(a.as_bytes());
        H256(result)
    }

    pub fn exit_to_near_schema() -> ethabi::Event {
        ethabi::Event {
            name: "ExitToNear".to_string(),
            inputs: vec![
                ethabi::EventParam {
                    name: "sender".to_string(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "erc20_address".to_string(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "dest".to_string(),
                    kind: ethabi::ParamType::String,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "amount".to_string(),
                    kind: ethabi::ParamType::Uint(256),
                    indexed: false,
                },
            ],
            anonymous: false,
        }
    }

    pub fn exit_to_eth_schema() -> ethabi::Event {
        ethabi::Event {
            name: "ExitToEth".to_string(),
            inputs: vec![
                ethabi::EventParam {
                    name: "sender".to_string(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "erc20_address".to_string(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "dest".to_string(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "amount".to_string(),
                    kind: ethabi::ParamType::Uint(256),
                    indexed: false,
                },
            ],
            anonymous: false,
        }
    }
}

//TransferEthToNear
pub struct ExitToNear<I> {
    current_account_id: AccountId,
    io: I,
}

pub mod exit_to_near {
    use aurora_engine_types::types::Address;

    /// Exit to NEAR precompile address
    ///
    /// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
    /// This address is computed as: `&keccak("exitToNear")[12..]`
    pub const ADDRESS: Address =
        crate::make_address(0xe9217bc7, 0x0b7ed1f598ddd3199e80b093fa71124f);
}

impl<I> ExitToNear<I> {
    pub fn new(current_account_id: AccountId, io: I) -> Self {
        Self {
            current_account_id,
            io,
        }
    }
}

fn get_nep141_from_erc20<I: IO>(erc20_token: &[u8], io: &I) -> Result<AccountId, ExitError> {
    AccountId::try_from(
        io.read_storage(bytes_to_key(KeyPrefix::Erc20Nep141Map, erc20_token).as_slice())
            .map(|s| s.to_vec())
            .ok_or(ExitError::Other(Cow::Borrowed(ERR_TARGET_TOKEN_NOT_FOUND)))?,
    )
    .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_INVALID_NEP141_ACCOUNT")))
}

impl<I: IO> Precompile for ExitToNear<I> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::EXIT_TO_NEAR_GAS)
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        #[cfg(feature = "error_refund")]
        fn parse_input(input: &[u8]) -> Result<(Address, &[u8]), ExitError> {
            if input.len() < 21 {
                return Err(ExitError::Other(Cow::from("ERR_INVALID_INPUT")));
            }
            let refund_address = Address::try_from_slice(&input[1..21]).unwrap();
            Ok((refund_address, &input[21..]))
        }
        #[cfg(not(feature = "error_refund"))]
        fn parse_input(input: &[u8]) -> &[u8] {
            &input[1..]
        }

        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // It's not allowed to call exit precompiles in static mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        } else if context.address != exit_to_near::ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_DELEGATE")));
        }

        // First byte of the input is a flag, selecting the behavior to be triggered:
        //      0x0 -> Eth transfer
        //      0x1 -> Erc20 transfer
        let flag = input[0];
        #[cfg(feature = "error_refund")]
        let (refund_address, mut input) = parse_input(input)?;
        #[cfg(not(feature = "error_refund"))]
        let mut input = parse_input(input);
        let current_account_id = self.current_account_id.clone();
        #[cfg(feature = "error_refund")]
        let refund_on_error_target = current_account_id.clone();

        let (nep141_address, args, exit_event) = match flag {
            0x0 => {
                // ETH transfer
                //
                // Input slice format:
                //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens

                if let Ok(dest_account) = AccountId::try_from(input) {
                    (
                        current_account_id,
                        // There is no way to inject json, given the encoding of both arguments
                        // as decimal and valid account id respectively.
                        format!(
                            r#"{{"receiver_id": "{}", "amount": "{}", "memo": null}}"#,
                            dest_account,
                            context.apparent_value.as_u128()
                        ),
                        events::ExitToNear {
                            sender: Address::new(context.caller),
                            erc20_address: events::ETH_ADDRESS,
                            dest: dest_account.to_string(),
                            amount: context.apparent_value,
                        },
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

                let erc20_address = context.caller;
                let nep141_address = get_nep141_from_erc20(erc20_address.as_bytes(), &self.io)?;

                let amount = U256::from_big_endian(&input[..32]);
                input = &input[32..];

                if let Ok(receiver_account_id) = AccountId::try_from(input) {
                    (
                        nep141_address,
                        // There is no way to inject json, given the encoding of both arguments
                        // as decimal and valid account id respectively.
                        format!(
                            r#"{{"receiver_id": "{}", "amount": "{}", "memo": null}}"#,
                            receiver_account_id,
                            amount.as_u128()
                        ),
                        events::ExitToNear {
                            sender: Address::new(erc20_address),
                            erc20_address: Address::new(erc20_address),
                            dest: receiver_account_id.to_string(),
                            amount,
                        },
                    )
                } else {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                    )));
                }
            }
            _ => return Err(ExitError::Other(Cow::from("ERR_INVALID_FLAG"))),
        };

        #[cfg(feature = "error_refund")]
        let erc20_address = if flag == 0 {
            None
        } else {
            Some(exit_event.erc20_address)
        };
        #[cfg(feature = "error_refund")]
        let refund_args = RefundCallArgs {
            recipient_address: refund_address,
            erc20_address,
            amount: types::u256_to_arr(&exit_event.amount),
        };
        #[cfg(feature = "error_refund")]
        let refund_promise = PromiseCreateArgs {
            target_account_id: refund_on_error_target,
            method: "refund_on_error".to_string(),
            args: refund_args.try_to_vec().unwrap(),
            attached_balance: Yocto::new(0),
            attached_gas: costs::REFUND_ON_ERROR_GAS,
        };
        let transfer_promise = PromiseCreateArgs {
            target_account_id: nep141_address,
            method: "ft_transfer".to_string(),
            args: args.as_bytes().to_vec(),
            attached_balance: Yocto::new(1),
            attached_gas: costs::FT_TRANSFER_GAS,
        };

        #[cfg(feature = "error_refund")]
        let promise = PromiseArgs::Callback(PromiseWithCallbackArgs {
            base: transfer_promise,
            callback: refund_promise,
        });
        #[cfg(not(feature = "error_refund"))]
        let promise = PromiseArgs::Create(transfer_promise);

        let promise_log = Log {
            address: exit_to_near::ADDRESS.raw(),
            topics: Vec::new(),
            data: promise.try_to_vec().unwrap(),
        };
        let exit_event_log = exit_event.encode();
        let exit_event_log = Log {
            address: exit_to_near::ADDRESS.raw(),
            topics: exit_event_log.topics,
            data: exit_event_log.data,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log, exit_event_log],
            ..Default::default()
        })
    }
}

pub struct ExitToEthereum<I> {
    current_account_id: AccountId,
    io: I,
}

pub mod exit_to_ethereum {
    use aurora_engine_types::types::Address;

    /// Exit to Ethereum precompile address
    ///
    /// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
    /// This address is computed as: `&keccak("exitToEthereum")[12..]`
    pub const ADDRESS: Address =
        crate::make_address(0xb0bd02f6, 0xa392af548bdf1cfaee5dfa0eefcc8eab);
}

impl<I> ExitToEthereum<I> {
    pub fn new(current_account_id: AccountId, io: I) -> Self {
        Self {
            current_account_id,
            io,
        }
    }
}

impl<I: IO> Precompile for ExitToEthereum<I> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::EXIT_TO_ETHEREUM_GAS)
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        use crate::prelude::types::NEP141Wei;
        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // It's not allowed to call exit precompiles in static mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        } else if context.address != exit_to_ethereum::ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_DELEGATE")));
        }

        // First byte of the input is a flag, selecting the behavior to be triggered:
        //      0x0 -> Eth transfer
        //      0x1 -> Erc20 transfer
        let mut input = input;
        let flag = input[0];
        input = &input[1..];

        let (nep141_address, serialized_args, exit_event) = match flag {
            0x0 => {
                // ETH transfer
                //
                // Input slice format:
                //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum
                let recipient_address: Address = input
                    .try_into()
                    .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")))?;
                (
                    self.current_account_id.clone(),
                    // There is no way to inject json, given the encoding of both arguments
                    // as decimal and hexadecimal respectively.
                    WithdrawCallArgs {
                        recipient_address,
                        amount: NEP141Wei::new(context.apparent_value.as_u128()),
                    }
                    .try_to_vec()
                    .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")))?,
                    events::ExitToEth {
                        sender: Address::new(context.caller),
                        erc20_address: events::ETH_ADDRESS,
                        dest: recipient_address,
                        amount: context.apparent_value,
                    },
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

                let erc20_address = context.caller;
                let nep141_address = get_nep141_from_erc20(erc20_address.as_bytes(), &self.io)?;

                let amount = U256::from_big_endian(&input[..32]);
                input = &input[32..];

                if input.len() == 20 {
                    // Parse ethereum address in hex
                    let eth_recipient: String = hex::encode(input);
                    // unwrap cannot fail since we checked the length already
                    let recipient_address = Address::try_from_slice(input).map_err(|_| {
                        ExitError::Other(crate::prelude::Cow::from("ERR_WRONG_ADDRESS"))
                    })?;

                    (
                        nep141_address,
                        // There is no way to inject json, given the encoding of both arguments
                        // as decimal and hexadecimal respectively.
                        format!(
                            r#"{{"amount": "{}", "recipient": "{}"}}"#,
                            amount.as_u128(),
                            eth_recipient
                        )
                        .into_bytes(),
                        events::ExitToEth {
                            sender: Address::new(erc20_address),
                            erc20_address: Address::new(erc20_address),
                            dest: recipient_address,
                            amount,
                        },
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

        let withdraw_promise = PromiseCreateArgs {
            target_account_id: nep141_address,
            method: "withdraw".to_string(),
            args: serialized_args,
            attached_balance: Yocto::new(1),
            attached_gas: costs::WITHDRAWAL_GAS,
        };

        let promise = PromiseArgs::Create(withdraw_promise).try_to_vec().unwrap();
        let promise_log = Log {
            address: exit_to_ethereum::ADDRESS.raw(),
            topics: Vec::new(),
            data: promise,
        };
        let exit_event_log = exit_event.encode();
        let exit_event_log = Log {
            address: exit_to_ethereum::ADDRESS.raw(),
            topics: exit_event_log.topics,
            data: exit_event_log.data,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log, exit_event_log],
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{exit_to_ethereum, exit_to_near};
    use crate::prelude::sdk::types::near_account_to_evm_address;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            exit_to_ethereum::ADDRESS,
            near_account_to_evm_address("exitToEthereum".as_bytes())
        );
        assert_eq!(
            exit_to_near::ADDRESS,
            near_account_to_evm_address("exitToNear".as_bytes())
        );
    }

    #[test]
    fn test_exit_signatures() {
        let exit_to_near = super::events::exit_to_near_schema();
        let exit_to_eth = super::events::exit_to_eth_schema();

        assert_eq!(
            exit_to_near.signature(),
            super::events::EXIT_TO_NEAR_SIGNATURE
        );
        assert_eq!(
            exit_to_eth.signature(),
            super::events::EXIT_TO_ETH_SIGNATURE
        );
    }
}
