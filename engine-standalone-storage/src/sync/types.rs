use aurora_engine::parameters::{CallArgs, DeployErc20TokenArgs};
use aurora_engine_sdk::env::DEFAULT_PREPAID_GAS;
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::{self, BorshDeserialize, BorshSerialize},
    parameters::{connector::FtOnTransferArgs, engine},
    types::{Address, NearGas, PromiseResult},
    H256,
};
use std::borrow::Cow;

/// Type describing the format of messages sent to the storage layer for keeping
/// it in sync with the blockchain.
#[derive(Debug, Clone)]
pub enum Message {
    Block(BlockMessage),
    Transaction(Box<TransactionMessage>),
}

#[derive(Debug, Clone)]
pub struct BlockMessage {
    pub height: u64,
    pub hash: H256,
    pub metadata: crate::BlockMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionMessage {
    /// Hash of the block which included this transaction
    pub block_hash: H256,
    /// Receipt ID of the receipt that was actually executed on NEAR
    pub near_receipt_id: H256,
    /// If multiple Aurora transactions are included in the same block,
    /// this index gives the order in which they should be executed.
    pub position: u16,
    /// True if the transaction executed successfully on the blockchain, false otherwise.
    pub succeeded: bool,
    /// NEAR account that signed the transaction
    pub signer: AccountId,
    /// NEAR account that called the Aurora engine contract
    pub caller: AccountId,
    /// Amount of NEAR token attached to the transaction
    pub attached_near: u128,
    /// Details of the transaction that was executed
    pub transaction: TransactionKind,
    /// Results from previous NEAR receipts
    /// (only present when this transaction is a callback of another transaction).
    pub promise_data: Vec<Option<Vec<u8>>>,
    /// A Near protocol quantity equal to
    /// `sha256(receipt_id || block_hash || le_bytes(u64 - action_index))`.
    /// This quantity is used together with the block random seed
    /// to generate the random value available to the transaction.
    /// nearcore references:
    /// - <https://github.com/near/nearcore/blob/00ca2f3f73e2a547ba881f76ecc59450dbbef6e2/core/primitives/src/utils.rs#L261>
    /// - <https://github.com/near/nearcore/blob/00ca2f3f73e2a547ba881f76ecc59450dbbef6e2/core/primitives/src/utils.rs#L295>
    pub action_hash: H256,
    pub prepaid_gas: NearGas,
}

impl TransactionMessage {
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let borshable: BorshableTransactionMessage = self.into();
        borsh::to_vec(&borshable).expect("self to be valid")
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, std::io::Error> {
        let borshable = match BorshableTransactionMessage::try_from_slice(bytes) {
            Ok(b) => b,
            // To avoid DB migration, allow fallback on deserializing V1 messages
            Err(_) => BorshableTransactionMessageV1::try_from_slice(bytes)
                .map(BorshableTransactionMessage::V1)?,
        };
        Ok(Self::from(borshable))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[allow(clippy::large_enum_variant)]
#[borsh(crate = "aurora_engine_types::borsh")]
pub struct TransactionKind {
    pub(super) method_name: String,
    pub(super) args: Vec<u8>,
    promise_result: Option<PromiseResult>,
}

impl TransactionKind {
    /// Try to parse an Aurora transaction from raw information available in a Near action
    /// (method name, input bytes, data returned from promises).
    pub fn new(method_name: &str, bytes: Vec<u8>, promise_data: &[Option<Vec<u8>>]) -> Self {
        Self {
            method_name: method_name.to_owned(),
            args: bytes,
            promise_result: match method_name {
                "ft_resolve_transfer" => {
                    Some(promise_data.first().and_then(Option::as_ref).map_or(
                        aurora_engine_types::types::PromiseResult::Failed,
                        |bytes| {
                            aurora_engine_types::types::PromiseResult::Successful(bytes.clone())
                        },
                    ))
                }
                _ => None,
            },
        }
    }

    #[must_use]
    pub fn clone_raw_input(&self) -> Vec<u8> {
        self.args.clone()
    }

    #[must_use]
    pub fn submit(tx: &EthTransactionKind) -> Self {
        Self {
            method_name: "submit".to_owned(),
            args: tx.into(),
            promise_result: None,
        }
    }

    #[must_use]
    pub fn deploy_erc20(args: &DeployErc20TokenArgs) -> Self {
        Self {
            method_name: "deploy_erc20_token".to_owned(),
            args: borsh::to_vec(args).unwrap(),
            promise_result: None,
        }
    }

    #[must_use]
    pub fn new_deploy(args: Vec<u8>) -> Self {
        Self {
            method_name: "deploy_code".to_owned(),
            args,
            promise_result: None,
        }
    }

    #[must_use]
    pub fn new_ft_on_transfer(args: &FtOnTransferArgs) -> Self {
        Self {
            method_name: "ft_on_transfer".to_owned(),
            args: serde_json::to_vec(args).unwrap(),
            promise_result: None,
        }
    }

    #[must_use]
    pub fn new_call(args: &CallArgs) -> Self {
        Self {
            method_name: "call".to_owned(),
            args: borsh::to_vec(&args).unwrap(),
            promise_result: None,
        }
    }

    #[must_use]
    pub fn new_factory_update(args: Vec<u8>) -> Self {
        Self {
            method_name: "factory_update".to_owned(),
            args,
            promise_result: None,
        }
    }

    #[must_use]
    pub fn new_factory_set_wnear_address(args: Address) -> Self {
        Self {
            method_name: "factory_set_wnear_address".to_owned(),
            args: args.as_bytes().to_vec(),
            promise_result: None,
        }
    }

    #[must_use]
    pub fn unknown() -> Self {
        Self {
            method_name: "unknown".to_owned(),
            args: vec![],
            promise_result: None,
        }
    }

    #[must_use]
    pub fn get_submit_args(&self) -> Option<engine::SubmitArgs> {
        if self.method_name == "submit_with_args" {
            engine::SubmitArgs::try_from_slice(&self.args).ok()
        } else {
            None
        }
    }
}

impl TransactionKind {
    #[must_use]
    pub fn raw_bytes(&self) -> Vec<u8> {
        self.args.clone()
    }
}

/// This data type represents `TransactionMessage` above in the way consistent with how it is
/// stored on disk (in the DB). This type implements borsh (de)serialization. The purpose of
/// having a private struct for borsh, which is separate from the main `TransactionMessage`
/// which is used in the actual logic of executing transactions,
/// is to decouple the on-disk representation of the data from how it is used in the code.
/// This allows us to keep the `TransactionMessage` structure clean (no need to worry about
/// backwards compatibility with storage), hiding the complexity which is not important to
/// the logic of processing transactions.
///
/// V1 is an older version of `TransactionMessage`, before the addition of `promise_data`.
///
/// V2 is a structurally identical message to `TransactionMessage` above.
///
/// For details of what the individual fields mean, see the comments on the main
/// `TransactionMessage` type.
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
enum BorshableTransactionMessage<'a> {
    V1(BorshableTransactionMessageV1<'a>),
    V2(BorshableTransactionMessageV2<'a>),
    V3(BorshableTransactionMessageV3<'a>),
    V4(BorshableTransactionMessageV4<'a>),
}

#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
struct BorshableTransactionMessageV1<'a> {
    pub block_hash: [u8; 32],
    pub near_receipt_id: [u8; 32],
    pub position: u16,
    pub succeeded: bool,
    pub signer: Cow<'a, AccountId>,
    pub caller: Cow<'a, AccountId>,
    pub attached_near: u128,
    pub transaction: BorshableTransactionKind<'a>,
}

#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
struct BorshableTransactionMessageV2<'a> {
    pub block_hash: [u8; 32],
    pub near_receipt_id: [u8; 32],
    pub position: u16,
    pub succeeded: bool,
    pub signer: Cow<'a, AccountId>,
    pub caller: Cow<'a, AccountId>,
    pub attached_near: u128,
    pub transaction: BorshableTransactionKind<'a>,
    pub promise_data: Cow<'a, Vec<Option<Vec<u8>>>>,
}

#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
struct BorshableTransactionMessageV3<'a> {
    pub block_hash: [u8; 32],
    pub near_receipt_id: [u8; 32],
    pub position: u16,
    pub succeeded: bool,
    pub signer: Cow<'a, AccountId>,
    pub caller: Cow<'a, AccountId>,
    pub attached_near: u128,
    pub transaction: BorshableTransactionKind<'a>,
    pub promise_data: Cow<'a, Vec<Option<Vec<u8>>>>,
}

#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
struct BorshableTransactionMessageV4<'a> {
    pub block_hash: [u8; 32],
    pub near_receipt_id: [u8; 32],
    pub position: u16,
    pub succeeded: bool,
    pub signer: Cow<'a, AccountId>,
    pub caller: Cow<'a, AccountId>,
    pub attached_near: u128,
    pub transaction: BorshableTransactionKind<'a>,
    pub promise_data: Cow<'a, Vec<Option<Vec<u8>>>>,
    pub raw_input: Cow<'a, Vec<u8>>,
    pub action_hash: [u8; 32],
}

impl<'a> From<&'a TransactionMessage> for BorshableTransactionMessage<'a> {
    fn from(t: &'a TransactionMessage) -> Self {
        Self::V3(BorshableTransactionMessageV3 {
            block_hash: t.block_hash.0,
            near_receipt_id: t.near_receipt_id.0,
            position: t.position,
            succeeded: t.succeeded,
            signer: Cow::Borrowed(&t.signer),
            caller: Cow::Borrowed(&t.caller),
            attached_near: t.attached_near,
            transaction: (&t.transaction).into(),
            promise_data: Cow::Borrowed(&t.promise_data),
        })
    }
}

impl<'a> From<BorshableTransactionMessage<'a>> for TransactionMessage {
    fn from(t: BorshableTransactionMessage<'a>) -> Self {
        match t {
            BorshableTransactionMessage::V1(t) => {
                let transaction: TransactionKind = t.transaction.into();
                Self {
                    block_hash: H256(t.block_hash),
                    near_receipt_id: H256(t.near_receipt_id),
                    position: t.position,
                    succeeded: t.succeeded,
                    signer: t.signer.into_owned(),
                    caller: t.caller.into_owned(),
                    attached_near: t.attached_near,
                    transaction,
                    promise_data: Vec::new(),
                    action_hash: H256::default(),
                    prepaid_gas: DEFAULT_PREPAID_GAS,
                }
            }
            BorshableTransactionMessage::V2(t) => {
                let transaction: TransactionKind = t.transaction.into();
                Self {
                    block_hash: H256(t.block_hash),
                    near_receipt_id: H256(t.near_receipt_id),
                    position: t.position,
                    succeeded: t.succeeded,
                    signer: t.signer.into_owned(),
                    caller: t.caller.into_owned(),
                    attached_near: t.attached_near,
                    transaction,
                    promise_data: t.promise_data.into_owned(),
                    action_hash: H256::default(),
                    prepaid_gas: DEFAULT_PREPAID_GAS,
                }
            }
            BorshableTransactionMessage::V3(t) => Self {
                block_hash: H256(t.block_hash),
                near_receipt_id: H256(t.near_receipt_id),
                position: t.position,
                succeeded: t.succeeded,
                signer: t.signer.into_owned(),
                caller: t.caller.into_owned(),
                attached_near: t.attached_near,
                transaction: t.transaction.into(),
                promise_data: t.promise_data.into_owned(),
                action_hash: H256::default(),
                prepaid_gas: DEFAULT_PREPAID_GAS,
            },
            BorshableTransactionMessage::V4(t) => Self {
                block_hash: H256(t.block_hash),
                near_receipt_id: H256(t.near_receipt_id),
                position: t.position,
                succeeded: t.succeeded,
                signer: t.signer.into_owned(),
                caller: t.caller.into_owned(),
                attached_near: t.attached_near,
                transaction: t.transaction.into(),
                promise_data: t.promise_data.into_owned(),
                action_hash: H256(t.action_hash),
                prepaid_gas: DEFAULT_PREPAID_GAS,
            },
        }
    }
}

/// Same as `TransactionKind`, but with `Submit` variant replaced with raw bytes
/// so that it can derive the Borsh traits. All non-copy elements are `Cow` also
/// so that this type can be cheaply created from a `TransactionKind` reference.
/// !!!!! New types of transactions must be added at the end of the enum. !!!!!!
#[derive(BorshDeserialize, BorshSerialize, Clone)]
#[borsh(crate = "aurora_engine_types::borsh")]
struct BorshableTransactionKind<'a> {
    method_name: String,
    args: Cow<'a, [u8]>,
    #[allow(clippy::option_option)]
    promise_result: Option<Option<Cow<'a, [u8]>>>,
}

impl<'a> From<&'a TransactionKind> for BorshableTransactionKind<'a> {
    fn from(t: &'a TransactionKind) -> Self {
        BorshableTransactionKind {
            method_name: t.method_name.clone(),
            args: Cow::Borrowed(&t.args),
            promise_result: match &t.promise_result {
                Some(PromiseResult::Successful(v)) => Some(Some(Cow::Borrowed(v))),
                Some(PromiseResult::Failed) => Some(None),
                Some(PromiseResult::NotReady) => {
                    debug_assert!(false, "should never happen");
                    Some(None)
                }
                _ => None,
            },
        }
    }
}

impl<'a> From<BorshableTransactionKind<'a>> for TransactionKind {
    fn from(t: BorshableTransactionKind<'a>) -> Self {
        Self {
            method_name: t.method_name,
            args: t.args.into_owned(),
            promise_result: t.promise_result.map(|res| {
                res.map_or(PromiseResult::Failed, |result| {
                    PromiseResult::Successful(result.into_owned())
                })
            }),
        }
    }
}
