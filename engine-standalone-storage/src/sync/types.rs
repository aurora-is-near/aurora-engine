use aurora_engine::parameters;
use aurora_engine::xcc::AddressVersionUpdateArgs;
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::{types, H256};
use borsh::{BorshDeserialize, BorshSerialize};
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
}

impl TransactionMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        let borshable: BorshableTransactionMessage = self.into();
        borshable.try_to_vec().unwrap()
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, std::io::Error> {
        let borshable = BorshableTransactionMessage::try_from_slice(bytes)?;
        Self::try_from(borshable).map_err(|e| {
            let message = e.as_str();
            std::io::Error::new(std::io::ErrorKind::Other, message)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum TransactionKind {
    /// Raw Ethereum transaction submitted to the engine
    Submit(EthTransactionKind),
    /// Ethereum transaction triggered by a NEAR account
    Call(parameters::CallArgs),
    /// Input here represents the EVM code used to create the new contract
    Deploy(Vec<u8>),
    /// New bridged token
    DeployErc20(parameters::DeployErc20TokenArgs),
    /// This type of transaction can impact the aurora state because of the bridge
    FtOnTransfer(parameters::NEP141FtOnTransferArgs),
    /// Bytes here will be parsed into `aurora_engine::proof::Proof`
    Deposit(Vec<u8>),
    /// This can change balances on aurora in the case that `receiver_id == aurora`.
    /// Example: https://explorer.mainnet.near.org/transactions/DH6iNvXCt5n5GZBZPV1A6sLmMf1EsKcxXE4uqk1cShzj
    FtTransferCall(parameters::TransferCallCallArgs),
    /// FinishDeposit-type receipts are created by calls to `deposit`
    FinishDeposit(parameters::FinishDepositCallArgs),
    /// ResolveTransfer-type receipts are created by calls to ft_on_transfer
    ResolveTransfer(parameters::ResolveTransferCallArgs, types::PromiseResult),
    /// ft_transfer (related to eth-connector)
    FtTransfer(parameters::TransferCallArgs),
    /// Function to take ETH out of Aurora
    Withdraw(aurora_engine_types::parameters::WithdrawCallArgs),
    /// FT storage standard method
    StorageDeposit(parameters::StorageDepositCallArgs),
    /// FT storage standard method
    StorageUnregister(Option<bool>),
    /// FT storage standard method
    StorageWithdraw(parameters::StorageWithdrawCallArgs),
    /// Admin only method
    SetPausedFlags(parameters::PauseEthConnectorCallArgs),
    /// Ad entry mapping from address to relayer NEAR account
    RegisterRelayer(types::Address),
    /// Called if exist precompiles fail
    RefundOnError(Option<aurora_engine_types::parameters::RefundCallArgs>),
    /// Update eth-connector config
    SetConnectorData(parameters::SetContractDataCallArgs),
    /// Initialize eth-connector
    NewConnector(parameters::InitCallArgs),
    /// Initialize Engine
    NewEngine(parameters::NewCallArgs),
    /// Update xcc-router bytecode
    FactoryUpdate(Vec<u8>),
    /// Update the version of a deployed xcc-router contract
    FactoryUpdateAddressVersion(AddressVersionUpdateArgs),
    /// Sentinel kind for cases where a NEAR receipt caused a
    /// change in Aurora state, but we failed to parse the Action.
    Unknown,
}

#[derive(BorshDeserialize, BorshSerialize)]
struct BorshableTransactionMessage<'a> {
    /// Hash of the block which included this transaction
    pub block_hash: [u8; 32],
    /// Receipt ID of the receipt that was actually executed on NEAR
    pub near_receipt_id: [u8; 32],
    /// If multiple Aurora transactions are included in the same block,
    /// this index gives the order in which they should be executed.
    pub position: u16,
    /// True if the transaction executed successfully on the blockchain, false otherwise.
    pub succeeded: bool,
    /// NEAR account that signed the transaction
    pub signer: Cow<'a, AccountId>,
    /// NEAR account that called the Aurora engine contract
    pub caller: Cow<'a, AccountId>,
    /// Amount of NEAR token attached to the transaction
    pub attached_near: u128,
    /// Details of the transaction that was executed
    pub transaction: BorshableTransactionKind<'a>,
}

impl<'a> From<&'a TransactionMessage> for BorshableTransactionMessage<'a> {
    fn from(t: &'a TransactionMessage) -> Self {
        Self {
            block_hash: t.block_hash.0,
            near_receipt_id: t.near_receipt_id.0,
            position: t.position,
            succeeded: t.succeeded,
            signer: Cow::Borrowed(&t.signer),
            caller: Cow::Borrowed(&t.caller),
            attached_near: t.attached_near,
            transaction: (&t.transaction).into(),
        }
    }
}

impl<'a> TryFrom<BorshableTransactionMessage<'a>> for TransactionMessage {
    type Error = aurora_engine_transactions::ParseTransactionError;

    fn try_from(t: BorshableTransactionMessage<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            block_hash: H256(t.block_hash),
            near_receipt_id: H256(t.near_receipt_id),
            position: t.position,
            succeeded: t.succeeded,
            signer: t.signer.into_owned(),
            caller: t.caller.into_owned(),
            attached_near: t.attached_near,
            transaction: t.transaction.try_into()?,
        })
    }
}

/// Same as `TransactionKind`, but with `Submit` variant replaced with raw bytes
/// so that it can derive the Borsh traits. All non-copy elements are `Cow` also
/// so that this type can be cheaply created from a `TransactionKind` reference.
#[derive(BorshDeserialize, BorshSerialize, Clone)]
enum BorshableTransactionKind<'a> {
    Submit(Cow<'a, Vec<u8>>),
    Call(Cow<'a, parameters::CallArgs>),
    Deploy(Cow<'a, Vec<u8>>),
    DeployErc20(Cow<'a, parameters::DeployErc20TokenArgs>),
    FtOnTransfer(Cow<'a, parameters::NEP141FtOnTransferArgs>),
    Deposit(Cow<'a, Vec<u8>>),
    FtTransferCall(Cow<'a, parameters::TransferCallCallArgs>),
    FinishDeposit(Cow<'a, parameters::FinishDepositCallArgs>),
    ResolveTransfer(
        Cow<'a, parameters::ResolveTransferCallArgs>,
        Cow<'a, types::PromiseResult>,
    ),
    FtTransfer(Cow<'a, parameters::TransferCallArgs>),
    Withdraw(Cow<'a, aurora_engine_types::parameters::WithdrawCallArgs>),
    StorageDeposit(Cow<'a, parameters::StorageDepositCallArgs>),
    StorageUnregister(Option<bool>),
    StorageWithdraw(Cow<'a, parameters::StorageWithdrawCallArgs>),
    SetPausedFlags(Cow<'a, parameters::PauseEthConnectorCallArgs>),
    RegisterRelayer(Cow<'a, types::Address>),
    RefundOnError(Cow<'a, Option<aurora_engine_types::parameters::RefundCallArgs>>),
    SetConnectorData(Cow<'a, parameters::SetContractDataCallArgs>),
    NewConnector(Cow<'a, parameters::InitCallArgs>),
    NewEngine(Cow<'a, parameters::NewCallArgs>),
    FactoryUpdate(Cow<'a, Vec<u8>>),
    FactoryUpdateAddressVersion(Cow<'a, AddressVersionUpdateArgs>),
    Unknown,
}

impl<'a> From<&'a TransactionKind> for BorshableTransactionKind<'a> {
    fn from(t: &'a TransactionKind) -> Self {
        match t {
            TransactionKind::Submit(eth_tx) => {
                let tx_bytes = eth_tx.into();
                Self::Submit(Cow::Owned(tx_bytes))
            }
            TransactionKind::Call(x) => Self::Call(Cow::Borrowed(x)),
            TransactionKind::Deploy(x) => Self::Deploy(Cow::Borrowed(x)),
            TransactionKind::DeployErc20(x) => Self::DeployErc20(Cow::Borrowed(x)),
            TransactionKind::FtOnTransfer(x) => Self::FtOnTransfer(Cow::Borrowed(x)),
            TransactionKind::Deposit(x) => Self::Deposit(Cow::Borrowed(x)),
            TransactionKind::FtTransferCall(x) => Self::FtTransferCall(Cow::Borrowed(x)),
            TransactionKind::FinishDeposit(x) => Self::FinishDeposit(Cow::Borrowed(x)),
            TransactionKind::ResolveTransfer(x, y) => {
                Self::ResolveTransfer(Cow::Borrowed(x), Cow::Borrowed(y))
            }
            TransactionKind::FtTransfer(x) => Self::FtTransfer(Cow::Borrowed(x)),
            TransactionKind::Withdraw(x) => Self::Withdraw(Cow::Borrowed(x)),
            TransactionKind::StorageDeposit(x) => Self::StorageDeposit(Cow::Borrowed(x)),
            TransactionKind::StorageUnregister(x) => Self::StorageUnregister(*x),
            TransactionKind::StorageWithdraw(x) => Self::StorageWithdraw(Cow::Borrowed(x)),
            TransactionKind::SetPausedFlags(x) => Self::SetPausedFlags(Cow::Borrowed(x)),
            TransactionKind::RegisterRelayer(x) => Self::RegisterRelayer(Cow::Borrowed(x)),
            TransactionKind::RefundOnError(x) => Self::RefundOnError(Cow::Borrowed(x)),
            TransactionKind::SetConnectorData(x) => Self::SetConnectorData(Cow::Borrowed(x)),
            TransactionKind::NewConnector(x) => Self::NewConnector(Cow::Borrowed(x)),
            TransactionKind::NewEngine(x) => Self::NewEngine(Cow::Borrowed(x)),
            TransactionKind::FactoryUpdate(x) => Self::FactoryUpdate(Cow::Borrowed(x)),
            TransactionKind::FactoryUpdateAddressVersion(x) => {
                Self::FactoryUpdateAddressVersion(Cow::Borrowed(x))
            }
            TransactionKind::Unknown => Self::Unknown,
        }
    }
}

impl<'a> TryFrom<BorshableTransactionKind<'a>> for TransactionKind {
    type Error = aurora_engine_transactions::ParseTransactionError;

    fn try_from(t: BorshableTransactionKind<'a>) -> Result<Self, Self::Error> {
        match t {
            BorshableTransactionKind::Submit(tx_bytes) => {
                // `BorshableTransactionKind` is an internal type, so we will
                // assume the conversion is infallible. If the conversion were to
                // fail then something has gone very wrong.
                let eth_tx = tx_bytes.as_slice().try_into()?;
                Ok(Self::Submit(eth_tx))
            }
            BorshableTransactionKind::Call(x) => Ok(Self::Call(x.into_owned())),
            BorshableTransactionKind::Deploy(x) => Ok(Self::Deploy(x.into_owned())),
            BorshableTransactionKind::DeployErc20(x) => Ok(Self::DeployErc20(x.into_owned())),
            BorshableTransactionKind::FtOnTransfer(x) => Ok(Self::FtOnTransfer(x.into_owned())),
            BorshableTransactionKind::Deposit(x) => Ok(Self::Deposit(x.into_owned())),
            BorshableTransactionKind::FtTransferCall(x) => Ok(Self::FtTransferCall(x.into_owned())),
            BorshableTransactionKind::FinishDeposit(x) => Ok(Self::FinishDeposit(x.into_owned())),
            BorshableTransactionKind::ResolveTransfer(x, y) => {
                Ok(Self::ResolveTransfer(x.into_owned(), y.into_owned()))
            }
            BorshableTransactionKind::FtTransfer(x) => Ok(Self::FtTransfer(x.into_owned())),
            BorshableTransactionKind::Withdraw(x) => Ok(Self::Withdraw(x.into_owned())),
            BorshableTransactionKind::StorageDeposit(x) => Ok(Self::StorageDeposit(x.into_owned())),
            BorshableTransactionKind::StorageUnregister(x) => Ok(Self::StorageUnregister(x)),
            BorshableTransactionKind::StorageWithdraw(x) => {
                Ok(Self::StorageWithdraw(x.into_owned()))
            }
            BorshableTransactionKind::SetPausedFlags(x) => Ok(Self::SetPausedFlags(x.into_owned())),
            BorshableTransactionKind::RegisterRelayer(x) => {
                Ok(Self::RegisterRelayer(x.into_owned()))
            }
            BorshableTransactionKind::RefundOnError(x) => Ok(Self::RefundOnError(x.into_owned())),
            BorshableTransactionKind::SetConnectorData(x) => {
                Ok(Self::SetConnectorData(x.into_owned()))
            }
            BorshableTransactionKind::NewConnector(x) => Ok(Self::NewConnector(x.into_owned())),
            BorshableTransactionKind::NewEngine(x) => Ok(Self::NewEngine(x.into_owned())),
            BorshableTransactionKind::FactoryUpdate(x) => Ok(Self::FactoryUpdate(x.into_owned())),
            BorshableTransactionKind::FactoryUpdateAddressVersion(x) => {
                Ok(Self::FactoryUpdateAddressVersion(x.into_owned()))
            }
            BorshableTransactionKind::Unknown => Ok(Self::Unknown),
        }
    }
}
