use aurora_engine_types::{
    account_id::AccountId,
    borsh,
    parameters::{
        PromiseWithCallbackArgs,
        connector::{ExitToNearPrecompileCallbackArgs, TransferNearArgs},
    },
};
use aurora_evm::backend::Log;
use aurora_evm::{Context, ExitError};

use super::{
    events::{self, ExitToNearLegacy, ExitToNearOmni},
    get_eth_connector_contract_account, get_nep141_from_erc20, parse_amount, to_u128,
    validate_input_size,
};
use crate::prelude::types::{EthGas, NearGas, make_address};
use crate::prelude::{
    Cow, H256, String, ToString, U256, Vec, format,
    parameters::{PromiseArgs, PromiseCreateArgs},
    sdk::io::IO,
    str,
    types::{Address, Yocto},
    vec,
};
#[cfg(feature = "error_refund")]
use crate::prelude::{parameters::connector::RefundCallArgs, types};
use crate::xcc::state::get_wnear_address;
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};

const UNWRAP_WNEAR_MSG: &str = "unwrap";
#[cfg(not(feature = "error_refund"))]
const MIN_INPUT_SIZE: usize = 3;
#[cfg(feature = "error_refund")]
const MIN_INPUT_SIZE: usize = 21;
const MAX_INPUT_SIZE: usize = 1_024;

mod costs {
    use crate::prelude::types::{EthGas, NearGas};

    // TODO(#483): Determine the correct amount of gas
    pub(super) const EXIT_TO_NEAR_GAS: EthGas = EthGas::new(0);

    /// Value determined experimentally based on tests and mainnet data. Example:
    /// `https://explorer.mainnet.near.org/transactions/5CD7NrqWpK3H8MAAU4mYEPuuWz9AqR9uJkkZJzw5b8PM#D1b5NVRrAsJKUX2ZGs3poKViu1Rgt4RJZXtTfMgdxH4S`
    pub(super) const FT_TRANSFER_GAS: NearGas = NearGas::new(10_000_000_000_000);

    pub(super) const FT_TRANSFER_CALL_GAS: NearGas = NearGas::new(70_000_000_000_000);

    /// Value determined experimentally based on tests.
    pub(super) const EXIT_TO_NEAR_CALLBACK_GAS: NearGas = NearGas::new(10_000_000_000_000);
}

/// Exit to NEAR precompile address
///
/// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
/// This address is computed as: `&keccak("exitToNear")[12..]`
pub const ADDRESS: Address = make_address(0xe9217bc7, 0x0b7ed1f598ddd3199e80b093fa71124f);

/// Transfer ETH(base) or ERC-20 tokens to NEAR.
pub struct ExitToNear<I> {
    current_account_id: AccountId,
    io: I,
}

impl<I> ExitToNear<I> {
    pub const fn new(current_account_id: AccountId, io: I) -> Self {
        Self {
            current_account_id,
            io,
        }
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct Recipient<'a> {
    receiver_account_id: AccountId,
    message: Option<Message<'a>>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum Message<'a> {
    UnwrapWnear,
    Omni(&'a str),
}

struct ExitPlan {
    target_account_id: AccountId,
    args: String,
    event: events::ExitToNear,
    method: NearMethod,
    transfer_near_args: Option<TransferNearArgs>,
}

#[derive(Clone, Copy)]
enum NearMethod {
    FtTransfer,
    FtTransferCall,
    NearWithdraw,
}

impl NearMethod {
    const fn name(self) -> &'static str {
        match self {
            Self::FtTransfer => "ft_transfer",
            Self::FtTransferCall => "ft_transfer_call",
            Self::NearWithdraw => "near_withdraw",
        }
    }

    const fn attached_gas(self) -> NearGas {
        match self {
            Self::FtTransferCall => costs::FT_TRANSFER_CALL_GAS,
            Self::FtTransfer | Self::NearWithdraw => costs::FT_TRANSFER_GAS,
        }
    }
}

impl<I: IO> Precompile for ExitToNear<I> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::EXIT_TO_NEAR_GAS)
    }

    #[allow(clippy::too_many_lines)]
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        // ETH (base) transfer input format (up to MAX_INPUT_SIZE bytes):
        //  - flag (1 byte)
        //  - refund_address (20 bytes), present if the feature "error_refund" is enabled
        //  - recipient_account_id (the rest of the input), optionally followed by `:msg`
        // ERC-20 transfer input format (up to MAX_INPUT_SIZE bytes):
        //  - flag (1 byte)
        //  - refund_address (20 bytes), present if the feature "error_refund" is enabled
        //  - amount (32 bytes)
        //  - recipient_account_id (the rest of the input), optionally followed by
        //    the `:unwrap` suffix in a case of wNEAR or by `:msg`
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

        let exit_to_near_params = ExitToNearParams::try_from(input)?;

        let ExitPlan {
            target_account_id,
            args,
            event,
            method,
            transfer_near_args,
        } = match exit_to_near_params {
            // ETH(base) token transfer
            //
            // Input slice format:
            //  recipient_account_id (bytes) - the NEAR recipient account which will receive
            //  NEP-141 (base) tokens, or also can contain another message of JSON in case of
            //  OMNI, or address of receiver in case of transfer tokens to another engine
            //  contract. The `:unwrap` suffix isn't supported for the base token.
            ExitToNearParams::BaseToken(ref exit_params) => {
                let eth_connector_account_id = get_eth_connector_contract_account(&self.io)?;
                exit_base_token_to_near(eth_connector_account_id, context, exit_params)?
            }
            // ERC-20 token transfer
            //
            // This precompile branch is expected to be called from the ERC-20 burn function.
            //
            // Input slice format:
            //  amount (U256 big-endian bytes) - the amount that was burned
            //  recipient_account_id (bytes) - the NEAR recipient account which will receive
            //  NEP-141 tokens, or also can contain the `:unwrap` suffix in case of withdrawing
            //  wNEAR, or another message of JSON in case of OMNI, or address of receiver in case
            //  of transfer tokens to another engine contract.
            ExitToNearParams::Erc20TokenParams(ref exit_params) => {
                exit_erc20_token_to_near(context, exit_params, &self.io)?
            }
        };

        let callback_args = ExitToNearPrecompileCallbackArgs {
            #[cfg(feature = "error_refund")]
            refund: Some(refund_call_args(&exit_to_near_params, context)),
            #[cfg(not(feature = "error_refund"))]
            refund: None,
            transfer_near: transfer_near_args,
        };

        let transfer_promise = PromiseCreateArgs {
            target_account_id,
            method: method.name().to_string(),
            args: args.into_bytes(),
            attached_balance: Yocto::new(1),
            attached_gas: method.attached_gas(),
        };

        let promise = if callback_args == ExitToNearPrecompileCallbackArgs::default() {
            PromiseArgs::Create(transfer_promise)
        } else {
            PromiseArgs::Callback(PromiseWithCallbackArgs {
                base: transfer_promise,
                callback: PromiseCreateArgs {
                    target_account_id: self.current_account_id.clone(),
                    method: "exit_to_near_precompile_callback".to_string(),
                    args: borsh::to_vec(&callback_args).unwrap(),
                    attached_balance: Yocto::new(0),
                    attached_gas: costs::EXIT_TO_NEAR_CALLBACK_GAS,
                },
            })
        };
        let promise_log = Log {
            address: ADDRESS.raw(),
            topics: Vec::new(),
            data: borsh::to_vec(&promise).unwrap(),
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

fn exit_base_token_to_near(
    eth_connector_account_id: AccountId,
    context: &Context,
    exit_params: &BaseTokenParams,
) -> Result<ExitPlan, ExitError> {
    match exit_params.message {
        Some(Message::Omni(msg)) => Ok(ExitPlan {
            target_account_id: eth_connector_account_id,
            args: ft_transfer_call_args(
                &exit_params.receiver_account_id,
                context.apparent_value,
                msg,
            )?,
            event: events::ExitToNear::Omni(ExitToNearOmni {
                sender: Address::new(context.caller),
                erc20_address: events::ETH_ADDRESS,
                dest: exit_params.receiver_account_id.to_string(),
                amount: context.apparent_value,
                msg: msg.to_string(),
            }),
            method: NearMethod::FtTransferCall,
            transfer_near_args: None,
        }),
        None => Ok(ExitPlan {
            target_account_id: eth_connector_account_id,
            // There is no way to inject json, given the encoding of both arguments
            // as decimal and valid account id respectively.
            args: format!(
                r#"{{"receiver_id":"{}","amount":"{}"}}"#,
                exit_params.receiver_account_id,
                to_u128(context.apparent_value)?
            ),
            event: events::ExitToNear::Legacy(ExitToNearLegacy {
                sender: Address::new(context.caller),
                erc20_address: events::ETH_ADDRESS,
                dest: exit_params.receiver_account_id.to_string(),
                amount: context.apparent_value,
            }),
            method: NearMethod::FtTransfer,
            transfer_near_args: None,
        }),
        _ => Err(ExitError::Other(Cow::from("ERR_INVALID_MESSAGE"))),
    }
}

fn exit_erc20_token_to_near<I: IO>(
    context: &Context,
    exit_params: &Erc20TokenParams,
    io: &I,
) -> Result<ExitPlan, ExitError> {
    // In case of withdrawing ERC-20 tokens, the `apparent_value` should be zero. In opposite way
    // the funds will be locked in the address of the precompile without any possibility
    // to withdraw them in the future. So, in case if the `apparent_value` is not zero, the error
    // will be returned to prevent that.
    if context.apparent_value != U256::zero() {
        return Err(ExitError::Other(Cow::from(
            "ERR_ETH_ATTACHED_FOR_ERC20_EXIT",
        )));
    }

    let erc20_address = context.caller; // because ERC-20 contract calls the precompile.
    let nep141_account_id = get_nep141_from_erc20(erc20_address.as_bytes(), io)?;

    match exit_params.message {
        // wNEAR address should be set via the `factory_set_wnear_address` transaction first.
        Some(Message::UnwrapWnear) if erc20_address == get_wnear_address(io).raw() =>
        // The flow is following here:
        // 1. We call `near_withdraw` on wNEAR account id on `aurora` behalf.
        // In such way we unwrap wNEAR to NEAR.
        // 2. After that, we call callback `exit_to_near_precompile_callback` on the `aurora`
        // in which make transfer of unwrapped NEAR to the `target_account_id`.
        {
            Ok(ExitPlan {
                target_account_id: nep141_account_id,
                args: format!(r#"{{"amount":"{}"}}"#, exit_params.amount),
                method: NearMethod::NearWithdraw,
                transfer_near_args: Some(TransferNearArgs {
                    target_account_id: exit_params.receiver_account_id.clone(),
                    amount: to_u128(exit_params.amount)?,
                }),
                event: events::ExitToNear::Legacy(ExitToNearLegacy {
                    sender: Address::new(erc20_address),
                    erc20_address: Address::new(erc20_address),
                    dest: exit_params.receiver_account_id.to_string(),
                    amount: exit_params.amount,
                }),
            })
        }
        // In this flow, we're just forwarding the `msg` to the `ft_transfer_call` transaction.
        Some(Message::Omni(msg)) => Ok(ExitPlan {
            target_account_id: nep141_account_id,
            args: ft_transfer_call_args(&exit_params.receiver_account_id, exit_params.amount, msg)?,
            method: NearMethod::FtTransferCall,
            transfer_near_args: None,
            event: events::ExitToNear::Omni(ExitToNearOmni {
                sender: Address::new(erc20_address),
                erc20_address: Address::new(erc20_address),
                dest: exit_params.receiver_account_id.to_string(),
                amount: exit_params.amount,
                msg: msg.to_string(),
            }),
        }),
        // The legacy flow. Just withdraw the tokens to the NEAR account id.
        // P.S. We use underscore here instead of `None` to handle the case when a user
        // could add the `unwrap` suffix for non wNEAR ERC-20 token by mistake.
        _ => {
            // There is no way to inject json, given the encoding of both arguments
            // as decimal and valid account id respectively.
            Ok(ExitPlan {
                target_account_id: nep141_account_id,
                args: format!(
                    r#"{{"receiver_id":"{}","amount":"{}"}}"#,
                    exit_params.receiver_account_id, exit_params.amount
                ),
                method: NearMethod::FtTransfer,
                transfer_near_args: None,
                event: events::ExitToNear::Legacy(ExitToNearLegacy {
                    sender: Address::new(erc20_address),
                    erc20_address: Address::new(erc20_address),
                    dest: exit_params.receiver_account_id.to_string(),
                    amount: exit_params.amount,
                }),
            })
        }
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum ExitToNearParams<'a> {
    BaseToken(BaseTokenParams<'a>),
    Erc20TokenParams(Erc20TokenParams<'a>),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct BaseTokenParams<'a> {
    #[cfg(feature = "error_refund")]
    refund_address: Address,
    receiver_account_id: AccountId,
    message: Option<Message<'a>>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct Erc20TokenParams<'a> {
    #[cfg(feature = "error_refund")]
    refund_address: Address,
    receiver_account_id: AccountId,
    amount: U256,
    message: Option<Message<'a>>,
}

#[cfg(feature = "error_refund")]
fn refund_call_args(params: &ExitToNearParams, context: &Context) -> RefundCallArgs {
    let (recipient_address, erc20_address, amount) = match params {
        ExitToNearParams::BaseToken(params) => {
            (params.refund_address, None, context.apparent_value)
        }
        ExitToNearParams::Erc20TokenParams(params) => (
            params.refund_address,
            Some(Address::new(context.caller)),
            params.amount,
        ),
    };

    RefundCallArgs {
        recipient_address,
        erc20_address,
        amount: types::u256_to_arr(&amount),
    }
}

impl<'a> TryFrom<&'a [u8]> for ExitToNearParams<'a> {
    type Error = ExitError;

    fn try_from(input: &'a [u8]) -> Result<Self, Self::Error> {
        // The first byte of the input is a flag, selecting the behavior to be triggered:
        // 0x00 -> Eth(base) token withdrawal
        // 0x01 -> ERC-20 token withdrawal
        let (flag, payload) = input
            .split_first()
            .ok_or_else(|| ExitError::Other(Cow::from("ERR_MISSING_FLAG")))?;

        validate_input_size(input, MIN_INPUT_SIZE..=MAX_INPUT_SIZE)?;

        #[cfg(feature = "error_refund")]
        let (refund_address, payload) = {
            let (address, payload) = payload
                .split_at_checked(20)
                .ok_or_else(|| ExitError::Other(Cow::from("ERR_INVALID_INPUT")))?;
            let mut buffer = [0; 20];
            buffer.copy_from_slice(address);
            (Address::from_array(buffer), payload)
        };

        match *flag {
            0x0 => {
                let Recipient {
                    receiver_account_id,
                    message,
                } = parse_recipient(payload)?;

                Ok(ExitToNearParams::BaseToken(BaseTokenParams {
                    #[cfg(feature = "error_refund")]
                    refund_address,
                    receiver_account_id,
                    message,
                }))
            }
            0x1 => {
                let (amount, recipient) = payload
                    .split_at_checked(32)
                    .ok_or_else(|| ExitError::Other(Cow::from("ERR_INVALID_INPUT")))?;
                let amount = parse_amount(amount)?;
                let Recipient {
                    receiver_account_id,
                    message,
                } = parse_recipient(recipient)?;

                Ok(ExitToNearParams::Erc20TokenParams(Erc20TokenParams {
                    #[cfg(feature = "error_refund")]
                    refund_address,
                    receiver_account_id,
                    amount,
                    message,
                }))
            }
            _ => Err(ExitError::Other(Cow::from("ERR_INVALID_FLAG"))),
        }
    }
}

fn parse_recipient(recipient: &[u8]) -> Result<Recipient<'_>, ExitError> {
    let recipient = str::from_utf8(recipient)
        .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_RECEIVER_ACCOUNT_ID")))?;
    let (receiver_account_id, message) = recipient.split_once(':').map_or_else(
        || (recipient, None),
        |(recipient, msg)| {
            if msg == UNWRAP_WNEAR_MSG {
                (recipient, Some(Message::UnwrapWnear))
            } else {
                (recipient, Some(Message::Omni(msg)))
            }
        },
    );

    Ok(Recipient {
        receiver_account_id: receiver_account_id
            .parse()
            .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_RECEIVER_ACCOUNT_ID")))?,
        message,
    })
}

#[derive(serde::Serialize)]
struct FtTransferCallArgs<'a> {
    receiver_id: &'a AccountId,
    amount: String,
    msg: &'a str,
}

fn ft_transfer_call_args(
    receiver_id: &AccountId,
    amount: U256,
    msg: &str,
) -> Result<String, ExitError> {
    serde_json::to_string(&FtTransferCallArgs {
        receiver_id,
        amount: to_u128(amount)?.to_string(),
        msg,
    })
    .map_err(|_| ExitError::Other(Cow::from("ERR_SERIALIZE_JSON")))
}

#[cfg(test)]
mod tests {
    use super::{
        BaseTokenParams, Erc20TokenParams, ExitToNearParams, MAX_INPUT_SIZE, Message, Recipient,
        parse_recipient,
    };
    use crate::{ExitError, prelude::Cow};
    use aurora_engine_types::{U256, account_id::AccountId, types::Address};

    #[test]
    fn test_parse_recipient() {
        assert_eq!(
            parse_recipient(b"test.near").unwrap(),
            Recipient {
                receiver_account_id: "test.near".parse().unwrap(),
                message: None,
            }
        );

        assert_eq!(
            parse_recipient(b"test.near:unwrap").unwrap(),
            Recipient {
                receiver_account_id: "test.near".parse().unwrap(),
                message: Some(Message::UnwrapWnear),
            }
        );

        assert_eq!(
            parse_recipient(
                b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2:unwrap"
            )
            .unwrap(),
            Recipient {
                receiver_account_id:
                    "e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2"
                        .parse()
                        .unwrap(),
                message: Some(Message::UnwrapWnear),
            }
        );

        assert_eq!(
            parse_recipient(b"test.near:some_msg:with_extra_colon").unwrap(),
            Recipient {
                receiver_account_id: "test.near".parse().unwrap(),
                message: Some(Message::Omni("some_msg:with_extra_colon")),
            }
        );

        assert_eq!(
            parse_recipient(b"test.near:").unwrap(),
            Recipient {
                receiver_account_id: "test.near".parse().unwrap(),
                message: Some(Message::Omni("")),
            }
        );
    }

    #[test]
    fn test_parse_invalid_recipient() {
        assert!(parse_recipient(b"test@.near").is_err());
        assert!(parse_recipient(b"test@.near:msg").is_err());
        assert!(parse_recipient(&[0xc2]).is_err());
    }

    #[test]
    fn test_parse_input() {
        #[cfg(feature = "error_refund")]
        let refund_address = Address::zero();
        let amount = U256::from(100);
        let input = [
            &[1],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(),
            b"test.near",
        ]
        .concat();
        assert!(ExitToNearParams::try_from(input.as_slice()).is_ok());

        let input = [
            &[0],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            b"test.near:unwrap".as_slice(),
        ]
        .concat();
        assert!(ExitToNearParams::try_from(input.as_slice()).is_ok());

        let input = [
            &[1],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(),
            b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2:unwrap",
        ]
        .concat();
        assert!(ExitToNearParams::try_from(input.as_slice()).is_ok());

        let input = [
            &[1], // flag
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(), // amount
            b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2:unwrap",
        ]
        .concat();
        assert!(ExitToNearParams::try_from(input.as_slice()).is_ok());
    }

    #[test]
    fn test_parse_input_rejects_short_erc20_amount() {
        let prefix = [
            &[1][..],
            #[cfg(feature = "error_refund")]
            &[0; 20][..],
        ]
        .concat();

        for amount_size in 0..32 {
            let mut input = prefix.clone();
            input.extend_from_slice(&vec![0; amount_size]);
            assert_eq!(
                ExitToNearParams::try_from(input.as_slice()).unwrap_err(),
                ExitError::Other(Cow::from("ERR_INVALID_INPUT"))
            );
        }
    }

    #[test]
    fn test_parse_input_rejects_oversized_erc20_amount() {
        let amount = U256::from(u128::MAX) + 1;
        let input = [
            &[1][..],
            #[cfg(feature = "error_refund")]
            &[0; 20][..],
            amount.to_big_endian().as_slice(),
            b"test.near",
        ]
        .concat();

        assert_eq!(
            ExitToNearParams::try_from(input.as_slice()).unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_AMOUNT"))
        );
    }

    #[test]
    fn test_parse_input_rejects_missing_or_invalid_flag_and_oversized_input() {
        assert_eq!(
            ExitToNearParams::try_from(&[][..]).unwrap_err(),
            ExitError::Other(Cow::from("ERR_MISSING_FLAG"))
        );

        let invalid_flag = [
            &[2][..],
            #[cfg(feature = "error_refund")]
            &[0; 20][..],
            b"test.near",
        ]
        .concat();
        assert_eq!(
            ExitToNearParams::try_from(invalid_flag.as_slice()).unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_FLAG"))
        );

        let oversized = vec![0; MAX_INPUT_SIZE + 1];
        assert_eq!(
            ExitToNearParams::try_from(oversized.as_slice()).unwrap_err(),
            ExitError::Other(Cow::from("ERR_INVALID_INPUT"))
        );
    }

    #[test]
    fn test_parse_exit_to_near_params() {
        let amount = U256::from(100);
        #[cfg(feature = "error_refund")]
        let refund_address = Address::from_array([1; 20]);

        let assert_input = |input: Vec<u8>, expected| {
            let actual = ExitToNearParams::try_from(input.as_slice()).unwrap();
            assert_eq!(actual, expected);
        };

        let input = [
            &[0],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            b"test.near".as_slice(),
        ]
        .concat();
        assert_input(
            input,
            ExitToNearParams::BaseToken(BaseTokenParams {
                #[cfg(feature = "error_refund")]
                refund_address,
                receiver_account_id: "test.near".parse().unwrap(),
                message: None,
            }),
        );

        let input = [
            &[1],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(),
            b"test.near:unwrap",
        ]
        .concat();
        assert_input(
            input,
            ExitToNearParams::Erc20TokenParams(Erc20TokenParams {
                #[cfg(feature = "error_refund")]
                refund_address,
                receiver_account_id: "test.near".parse().unwrap(),
                amount,
                message: Some(Message::UnwrapWnear),
            }),
        );

        let input = [
            &[1],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(),
            b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2:unwrap",
        ]
        .concat();
        assert_input(
            input,
            ExitToNearParams::Erc20TokenParams(Erc20TokenParams {
                #[cfg(feature = "error_refund")]
                refund_address,
                receiver_account_id:
                    "e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2"
                        .parse()
                        .unwrap(),
                amount,
                message: Some(Message::UnwrapWnear),
            }),
        );

        let input = [
            &[1], // flag
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(), // amount
            b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2",
            b":",
            "{\\\"recipient\\\":\\\"eth:013fe02fb1470d0f4ff072f40960658c4ec8139a\\\",\\\"fee\\\":\\\"0\\\",\\\"native_token_fee\\\":\\\"0\\\"}".as_bytes(),
        ]
        .concat();
        assert_input(
            input,
            ExitToNearParams::Erc20TokenParams(Erc20TokenParams {
                #[cfg(feature = "error_refund")]
                refund_address,
                receiver_account_id:
                    "e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2"
                        .parse()
                        .unwrap(),
                amount,
                message: Some(Message::Omni(
                    "{\\\"recipient\\\":\\\"eth:013fe02fb1470d0f4ff072f40960658c4ec8139a\\\",\\\"fee\\\":\\\"0\\\",\\\"native_token_fee\\\":\\\"0\\\"}",
                )),
            }),
        );
    }

    #[test]
    fn test_exit_base_token_to_near_amount_bounds() {
        let connector_account_id: AccountId = "connector.near".parse().unwrap();
        let params = BaseTokenParams {
            #[cfg(feature = "error_refund")]
            refund_address: Address::zero(),
            receiver_account_id: "test.near".parse().unwrap(),
            message: None,
        };
        let mut context = crate::utils::new_context();
        context.apparent_value = U256::from(u128::MAX);

        let plan = super::exit_base_token_to_near(connector_account_id.clone(), &context, &params)
            .unwrap();
        assert_eq!(
            plan.args,
            format!(r#"{{"receiver_id":"test.near","amount":"{}"}}"#, u128::MAX)
        );
        assert_eq!(plan.method.name(), "ft_transfer");
        assert_eq!(plan.method.attached_gas(), super::costs::FT_TRANSFER_GAS);

        context.apparent_value += U256::from(1);
        assert_eq!(
            super::exit_base_token_to_near(connector_account_id, &context, &params).err(),
            Some(ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")))
        );
    }

    #[cfg(feature = "error_refund")]
    #[test]
    fn test_refund_call_args_from_input_and_context() {
        use aurora_engine_types::parameters::connector::RefundCallArgs;

        let refund_address = Address::from_array([1; 20]);
        let caller = Address::from_array([2; 20]);
        let mut context = crate::utils::new_context();
        context.caller = caller.raw();
        context.apparent_value = U256::from(100);

        let base = ExitToNearParams::BaseToken(BaseTokenParams {
            refund_address,
            receiver_account_id: "test.near".parse().unwrap(),
            message: None,
        });
        assert_eq!(
            super::refund_call_args(&base, &context),
            RefundCallArgs {
                recipient_address: refund_address,
                erc20_address: None,
                amount: context.apparent_value.to_big_endian(),
            }
        );

        let token_amount = U256::from(42);
        let erc20 = ExitToNearParams::Erc20TokenParams(Erc20TokenParams {
            refund_address,
            receiver_account_id: "test.near".parse().unwrap(),
            amount: token_amount,
            message: None,
        });
        assert_eq!(
            super::refund_call_args(&erc20, &context),
            RefundCallArgs {
                recipient_address: refund_address,
                erc20_address: Some(caller),
                amount: token_amount.to_big_endian(),
            }
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_exit_to_near_promise_methods_and_gas() {
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::{
            borsh::{self, BorshDeserialize},
            parameters::{PromiseArgs, connector::ExitToNearPrecompileCallbackArgs},
            storage::{EthConnectorStorageId, KeyPrefix, bytes_to_key},
            types::Yocto,
        };
        use std::cell::RefCell;

        use crate::{Precompile, prelude::types::NearGas};

        let connector_account_id: AccountId = "connector.near".parse().unwrap();
        let engine_account_id: AccountId = "engine.near".parse().unwrap();
        let token_account_id: AccountId = "token.near".parse().unwrap();
        let caller = Address::from_array([2; 20]);
        let storage = RefCell::new(Storage::default());
        {
            let mut storage = storage.borrow_mut();
            storage.kv_store.insert(
                super::super::construct_contract_key(EthConnectorStorageId::EthConnectorAccount),
                borsh::to_vec(&connector_account_id).unwrap(),
            );
            storage.kv_store.insert(
                bytes_to_key(KeyPrefix::Erc20Nep141Map, caller.as_bytes()),
                token_account_id.as_bytes().to_vec(),
            );
            storage.kv_store.insert(
                bytes_to_key(KeyPrefix::CrossContractCall, crate::xcc::state::WNEAR_KEY),
                borsh::to_vec(&caller).unwrap(),
            );
        }
        let precompile = super::ExitToNear::new(engine_account_id, StoragePointer(&storage));
        let mut context = crate::utils::new_context();
        context.address = super::ADDRESS.raw();
        context.caller = caller.raw();
        context.apparent_value = U256::from(7);

        let make_input = |flag, payload: &[u8]| {
            let mut input = vec![flag];
            #[cfg(feature = "error_refund")]
            input.extend_from_slice(&[3; 20]);
            input.extend_from_slice(payload);
            input
        };
        let check_promise = |input: &[u8], context: &crate::Context, method: &str, gas: NearGas| {
            let output = precompile.run(input, None, context, false).unwrap();
            assert_eq!(output.logs.len(), 2);
            let promise = PromiseArgs::try_from_slice(&output.logs[0].data).unwrap();
            let call = match &promise {
                PromiseArgs::Create(call) => call,
                PromiseArgs::Callback(callback) => &callback.base,
                PromiseArgs::Recursive(_) => panic!("unexpected recursive promise"),
            };
            assert_eq!(call.method, method);
            assert_eq!(call.attached_gas, gas);
            assert_eq!(call.attached_balance, Yocto::new(1));
            promise
        };

        let _ = check_promise(
            &make_input(0, b"test.near"),
            &context,
            "ft_transfer",
            super::costs::FT_TRANSFER_GAS,
        );
        let _ = check_promise(
            &make_input(0, b"test.near:forward"),
            &context,
            "ft_transfer_call",
            super::costs::FT_TRANSFER_CALL_GAS,
        );

        context.apparent_value = U256::zero();
        let mut erc20_payload = U256::from(u128::MAX).to_big_endian().to_vec();
        erc20_payload.extend_from_slice(b"test.near");
        let promise = check_promise(
            &make_input(1, &erc20_payload),
            &context,
            "ft_transfer",
            super::costs::FT_TRANSFER_GAS,
        );
        let call = match &promise {
            PromiseArgs::Create(call) => call,
            PromiseArgs::Callback(callback) => &callback.base,
            PromiseArgs::Recursive(_) => panic!("unexpected recursive promise"),
        };
        assert_eq!(
            call.args,
            format!(r#"{{"receiver_id":"test.near","amount":"{}"}}"#, u128::MAX).into_bytes()
        );

        erc20_payload.truncate(32);
        erc20_payload.extend_from_slice(b"test.near:forward");
        let _ = check_promise(
            &make_input(1, &erc20_payload),
            &context,
            "ft_transfer_call",
            super::costs::FT_TRANSFER_CALL_GAS,
        );

        erc20_payload.truncate(32);
        erc20_payload.extend_from_slice(b"test.near:unwrap");
        let promise = check_promise(
            &make_input(1, &erc20_payload),
            &context,
            "near_withdraw",
            super::costs::FT_TRANSFER_GAS,
        );
        let PromiseArgs::Callback(callback) = promise else {
            panic!("wNEAR withdrawal must have a callback");
        };
        assert_eq!(
            callback.base.args,
            format!(r#"{{"amount":"{}"}}"#, u128::MAX).into_bytes()
        );
        let callback_args =
            ExitToNearPrecompileCallbackArgs::try_from_slice(&callback.callback.args).unwrap();
        assert_eq!(
            callback_args.transfer_near,
            Some(
                aurora_engine_types::parameters::connector::TransferNearArgs {
                    target_account_id: "test.near".parse().unwrap(),
                    amount: u128::MAX,
                }
            )
        );
    }

    #[test]
    fn test_ft_transfer_call_args() {
        let receiver_id = "test.near".parse().unwrap();
        let amount = U256::from(100);
        let msg = "some message";

        let args = super::ft_transfer_call_args(&receiver_id, amount, msg).unwrap();
        let expected =
            format!(r#"{{"receiver_id":"{receiver_id}","amount":"{amount}","msg":"{msg}"}}"#);
        assert_eq!(args, expected);
    }

    #[test]
    fn test_ft_transfer_call_args_json_injection() {
        let receiver_id = "test.near".parse().unwrap();
        let amount = U256::from(100);
        let msg = "some message\", \"amount\": \"1000"; // attempt to increase amount

        let args = super::ft_transfer_call_args(&receiver_id, amount, msg).unwrap();
        let expected = format!(
            r#"{{"receiver_id":"{receiver_id}","amount":"{amount}","msg":"some message\", \"amount\": \"1000"}}"#
        );
        assert_eq!(args, expected);
    }

    #[test]
    fn test_ft_transfer_call_args_u256() {
        let receiver_id = "test.near".parse().unwrap();
        let amount = U256::from(u128::MAX) + 1;
        let msg = "some message";
        assert_eq!(
            super::ft_transfer_call_args(&receiver_id, amount, msg),
            Err(ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")))
        );
    }
}
