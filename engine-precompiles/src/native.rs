use super::{EvmPrecompileResult, Precompile};
use crate::native::events::{ExitToNearLegacy, ExitToNearOmni};
use crate::prelude::types::EthGas;
use crate::prelude::{
    format,
    parameters::{PromiseArgs, PromiseCreateArgs},
    sdk::io::{StorageIntermediate, IO},
    storage::{bytes_to_key, KeyPrefix},
    str,
    types::{Address, Yocto},
    vec, Cow, String, ToString, Vec, H256, U256,
};
#[cfg(feature = "error_refund")]
use crate::prelude::{parameters::RefundCallArgs, types};
use crate::xcc::state::get_wnear_address;
use crate::PrecompileOutput;
use aurora_engine_types::{
    account_id::AccountId,
    borsh,
    parameters::{
        connector::WithdrawSerializeType, ExitToNearPrecompileCallbackCallArgs,
        PromiseWithCallbackArgs, TransferNearCallArgs, WithdrawCallArgs,
    },
    storage::EthConnectorStorageId,
    types::NEP141Wei,
};
use aurora_evm::backend::Log;
use aurora_evm::{Context, ExitError};

const ERR_TARGET_TOKEN_NOT_FOUND: &str = "Target token not found";
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

    // TODO(#483): Determine the correct amount of gas
    pub(super) const EXIT_TO_ETHEREUM_GAS: EthGas = EthGas::new(0);

    /// Value determined experimentally based on tests and mainnet data. Example:
    /// `https://explorer.mainnet.near.org/transactions/5CD7NrqWpK3H8MAAU4mYEPuuWz9AqR9uJkkZJzw5b8PM#D1b5NVRrAsJKUX2ZGs3poKViu1Rgt4RJZXtTfMgdxH4S`
    pub(super) const FT_TRANSFER_GAS: NearGas = NearGas::new(10_000_000_000_000);

    pub(super) const FT_TRANSFER_CALL_GAS: NearGas = NearGas::new(70_000_000_000_000);

    /// Value determined experimentally based on tests.
    pub(super) const EXIT_TO_NEAR_CALLBACK_GAS: NearGas = NearGas::new(10_000_000_000_000);

    // TODO(#332): Determine the correct amount of gas
    pub(super) const WITHDRAWAL_GAS: NearGas = NearGas::new(100_000_000_000_000);
}

pub mod events {
    use crate::prelude::{types::Address, vec, String, ToString, H256, U256};

    /// Derived from event signature (see `tests::test_exit_signatures`)
    pub const EXIT_TO_NEAR_SIGNATURE: H256 = crate::make_h256(
        0x5a91b8bc9c1981673db8fb226dbd8fcd,
        0xd0c23f45cd28abb31403a5392f6dd0c7,
    );
    /// Derived from event signature (see `tests::test_exit_signatures`)
    pub const EXIT_TO_ETH_SIGNATURE: H256 = crate::make_h256(
        0xd046c2bb01a5622bc4b9696332391d87,
        0x491373762eeac0831c48400e2d5a5f07,
    );

    /// The exit precompile events have an `erc20_address` field to indicate
    /// which ERC-20 token is being withdrawn. However, ETH is not an ERC-20 token
    /// So we need to have some other address to fill this field. This constant is
    /// used for this purpose.
    pub const ETH_ADDRESS: Address = Address::zero();

    /// `ExitToNear`(
    ///    Address indexed sender,
    ///    Address indexed `erc20_address`,
    ///    string indexed dest,
    ///    uint amount
    /// )
    /// Note: in the ERC-20 exit case `sender` == `erc20_address` because it is
    /// the ERC-20 contract which calls the exit precompile. However, in the case
    /// of ETH exit the sender will give the true sender (and the `erc20_address`
    /// will not be meaningful because ETH is not an ERC-20 token).
    pub enum ExitToNear {
        Legacy(ExitToNearLegacy),
        Omni(ExitToNearOmni),
    }

    impl ExitToNear {
        #[must_use]
        pub fn encode(self) -> ethabi::RawLog {
            match self {
                Self::Legacy(legacy) => legacy.encode(),
                Self::Omni(omni) => omni.encode(),
            }
        }
    }

    pub struct ExitToNearLegacy {
        pub sender: Address,
        pub erc20_address: Address,
        pub dest: String,
        pub amount: U256,
    }

    impl ExitToNearLegacy {
        #[must_use]
        pub fn encode(self) -> ethabi::RawLog {
            let data = ethabi::encode(&[ethabi::Token::Uint(self.amount.to_big_endian().into())]);
            let topics = vec![
                EXIT_TO_NEAR_SIGNATURE.0.into(),
                encode_address(self.sender),
                encode_address(self.erc20_address),
                aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(self.dest)]))
                    .0
                    .into(),
            ];

            ethabi::RawLog { topics, data }
        }
    }

    pub struct ExitToNearOmni {
        pub sender: Address,
        pub erc20_address: Address,
        pub dest: String,
        pub amount: U256,
        pub msg: String,
    }

    impl ExitToNearOmni {
        #[must_use]
        pub fn encode(self) -> ethabi::RawLog {
            let data = ethabi::encode(&[
                ethabi::Token::Uint(self.amount.to_big_endian().into()),
                ethabi::Token::String(self.msg),
            ]);
            let topics = vec![
                EXIT_TO_NEAR_SIGNATURE.0.into(),
                encode_address(self.sender),
                encode_address(self.erc20_address),
                aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(self.dest)]))
                    .0
                    .into(),
            ];

            ethabi::RawLog { topics, data }
        }
    }

    #[must_use]
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

    /// `ExitToEth`(
    ///    Address indexed sender,
    ///    Address indexed `erc20_address`,
    ///    string indexed dest,
    ///    uint amount
    /// )
    /// Note: in the ERC-20 exit case `sender` == `erc20_address` because it is
    /// the ERC-20 contract which calls the exit precompile. However, in the case
    /// of ETH exit the sender will give the true sender (and the `erc20_address`
    /// will not be meaningful because ETH is not an ERC-20 token).
    pub struct ExitToEth {
        pub sender: Address,
        pub erc20_address: Address,
        pub dest: Address,
        pub amount: U256,
    }

    impl ExitToEth {
        #[must_use]
        pub fn encode(self) -> ethabi::RawLog {
            let data = ethabi::encode(&[ethabi::Token::Uint(self.amount.to_big_endian().into())]);
            let topics = vec![
                EXIT_TO_ETH_SIGNATURE.0.into(),
                encode_address(self.sender),
                encode_address(self.erc20_address),
                encode_address(self.dest),
            ];

            ethabi::RawLog { topics, data }
        }
    }

    #[must_use]
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

    fn encode_address(a: Address) -> ethabi::Hash {
        let mut result = [0u8; 32];
        result[12..].copy_from_slice(a.as_bytes());
        result.into()
    }
}

trait EthConnector {
    fn get_eth_connector_contract_account(&self) -> Result<AccountId, ExitError>;
}

/// Transfer ETH(base) or ERC-20 tokens to NEAR.
pub struct ExitToNear<I> {
    current_account_id: AccountId,
    io: I,
}

pub mod exit_to_near {
    use crate::prelude::types::{make_address, Address};

    /// Exit to NEAR precompile address
    ///
    /// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
    /// This address is computed as: `&keccak("exitToNear")[12..]`
    pub const ADDRESS: Address = make_address(0xe9217bc7, 0x0b7ed1f598ddd3199e80b093fa71124f);
}

impl<I> ExitToNear<I> {
    pub const fn new(current_account_id: AccountId, io: I) -> Self {
        Self {
            current_account_id,
            io,
        }
    }
}

impl<I: IO> EthConnector for ExitToNear<I> {
    fn get_eth_connector_contract_account(&self) -> Result<AccountId, ExitError> {
        #[cfg(not(feature = "ext-connector"))]
        let eth_connector_account_id = self.current_account_id.clone();
        #[cfg(feature = "ext-connector")]
        let eth_connector_account_id = get_eth_connector_contract_account(&self.io)?;

        Ok(eth_connector_account_id)
    }
}

fn validate_input_size(input: &[u8], min: usize, max: usize) -> Result<(), ExitError> {
    if input.len() < min || input.len() > max {
        return Err(ExitError::Other(Cow::from("ERR_INVALID_INPUT")));
    }
    Ok(())
}

fn get_nep141_from_erc20<I: IO>(erc20_token: &[u8], io: &I) -> Result<AccountId, ExitError> {
    AccountId::try_from(
        io.read_storage(bytes_to_key(KeyPrefix::Erc20Nep141Map, erc20_token).as_slice())
            .map(|s| s.to_vec())
            .ok_or(ExitError::Other(Cow::Borrowed(ERR_TARGET_TOKEN_NOT_FOUND)))?,
    )
    .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_INVALID_NEP141_ACCOUNT")))
}

#[cfg(feature = "ext-connector")]
fn get_eth_connector_contract_account<I: IO>(io: &I) -> Result<AccountId, ExitError> {
    io.read_storage(&construct_contract_key(
        EthConnectorStorageId::EthConnectorAccount,
    ))
    .ok_or(ExitError::Other(Cow::Borrowed("ERR_KEY_NOT_FOUND")))
    .and_then(|x| {
        x.to_value()
            .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_DESERIALIZE")))
    })
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

fn construct_contract_key(suffix: EthConnectorStorageId) -> Vec<u8> {
    bytes_to_key(KeyPrefix::EthConnector, &[u8::from(suffix)])
}

fn parse_amount(input: &[u8]) -> Result<U256, ExitError> {
    let amount = U256::from_big_endian(input);

    if amount > U256::from(u128::MAX) {
        return Err(ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")));
    }

    Ok(amount)
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
        // ETH (base) transfer input format: (85 bytes)
        //  - flag (1 byte)
        //  - refund_address (20 bytes), present if the feature "error_refund" is enabled
        //  - recipient_account_id (max MAX_INPUT_SIZE - 20 - 1 bytes)
        // ERC-20 transfer input format: (124 bytes)
        //  - flag (1 byte)
        //  - refund_address (20 bytes), present if the feature "error_refund" is enabled.
        //  - amount (32 bytes)
        //  - recipient_account_id (max MAX_INPUT_SIZE - 1 - (20) - 32 bytes)
        //  - `:unwrap` suffix in a case of wNEAR (7 bytes)
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

        let exit_to_near_params = ExitToNearParams::try_from(input)?;

        let (nep141_address, args, exit_event, method, transfer_near_args) =
            match exit_to_near_params {
                // ETH(base) token transfer
                //
                // Input slice format:
                //  recipient_account_id (bytes) - the NEAR recipient account which will receive
                //  NEP-141 (base) tokens, or also can contain the `:unwrap` suffix in case of
                //  withdrawing wNEAR, or other message of JSON in case of OMNI, or address of
                //  receiver in case of transfer tokens to another engine contract.
                ExitToNearParams::BaseToken(ref exit_params) => {
                    let eth_connector_account_id = self.get_eth_connector_contract_account()?;
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
                //  wNEAR, or other message of JSON in case of OMNI, or address of receiver in case
                //  of transfer tokens to another engine contract.
                ExitToNearParams::Erc20TokenParams(ref exit_params) => {
                    exit_erc20_token_to_near(context, exit_params, &self.io)?
                }
            };

        let callback_args = ExitToNearPrecompileCallbackCallArgs {
            #[cfg(feature = "error_refund")]
            refund: refund_call_args(&exit_to_near_params, &exit_event),
            #[cfg(not(feature = "error_refund"))]
            refund: None,
            transfer_near: transfer_near_args,
        };
        let attached_gas = if method == "ft_transfer_call" {
            costs::FT_TRANSFER_CALL_GAS
        } else {
            costs::FT_TRANSFER_GAS
        };

        let transfer_promise = PromiseCreateArgs {
            target_account_id: nep141_address,
            method,
            args: args.into_bytes(),
            attached_balance: Yocto::new(1),
            attached_gas,
        };

        let promise = if callback_args == ExitToNearPrecompileCallbackCallArgs::default() {
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
            address: exit_to_near::ADDRESS.raw(),
            topics: Vec::new(),
            data: borsh::to_vec(&promise).unwrap(),
        };
        let ethabi::RawLog { topics, data } = exit_event.encode();
        let exit_event_log = Log {
            address: exit_to_near::ADDRESS.raw(),
            topics: topics.into_iter().map(|h| H256::from(h.0)).collect(),
            data,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log, exit_event_log],
            cost: Self::required_gas(input)?,
            output: Vec::new(),
        })
    }
}

fn exit_base_token_to_near(
    eth_connector_account_id: AccountId,
    context: &Context,
    exit_params: &BaseTokenParams,
) -> Result<
    (
        AccountId,
        String,
        events::ExitToNear,
        String,
        Option<TransferNearCallArgs>,
    ),
    ExitError,
> {
    match exit_params.message {
        Some(Message::Omni(msg)) => Ok((
            eth_connector_account_id,
            ft_transfer_call_args(
                &exit_params.receiver_account_id,
                context.apparent_value,
                msg,
            )?,
            events::ExitToNear::Omni(ExitToNearOmni {
                sender: Address::new(context.caller),
                erc20_address: events::ETH_ADDRESS,
                dest: exit_params.receiver_account_id.to_string(),
                amount: context.apparent_value,
                msg: msg.to_string(),
            }),
            "ft_transfer_call".to_string(),
            None,
        )),
        None => Ok((
            eth_connector_account_id,
            // There is no way to inject json, given the encoding of both arguments
            // as decimal and valid account id respectively.
            format!(
                r#"{{"receiver_id":"{}","amount":"{}"}}"#,
                exit_params.receiver_account_id,
                context.apparent_value.as_u128()
            ),
            events::ExitToNear::Legacy(ExitToNearLegacy {
                sender: Address::new(context.caller),
                erc20_address: events::ETH_ADDRESS,
                dest: exit_params.receiver_account_id.to_string(),
                amount: context.apparent_value,
            }),
            "ft_transfer".to_string(),
            None,
        )),
        _ => Err(ExitError::Other(Cow::from("ERR_INVALID_MESSAGE"))),
    }
}

fn exit_erc20_token_to_near<I: IO>(
    context: &Context,
    exit_params: &Erc20TokenParams,
    io: &I,
) -> Result<
    (
        AccountId,
        String,
        events::ExitToNear,
        String,
        Option<TransferNearCallArgs>,
    ),
    ExitError,
> {
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

    let (nep141_account_id, args, method, transfer_near_args, event) = match exit_params.message {
        Some(Message::UnwrapWnear) => {
            // The flow is following here:
            // 1. We call `near_withdraw` on wNEAR account id on `aurora` behalf.
            // In such way we unwrap wNEAR to NEAR.
            // 2. After that, we call callback `exit_to_near_precompile_callback` on the `aurora`
            // in which make transfer of unwrapped NEAR to the `target_account_id`.
            if erc20_address == get_wnear_address(io).raw() {
                // wNEAR address should be set via the `factory_set_wnear_address` transaction first.
                (
                    nep141_account_id,
                    format!(r#"{{"amount":"{}"}}"#, exit_params.amount.as_u128()),
                    "near_withdraw",
                    Some(TransferNearCallArgs {
                        target_account_id: exit_params.receiver_account_id.clone(),
                        amount: exit_params.amount.as_u128(),
                    }),
                    events::ExitToNear::Legacy(ExitToNearLegacy {
                        sender: Address::new(erc20_address),
                        erc20_address: Address::new(erc20_address),
                        dest: exit_params.receiver_account_id.to_string(),
                        amount: exit_params.amount,
                    }),
                )
            } else {
                return Err(ExitError::Other(Cow::from("ERR_INVALID_ERC20_FOR_UNWRAP")));
            }
        }
        // In this flow, we're just forwarding the `msg` to the `ft_transfer_call` transaction.
        Some(Message::Omni(msg)) => (
            nep141_account_id,
            ft_transfer_call_args(&exit_params.receiver_account_id, exit_params.amount, msg)?,
            "ft_transfer_call",
            None,
            events::ExitToNear::Omni(ExitToNearOmni {
                sender: Address::new(erc20_address),
                erc20_address: Address::new(erc20_address),
                dest: exit_params.receiver_account_id.to_string(),
                amount: exit_params.amount,
                msg: msg.to_string(),
            }),
        ),
        // The legacy flow. Just withdraw the tokens to the NEAR account id.
        None => {
            // There is no way to inject json, given the encoding of both arguments
            // as decimal and valid account id respectively.
            (
                nep141_account_id,
                format!(
                    r#"{{"receiver_id":"{}","amount":"{}"}}"#,
                    exit_params.receiver_account_id,
                    exit_params.amount.as_u128()
                ),
                "ft_transfer",
                None,
                events::ExitToNear::Legacy(ExitToNearLegacy {
                    sender: Address::new(erc20_address),
                    erc20_address: Address::new(erc20_address),
                    dest: exit_params.receiver_account_id.to_string(),
                    amount: exit_params.amount,
                }),
            )
        }
    };

    Ok((
        nep141_account_id,
        args,
        event,
        method.to_string(),
        transfer_near_args,
    ))
}

#[allow(clippy::unnecessary_wraps)]
fn json_args(address: Address, amount: U256) -> Result<Vec<u8>, ExitError> {
    Ok(format!(
        r#"{{"amount":"{}","recipient":"{}"}}"#,
        amount.as_u128(),
        address.encode(),
    )
    .into_bytes())
}

fn borsh_args(address: Address, amount: U256) -> Result<Vec<u8>, ExitError> {
    borsh::to_vec(&WithdrawCallArgs {
        recipient_address: address,
        amount: NEP141Wei::new(amount.as_u128()),
    })
    .map_err(|_| ExitError::Other(Cow::from("ERR_BORSH_SERIALIZE")))
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
#[allow(clippy::unnecessary_wraps)]
fn refund_call_args(
    params: &ExitToNearParams,
    event: &events::ExitToNear,
) -> Option<RefundCallArgs> {
    Some(RefundCallArgs {
        recipient_address: match params {
            ExitToNearParams::BaseToken(params) => params.refund_address,
            ExitToNearParams::Erc20TokenParams(params) => params.refund_address,
        },
        erc20_address: match params {
            ExitToNearParams::BaseToken(_) => None,
            ExitToNearParams::Erc20TokenParams(_) => {
                let erc20_address = match event {
                    events::ExitToNear::Legacy(ref legacy) => legacy.erc20_address,
                    events::ExitToNear::Omni(ref omni) => omni.erc20_address,
                };
                Some(erc20_address)
            }
        },
        amount: types::u256_to_arr(&match event {
            events::ExitToNear::Legacy(ref legacy) => legacy.amount,
            events::ExitToNear::Omni(ref omni) => omni.amount,
        }),
    })
}

impl<'a> TryFrom<&'a [u8]> for ExitToNearParams<'a> {
    type Error = ExitError;

    fn try_from(input: &'a [u8]) -> Result<Self, Self::Error> {
        // First byte of the input is a flag, selecting the behavior to be triggered:
        // 0x00 -> Eth(base) token withdrawal
        // 0x01 -> ERC-20 token withdrawal
        let flag = input
            .first()
            .copied()
            .ok_or_else(|| ExitError::Other(Cow::from("ERR_MISSING_FLAG")))?;

        #[cfg(feature = "error_refund")]
        let (refund_address, input) = parse_input(input)?;
        #[cfg(not(feature = "error_refund"))]
        let input = parse_input(input)?;

        match flag {
            0x0 => {
                let Recipient {
                    receiver_account_id,
                    message,
                } = parse_recipient(input)?;

                Ok(Self::BaseToken(BaseTokenParams {
                    #[cfg(feature = "error_refund")]
                    refund_address,
                    receiver_account_id,
                    message,
                }))
            }
            0x1 => {
                let amount = parse_amount(&input[..32])?;
                let Recipient {
                    receiver_account_id,
                    message,
                } = parse_recipient(&input[32..])?;

                Ok(Self::Erc20TokenParams(Erc20TokenParams {
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

#[cfg(feature = "error_refund")]
fn parse_input(input: &[u8]) -> Result<(Address, &[u8]), ExitError> {
    validate_input_size(input, MIN_INPUT_SIZE, MAX_INPUT_SIZE)?;
    let mut buffer = [0; 20];
    buffer.copy_from_slice(&input[1..21]);
    let refund_address = Address::from_array(buffer);
    Ok((refund_address, &input[21..]))
}

#[cfg(not(feature = "error_refund"))]
fn parse_input(input: &[u8]) -> Result<&[u8], ExitError> {
    validate_input_size(input, MIN_INPUT_SIZE, MAX_INPUT_SIZE)?;
    Ok(&input[1..])
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
    if amount > U256::from(u128::MAX) {
        return Err(ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")));
    }

    serde_json::to_string(&FtTransferCallArgs {
        receiver_id,
        amount: format!("{amount}"),
        msg,
    })
    .map_err(|_| ExitError::Other(Cow::from("ERR_SERIALIZE_JSON")))
}

pub struct ExitToEthereum<I> {
    io: I,
    #[cfg(not(feature = "ext-connector"))]
    current_account_id: AccountId,
}

pub mod exit_to_ethereum {
    use crate::prelude::types::{make_address, Address};

    /// Exit to Ethereum precompile address
    ///
    /// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
    /// This address is computed as: `&keccak("exitToEthereum")[12..]`
    pub const ADDRESS: Address = make_address(0xb0bd02f6, 0xa392af548bdf1cfaee5dfa0eefcc8eab);
}

impl<I> ExitToEthereum<I> {
    #[allow(clippy::missing_const_for_fn, clippy::needless_pass_by_value)]
    pub fn new(current_account_id: AccountId, io: I) -> Self {
        #[cfg(not(feature = "ext-connector"))]
        {
            Self {
                io,
                current_account_id,
            }
        }

        #[cfg(feature = "ext-connector")]
        {
            let _ = current_account_id;
            Self { io }
        }
    }
}

impl<I: IO> EthConnector for ExitToEthereum<I> {
    fn get_eth_connector_contract_account(&self) -> Result<AccountId, ExitError> {
        #[cfg(not(feature = "ext-connector"))]
        let eth_connector_account_id = self.current_account_id.clone();
        #[cfg(feature = "ext-connector")]
        let eth_connector_account_id = get_eth_connector_contract_account(&self.io)?;

        Ok(eth_connector_account_id)
    }
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
        validate_input_size(input, 21, 53)?;
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
        //  0x00 -> ETH (Base token) token transfer
        //  0x01 -> ERC-20 transfer
        let mut input = input;
        let flag = input[0];
        input = &input[1..];

        let (nep141_address, serialized_args, exit_event) = match flag {
            0x0 => {
                // ETH (base) transfer
                //
                // Input slice format:
                //  eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum
                let recipient_address: Address = input
                    .try_into()
                    .map_err(|_| ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS")))?;
                let serialize_fn = match get_withdraw_serialize_type(&self.io)? {
                    WithdrawSerializeType::Json => json_args,
                    WithdrawSerializeType::Borsh => borsh_args,
                };
                let eth_connector_account_id = self.get_eth_connector_contract_account()?;

                (
                    eth_connector_account_id,
                    // There is no way to inject json, given the encoding of both arguments
                    // as decimal and hexadecimal respectively.
                    serialize_fn(recipient_address, context.apparent_value)?,
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
                //  amount (U256 big-endian bytes) - the amount that was burned
                //  eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

                if context.apparent_value != U256::from(0) {
                    return Err(ExitError::Other(Cow::from(
                        "ERR_ETH_ATTACHED_FOR_ERC20_EXIT",
                    )));
                }

                let erc20_address = context.caller;
                let nep141_address = get_nep141_from_erc20(erc20_address.as_bytes(), &self.io)?;
                let amount = parse_amount(&input[..32])?;

                input = &input[32..];

                if input.len() == 20 {
                    // Parse ethereum address in hex
                    let mut buffer = [0; 40];
                    hex::encode_to_slice(input, &mut buffer).unwrap();
                    let recipient_in_hex = str::from_utf8(&buffer).map_err(|_| {
                        ExitError::Other(Cow::from("ERR_INVALID_RECIPIENT_ADDRESS"))
                    })?;
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
                            recipient_in_hex
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

        let promise = borsh::to_vec(&PromiseArgs::Create(withdraw_promise)).unwrap();
        let promise_log = Log {
            address: exit_to_ethereum::ADDRESS.raw(),
            topics: Vec::new(),
            data: promise,
        };
        let ethabi::RawLog { topics, data } = exit_event.encode();
        let exit_event_log = Log {
            address: exit_to_ethereum::ADDRESS.raw(),
            topics: topics.into_iter().map(|h| H256::from(h.0)).collect(),
            data,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log, exit_event_log],
            cost: Self::required_gas(input)?,
            output: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        exit_to_ethereum, exit_to_near, parse_amount, parse_input, parse_recipient,
        validate_input_size, BaseTokenParams, Erc20TokenParams, ExitToNearParams, Message,
    };
    use crate::{native::Recipient, prelude::sdk::types::near_account_to_evm_address};
    #[cfg(feature = "error_refund")]
    use aurora_engine_types::types::Address;
    use aurora_engine_types::U256;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            exit_to_ethereum::ADDRESS,
            near_account_to_evm_address(b"exitToEthereum")
        );
        assert_eq!(
            exit_to_near::ADDRESS,
            near_account_to_evm_address(b"exitToNear")
        );
    }

    #[test]
    fn test_exit_signatures() {
        let exit_to_near = super::events::exit_to_near_schema();
        let exit_to_eth = super::events::exit_to_eth_schema();

        assert_eq!(
            exit_to_near.signature().0,
            super::events::EXIT_TO_NEAR_SIGNATURE.0
        );
        assert_eq!(
            exit_to_eth.signature().0,
            super::events::EXIT_TO_ETH_SIGNATURE.0
        );
    }

    #[test]
    fn test_check_invalid_input_lt_min() {
        let input = [0u8; 4];
        assert!(validate_input_size(&input, 10, 20).is_err());
        assert!(validate_input_size(&input, 5, 0).is_err());
    }

    #[test]
    fn test_check_invalid_max_value_for_input() {
        let input = [0u8; 4];
        assert!(validate_input_size(&input, 5, 0).is_err());
    }

    #[test]
    fn test_check_invalid_input_gt_max() {
        let input = [1u8; 55];
        assert!(validate_input_size(&input, 10, 54).is_err());
    }

    #[test]
    fn test_check_valid_input() {
        let input = [1u8; 55];
        validate_input_size(&input, 10, input.len()).unwrap();
        validate_input_size(&input, 0, input.len()).unwrap();
    }

    #[test]
    #[should_panic(expected = "ERR_INVALID_AMOUNT")]
    fn test_exit_with_invalid_amount() {
        let input = (U256::from(u128::MAX) + 1).to_big_endian();
        parse_amount(input.as_slice()).unwrap();
    }

    #[test]
    fn test_exit_with_valid_amount() {
        let input = U256::from(u128::MAX).to_big_endian();
        assert_eq!(
            parse_amount(input.as_slice()).unwrap(),
            U256::from(u128::MAX)
        );
    }

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
        assert!(parse_input(&input).is_ok());

        let input = [
            &[0],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            b"test.near:unwrap".as_slice(),
        ]
        .concat();
        assert!(parse_input(&input).is_ok());

        let input = [
            &[1],
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(),
            b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2:unwrap",
        ]
        .concat();
        assert!(parse_input(&input).is_ok());

        let input = [
            &[1], // flag
            #[cfg(feature = "error_refund")]
            refund_address.as_bytes(),
            amount.to_big_endian().as_slice(), // amount
            b"e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2:unwrap",
        ]
        .concat();
        assert!(parse_input(&input).is_ok());
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
        assert_input(input,
            ExitToNearParams::Erc20TokenParams(Erc20TokenParams {
                #[cfg(feature = "error_refund")]
                refund_address,
                receiver_account_id:
                    "e523efec9b66c4c8f6e708f1cf56be1399181e5b7c1e35f845670429faf143c2"
                        .parse()
                        .unwrap(),
                amount,
                message: Some(Message::Omni("{\\\"recipient\\\":\\\"eth:013fe02fb1470d0f4ff072f40960658c4ec8139a\\\",\\\"fee\\\":\\\"0\\\",\\\"native_token_fee\\\":\\\"0\\\"}")),
            })
        );
    }

    #[test]
    fn test_ft_transfer_call_args() {
        let receiver_id = "test.near".parse().unwrap();
        let amount = U256::from(100);
        let msg = "some message";

        let args = super::ft_transfer_call_args(&receiver_id, amount, msg).unwrap();
        let expected =
            format!(r#"{{"receiver_id":"{receiver_id}","amount":"{amount}","msg":"{msg}"}}"#,);
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
    #[should_panic(expected = "ERR_INVALID_AMOUNT")]
    fn test_ft_transfer_call_args_u256() {
        let receiver_id = "test.near".parse().unwrap();
        let amount = U256::from(u128::MAX) + 1;
        let msg = "some message";
        let _ = super::ft_transfer_call_args(&receiver_id, amount, msg).unwrap();
    }
}
