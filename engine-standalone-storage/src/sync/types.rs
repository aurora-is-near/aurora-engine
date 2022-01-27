use aurora_engine::parameters;
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::H256;

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
    /// Hash of the transaction on NEAR
    pub near_tx_hash: H256,
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
}
