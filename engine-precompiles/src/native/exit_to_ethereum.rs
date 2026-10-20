use aurora_engine_types::{
    account_id::AccountId,
    borsh,
    parameters::connector::{WithdrawCallArgs, WithdrawSerializeType},
    storage::EthConnectorStorageId,
    types::NEP141Wei,
};
use aurora_evm::backend::Log;
use aurora_evm::{Context, ExitError};

use super::{
    construct_contract_key, events, get_eth_connector_contract_account, get_nep141_from_erc20,
    parse_amount, to_u128, validate_input_size,
};
use crate::prelude::types::{EthGas, make_address};
use crate::prelude::{
    Cow, H256, ToString, U256, Vec, format,
    parameters::{PromiseArgs, PromiseCreateArgs},
    sdk::io::{IO, StorageIntermediate},
    types::{Address, Yocto},
    vec,
};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};

mod costs {
    use crate::prelude::types::{EthGas, NearGas};

    // TODO(#483): Determine the correct amount of gas
    pub(super) const EXIT_TO_ETHEREUM_GAS: EthGas = EthGas::new(0);

    // TODO(#332): Determine the correct amount of gas
    pub(super) const WITHDRAWAL_GAS: NearGas = NearGas::new(100_000_000_000_000);
}

/// Exit to Ethereum precompile address
///
/// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
/// This address is computed as: `&keccak("exitToEthereum")[12..]`
pub const ADDRESS: Address = make_address(0xb0bd02f6, 0xa392af548bdf1cfaee5dfa0eefcc8eab);

pub struct ExitToEthereum<I> {
    io: I,
}

struct ExitPlan {
    target_account_id: AccountId,
    args: Vec<u8>,
    event: events::ExitToEth,
}

impl<I> ExitToEthereum<I> {
    pub const fn new(io: I) -> Self {
        Self { io }
    }
}

fn get_withdraw_serialize_type<I: IO>(io: &I) -> Result<WithdrawSerializeType, ExitError> {
    io.read_storage(&construct_contract_key(
        EthConnectorStorageId::WithdrawSerializationType,
    ))
    .map_or(Ok(WithdrawSerializeType::Borsh), |value| {
        value
            .to_value()
            .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_DESERIALIZE")))
    })
}

fn json_args(address: Address, amount: u128) -> Vec<u8> {
    // There is no way to inject JSON, given the encoding of both arguments
    // as decimal and hexadecimal respectively.
    format!(
        r#"{{"amount":"{amount}","recipient":"{}"}}"#,
        address.encode(),
    )
    .into_bytes()
}

fn borsh_args(address: Address, amount: u128) -> Result<Vec<u8>, ExitError> {
    borsh::to_vec(&WithdrawCallArgs {
        recipient_address: address,
        amount: NEP141Wei::new(amount),
    })
    .map_err(|_| ExitError::Other(Cow::from("ERR_BORSH_SERIALIZE")))
}

impl<I: IO> Precompile for ExitToEthereum<I> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::EXIT_TO_ETHEREUM_GAS)
    }

    #[allow(clippy::too_many_lines)]
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        // ETH (Base token) transfer input format (min size 21 bytes)
        //  - flag (1 byte)
        //  - eth_recipient (20 bytes)
        // ERC-20 transfer input format: max 53 bytes
        //  - flag (1 byte)
        //  - amount (32 bytes)
        //  - eth_recipient (20 bytes)
        validate_input_size(input, 21..=53)?;

        let required_gas = Self::required_gas(input)?;

        if let Some(target_gas) = target_gas
            && required_gas > target_gas
        {
            return Err(ExitError::OutOfGas);
        }

        // It's not allowed to call exit precompiles in static mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        } else if context.address != ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_DELEGATE")));
        }

        // The first byte of the input is a flag, selecting the behavior to be triggered:
        //  0x00 -> ETH (Base token) token transfer
        //  0x01 -> ERC-20 transfer
        let (flag, input) = input
            .split_first()
            .ok_or_else(|| ExitError::Other(Cow::from("ERR_INVALID_INPUT")))?;

        let ExitPlan {
            target_account_id,
            args,
            event,
        } = match *flag {
            0x0 => {
                // ETH (base) transfer
                //
                // Input slice format:
                //  eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum
                let recipient_address: Address = input
                    .try_into()
                    .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")))?;
                let serialize_type = get_withdraw_serialize_type(&self.io)?;
                let eth_connector_account_id = get_eth_connector_contract_account(&self.io)?;
                let amount = to_u128(context.apparent_value)?;
                let args = match serialize_type {
                    WithdrawSerializeType::Json => json_args(recipient_address, amount),
                    WithdrawSerializeType::Borsh => borsh_args(recipient_address, amount)?,
                };

                ExitPlan {
                    target_account_id: eth_connector_account_id,
                    args,
                    event: events::ExitToEth {
                        sender: Address::new(context.caller),
                        erc20_address: events::ETH_ADDRESS,
                        dest: recipient_address,
                        amount: context.apparent_value,
                    },
                }
            }
            0x1 => {
                // ERC-20 transfer
                //
                // This precompile branch is expected to be called from the ERC20 withdraw function
                // (or burn function with some flag provided that this is expected to be withdrawn)
                //
                // Input slice format:
                //  amount (U256 big-endian bytes) - the amount that was burned
                //  eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

                if context.apparent_value != U256::from(0) {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_ETH_ATTACHED_FOR_ERC20_EXIT",
                    )));
                }

                let erc20_address = context.caller;
                let nep141_address = get_nep141_from_erc20(erc20_address.as_bytes(), &self.io)?;
                let (amount_bytes, recipient_bytes) = input
                    .split_at_checked(32)
                    .ok_or_else(|| ExitError::Other(Cow::from("ERR_INVALID_INPUT")))?;
                let amount = parse_amount(amount_bytes)?;
                let recipient_address = Address::try_from_slice(recipient_bytes)
                    .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")))?;

                ExitPlan {
                    target_account_id: nep141_address,
                    // There is no way to inject json, given the encoding of both arguments
                    // as decimal and hexadecimal respectively.
                    args: format!(
                        r#"{{"amount": "{}", "recipient": "{}"}}"#,
                        amount,
                        recipient_address.encode()
                    )
                    .into_bytes(),
                    event: events::ExitToEth {
                        sender: Address::new(erc20_address),
                        erc20_address: Address::new(erc20_address),
                        dest: recipient_address,
                        amount,
                    },
                }
            }
            _ => {
                return Err(ExitError::Other(Cow::from(
                    "ERR_INVALID_RECEIVER_ACCOUNT_ID",
                )));
            }
        };

        let withdraw_promise = PromiseCreateArgs {
            target_account_id,
            method: "withdraw".to_string(),
            args,
            attached_balance: Yocto::new(1),
            attached_gas: costs::WITHDRAWAL_GAS,
        };

        let promise = borsh::to_vec(&PromiseArgs::Create(withdraw_promise)).unwrap();
        let promise_log = Log {
            address: ADDRESS.raw(),
            topics: Vec::new(),
            data: promise,
        };
        let ethabi::RawLog { topics, data } = event.encode();
        let exit_event_log = Log {
            address: ADDRESS.raw(),
            topics: topics.into_iter().map(|h| H256::from(h.0)).collect(),
            data,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log, exit_event_log],
            cost: required_gas,
            output: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ADDRESS, ExitToEthereum};
    use aurora_engine_types::U256;

    #[test]
    fn test_base_token_withdrawal_encoding_and_amount_bounds() {
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::{
            H256,
            account_id::AccountId,
            borsh::{self, BorshDeserialize},
            parameters::{
                PromiseArgs,
                connector::{WithdrawCallArgs, WithdrawSerializeType},
            },
            storage::EthConnectorStorageId,
            types::{Address, NEP141Wei, Yocto},
        };
        use std::cell::RefCell;

        use crate::{Context, ExitError, Precompile, prelude::Cow};

        let connector_account_id: AccountId = "connector.near".parse().unwrap();
        let recipient = Address::from_array([3; 20]);
        let storage = RefCell::new(Storage::default());
        storage.borrow_mut().kv_store.insert(
            super::super::construct_contract_key(EthConnectorStorageId::EthConnectorAccount),
            borsh::to_vec(&connector_account_id).unwrap(),
        );
        let precompile = ExitToEthereum::new(StoragePointer(&storage));
        let context = Context {
            address: ADDRESS.raw(),
            caller: Address::from_array([2; 20]).raw(),
            apparent_value: U256::from(u128::MAX),
        };
        let input = [&[0][..], recipient.as_bytes()].concat();
        let serialize_type_key =
            super::super::construct_contract_key(EthConnectorStorageId::WithdrawSerializationType);
        let overflow_context = Context {
            apparent_value: U256::from(u128::MAX) + 1,
            ..context
        };

        for serialize_type in [WithdrawSerializeType::Json, WithdrawSerializeType::Borsh] {
            storage.borrow_mut().kv_store.insert(
                serialize_type_key.clone(),
                borsh::to_vec(&serialize_type).unwrap(),
            );
            let output = precompile.run(&input, None, &context, false).unwrap();
            assert_eq!(output.logs.len(), 2);
            let PromiseArgs::Create(call) =
                PromiseArgs::try_from_slice(&output.logs[0].data).unwrap()
            else {
                panic!("expected a single withdraw promise");
            };
            assert_eq!(call.target_account_id, connector_account_id);
            assert_eq!(call.method, "withdraw");
            assert_eq!(call.attached_balance, Yocto::new(1));
            assert_eq!(call.attached_gas, super::costs::WITHDRAWAL_GAS);

            let expected_args = match serialize_type {
                WithdrawSerializeType::Json => format!(
                    r#"{{"amount":"{}","recipient":"{}"}}"#,
                    u128::MAX,
                    recipient.encode()
                )
                .into_bytes(),
                WithdrawSerializeType::Borsh => borsh::to_vec(&WithdrawCallArgs {
                    recipient_address: recipient,
                    amount: NEP141Wei::new(u128::MAX),
                })
                .unwrap(),
            };
            assert_eq!(call.args, expected_args);

            let expected_event = super::events::ExitToEth {
                sender: Address::new(context.caller),
                erc20_address: super::events::ETH_ADDRESS,
                dest: recipient,
                amount: context.apparent_value,
            }
            .encode();
            assert_eq!(output.logs[1].data, expected_event.data);
            assert_eq!(
                output.logs[1].topics,
                expected_event
                    .topics
                    .into_iter()
                    .map(|topic| H256::from(topic.0))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                precompile
                    .run(&input, None, &overflow_context, false)
                    .unwrap_err(),
                ExitError::Other(Cow::from("ERR_INVALID_AMOUNT"))
            );
        }
    }

    #[test]
    fn test_erc20_withdrawal_promise_encoding() {
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::{
            H256,
            borsh::BorshDeserialize,
            parameters::PromiseArgs,
            storage::{KeyPrefix, bytes_to_key},
            types::{Address, Yocto},
        };
        use std::cell::RefCell;

        use crate::{Context, Precompile};

        let storage = RefCell::new(Storage::default());
        let caller = Address::from_array([2; 20]);
        let recipient = Address::from_array([3; 20]);
        let context = Context {
            address: ADDRESS.raw(),
            caller: caller.raw(),
            apparent_value: U256::zero(),
        };
        storage.borrow_mut().kv_store.insert(
            bytes_to_key(KeyPrefix::Erc20Nep141Map, caller.as_bytes()),
            b"token.near".to_vec(),
        );
        let amount = U256::from(u128::MAX);
        let amount_bytes = amount.to_big_endian();
        let input = [&[1][..], amount_bytes.as_slice(), recipient.as_bytes()].concat();
        let precompile = ExitToEthereum::new(StoragePointer(&storage));

        let output = precompile.run(&input, None, &context, false).unwrap();
        assert_eq!(output.logs.len(), 2);
        let PromiseArgs::Create(call) = PromiseArgs::try_from_slice(&output.logs[0].data).unwrap()
        else {
            panic!("expected a single withdraw promise");
        };
        assert_eq!(call.target_account_id, "token.near".parse().unwrap());
        assert_eq!(call.method, "withdraw");
        assert_eq!(call.attached_balance, Yocto::new(1));
        assert_eq!(call.attached_gas, super::costs::WITHDRAWAL_GAS);
        assert_eq!(
            call.args,
            format!(
                r#"{{"amount": "{}", "recipient": "{}"}}"#,
                u128::MAX,
                recipient.encode()
            )
            .into_bytes()
        );

        let expected_event = super::events::ExitToEth {
            sender: caller,
            erc20_address: caller,
            dest: recipient,
            amount,
        }
        .encode();
        assert_eq!(output.logs[1].data, expected_event.data);
        assert_eq!(
            output.logs[1].topics,
            expected_event
                .topics
                .into_iter()
                .map(|topic| H256::from(topic.0))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_withdrawal_error_order() {
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::{
            storage::{KeyPrefix, bytes_to_key},
            types::Address,
        };
        use std::cell::RefCell;

        use crate::{Context, ExitError, Precompile, prelude::Cow};

        let storage = RefCell::new(Storage::default());
        let precompile = ExitToEthereum::new(StoragePointer(&storage));
        let mut context = Context {
            address: ADDRESS.raw(),
            caller: Address::from_array([2; 20]).raw(),
            apparent_value: U256::from(1),
        };

        assert_eq!(
            precompile.run(&[], None, &context, true).unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_INPUT"))
        );
        let malformed_base_input = vec![0; 22];
        assert_eq!(
            precompile
                .run(&malformed_base_input, None, &context, true)
                .unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC"))
        );

        let mut short_erc20_input = vec![0; 21];
        short_erc20_input[0] = 1;
        assert_eq!(
            precompile
                .run(&short_erc20_input, None, &context, false)
                .unwrap_err(),
            ExitError::Other(Cow::from("ERR_ETH_ATTACHED_FOR_ERC20_EXIT"))
        );
        context.apparent_value = U256::zero();
        assert_eq!(
            precompile
                .run(&short_erc20_input, None, &context, false)
                .unwrap_err(),
            ExitError::Other(Cow::from("Target token not found"))
        );
        storage.borrow_mut().kv_store.insert(
            bytes_to_key(KeyPrefix::Erc20Nep141Map, context.caller.as_bytes()),
            b"token.near".to_vec(),
        );
        assert_eq!(
            precompile
                .run(&short_erc20_input, None, &context, false)
                .unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_INPUT"))
        );
        short_erc20_input[0] = 2;
        assert_eq!(
            precompile
                .run(&short_erc20_input, None, &context, false)
                .unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_RECEIVER_ACCOUNT_ID"))
        );
    }

    #[test]
    fn test_exit_to_ethereum_erc20_input_size() {
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::storage::{KeyPrefix, bytes_to_key};
        use std::cell::RefCell;

        use crate::{Context, ExitError, Precompile, prelude::Cow};

        let storage = RefCell::new(Storage::default());
        let context = Context {
            address: ADDRESS.raw(),
            caller: aurora_engine_types::types::Address::from_array([1; 20]).raw(),
            apparent_value: U256::zero(),
        };
        storage.borrow_mut().kv_store.insert(
            bytes_to_key(KeyPrefix::Erc20Nep141Map, context.caller.as_bytes()),
            b"token.near".to_vec(),
        );
        let precompile = ExitToEthereum::new(StoragePointer(&storage));

        for input_size in 21..=53 {
            let mut input = vec![0; input_size];
            input[0] = 1;
            let result = precompile.run(&input, None, &context, false);

            match input_size {
                21..=32 => assert_eq!(
                    result.unwrap_err(),
                    ExitError::Other(Cow::from("ERR_INVALID_INPUT"))
                ),
                33..=52 => assert_eq!(
                    result.unwrap_err(),
                    ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS"))
                ),
                _ => assert_eq!(result.unwrap().logs.len(), 2),
            }
        }
    }
}
