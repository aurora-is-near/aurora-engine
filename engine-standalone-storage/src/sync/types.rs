use aurora_engine::parameters;
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::{types, H256};

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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
}
