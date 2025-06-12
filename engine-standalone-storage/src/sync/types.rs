use crate::Storage;
use aurora_engine::xcc::{AddressVersionUpdateArgs, FundXccArgs};
use aurora_engine_sdk::near_runtime::Runtime;
use aurora_engine_transactions::{EthTransactionKind, NormalizedEthTransaction};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::FtTransferMessageData;
use aurora_engine_types::parameters::xcc::WithdrawWnearToRouterArgs;
use aurora_engine_types::parameters::{connector, engine, silo};
use aurora_engine_types::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    types::{self, Address, Wei},
    H256, U256,
};
use serde::Serialize;
use std::borrow::Cow;
use strum::EnumString;

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
    /// Raw bytes passed as input when executed in the Near Runtime.
    pub raw_input: Vec<u8>,
    /// A Near protocol quantity equal to
    /// `sha256(receipt_id || block_hash || le_bytes(u64 - action_index))`.
    /// This quantity is used together with the block random seed
    /// to generate the random value available to the transaction.
    /// nearcore references:
    /// - <https://github.com/near/nearcore/blob/00ca2f3f73e2a547ba881f76ecc59450dbbef6e2/core/primitives/src/utils.rs#L261>
    /// - <https://github.com/near/nearcore/blob/00ca2f3f73e2a547ba881f76ecc59450dbbef6e2/core/primitives/src/utils.rs#L295>
    pub action_hash: H256,
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
    /// Raw Ethereum transaction with additional arguments submitted to the engine
    SubmitWithArgs(engine::SubmitArgs),
    /// Ethereum transaction triggered by a NEAR account
    Call(engine::CallArgs),
    /// Administrative method that makes a subset of precompiles paused
    PausePrecompiles(engine::PausePrecompilesCallArgs),
    /// Administrative method that resumes previously paused subset of precompiles
    ResumePrecompiles(engine::PausePrecompilesCallArgs),
    /// Input here represents the EVM code used to create the new contract
    Deploy(Vec<u8>),
    /// New bridged token
    DeployErc20(engine::DeployErc20TokenArgs),
    /// Callback for the `deploy_erc20_token` method
    DeployErc20Callback(AccountId),
    /// This type of transaction can impact the aurora state because of the bridge
    FtOnTransfer(connector::FtOnTransferArgs),
    /// Bytes here will be parsed into `aurora_engine::proof::Proof`
    Deposit(Vec<u8>),
    /// This can change balances on aurora in the case that `receiver_id == aurora`.
    /// Example: <https://explorer.mainnet.near.org/transactions/DH6iNvXCt5n5GZBZPV1A6sLmMf1EsKcxXE4uqk1cShzj>
    FtTransferCall(connector::FtTransferCallArgs),
    /// FinishDeposit-type receipts are created by calls to `deposit`
    FinishDeposit(connector::FinishDepositArgs),
    /// ResolveTransfer-type receipts are created by calls to `ft_on_transfer`
    ResolveTransfer(connector::FtResolveTransferArgs, types::PromiseResult),
    /// `ft_transfer` (related to eth-connector)
    FtTransfer(connector::FtTransferArgs),
    /// Function to take ETH out of Aurora
    Withdraw(connector::WithdrawCallArgs),
    /// FT storage standard method
    StorageDeposit(connector::StorageDepositArgs),
    /// FT storage standard method
    StorageUnregister(Option<bool>),
    /// FT storage standard method
    StorageWithdraw(connector::StorageWithdrawArgs),
    /// Admin only method; used to transfer administration
    SetOwner(engine::SetOwnerArgs),
    /// Admin only method; used to change upgrade delay blocks
    SetUpgradeDelayBlocks(engine::SetUpgradeDelayBlocksArgs),
    /// Set pause flags to eth-connector
    SetPausedFlags(connector::PauseEthConnectorArgs),
    /// Ad entry mapping from address to relayer NEAR account
    RegisterRelayer(Address),
    /// Callback called by `ExitToNear` precompile, also can refund on fail
    ExitToNear(Option<connector::ExitToNearPrecompileCallbackArgs>),
    /// Update eth-connector config
    SetConnectorData(connector::SetContractDataCallArgs),
    /// Initialize eth-connector
    NewConnector(connector::InitCallArgs),
    /// Set account id of the external eth-connector.
    SetEthConnectorContractAccount(connector::SetEthConnectorContractAccountArgs),
    /// Initialize Engine
    NewEngine(engine::NewCallArgs),
    /// Update xcc-router bytecode
    FactoryUpdate(Vec<u8>),
    /// Update the version of a deployed xcc-router contract
    FactoryUpdateAddressVersion(AddressVersionUpdateArgs),
    FactorySetWNearAddress(Address),
    FundXccSubAccount(FundXccArgs),
    /// Self-call used during XCC flow to move wNEAR tokens to user's XCC account
    WithdrawWnearToRouter(WithdrawWnearToRouterArgs),
    /// Pause the contract
    PauseContract,
    /// Resume the contract
    ResumeContract,
    /// Set the relayer key manager
    SetKeyManager(engine::RelayerKeyManagerArgs),
    /// Add a new relayer public function call access key
    AddRelayerKey(engine::RelayerKeyArgs),
    /// Callback which stores the relayer public function call access key into the storage
    StoreRelayerKeyCallback(engine::RelayerKeyArgs),
    /// Remove the relayer public function call access key
    RemoveRelayerKey(engine::RelayerKeyArgs),
    StartHashchain(engine::StartHashchainArgs),
    /// Set metadata of ERC-20 contract.
    SetErc20Metadata(connector::SetErc20MetadataArgs),
    /// Silo operations
    SetFixedGas(silo::FixedGasArgs),
    SetErc20FallbackAddress(silo::Erc20FallbackAddressArgs),
    SetSiloParams(Option<silo::SiloParamsArgs>),
    AddEntryToWhitelist(silo::WhitelistArgs),
    AddEntryToWhitelistBatch(Vec<silo::WhitelistArgs>),
    RemoveEntryFromWhitelist(silo::WhitelistArgs),
    SetWhitelistStatus(silo::WhitelistStatusArgs),
    SetWhitelistsStatuses(Vec<silo::WhitelistStatusArgs>),
    /// Callback which mirrors existed ERC-20 contract deployed on the main contract.
    MirrorErc20TokenCallback(connector::MirrorErc20TokenArgs),
    /// Sentinel kind for cases where a NEAR receipt caused a
    /// change in Aurora state, but we failed to parse the Action.
    Unknown,
}

impl TransactionKind {
    // TODO: The method is used nowhere now. It should be actualized and then potentially, used
    // TODO: in the borealis-refiner or removed at all, because now the borealis-refiner includes
    // TODO: its custom logic for creating the `NormalizedEthTransaction`s.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn eth_repr(
        self,
        engine_account: &AccountId,
        caller: &AccountId,
        block_height: u64,
        transaction_position: u16,
        storage: &Storage,
    ) -> NormalizedEthTransaction {
        match self {
            // In the case the submit arg fails to normalize, there is no EVM execution
            Self::Submit(eth_tx_kind) => eth_tx_kind
                .try_into()
                .unwrap_or_else(|_| Self::no_evm_execution("submit")),
            Self::SubmitWithArgs(args) => EthTransactionKind::try_from(args.tx_data.as_slice())
                .and_then(TryInto::try_into)
                .unwrap_or_else(|_| Self::no_evm_execution("submit_with_args")),
            Self::Call(call_args) => {
                let from = Self::get_implicit_address(caller);
                let nonce =
                    Self::get_implicit_nonce(&from, block_height, transaction_position, storage);
                let (to, data, value) = match call_args {
                    engine::CallArgs::V1(args) => (args.contract, args.input, Wei::zero()),
                    engine::CallArgs::V2(args) => (
                        args.contract,
                        args.input,
                        Wei::new(U256::from_big_endian(&args.value)),
                    ),
                };
                NormalizedEthTransaction {
                    address: from,
                    chain_id: None,
                    nonce,
                    gas_limit: U256::from(u64::MAX),
                    max_priority_fee_per_gas: U256::zero(),
                    max_fee_per_gas: U256::zero(),
                    to: Some(to),
                    value,
                    data,
                    access_list: Vec::new(),
                }
            }
            Self::Deploy(data) => {
                let from = Self::get_implicit_address(caller);
                let nonce =
                    Self::get_implicit_nonce(&from, block_height, transaction_position, storage);
                NormalizedEthTransaction {
                    address: from,
                    chain_id: None,
                    nonce,
                    gas_limit: U256::from(u64::MAX),
                    max_priority_fee_per_gas: U256::zero(),
                    max_fee_per_gas: U256::zero(),
                    to: None,
                    value: Wei::zero(),
                    data,
                    access_list: Vec::new(),
                }
            }
            Self::DeployErc20(_) | Self::DeployErc20Callback(_) => {
                let from = Self::get_implicit_address(caller);
                let nonce =
                    Self::get_implicit_nonce(&from, block_height, transaction_position, storage);
                let data = aurora_engine::engine::setup_deploy_erc20_input(engine_account, None);
                NormalizedEthTransaction {
                    address: from,
                    chain_id: None,
                    nonce,
                    gas_limit: U256::from(u64::MAX),
                    max_priority_fee_per_gas: U256::zero(),
                    max_fee_per_gas: U256::zero(),
                    to: None,
                    value: Wei::zero(),
                    data,
                    access_list: Vec::new(),
                }
            }
            Self::FtOnTransfer(args) => {
                if engine_account == caller {
                    let recipient = FtTransferMessageData::try_from(args.msg.as_str())
                        .map(|data| data.recipient)
                        .unwrap_or_default();
                    let value = Wei::new(U256::from(args.amount.as_u128()));
                    // This transaction mints new ETH, so we'll say it comes from the zero address.
                    NormalizedEthTransaction {
                        address: Address::default(),
                        chain_id: None,
                        nonce: U256::zero(),
                        gas_limit: U256::from(u64::MAX),
                        max_priority_fee_per_gas: U256::zero(),
                        max_fee_per_gas: U256::zero(),
                        to: Some(recipient),
                        value,
                        data: Vec::new(),
                        access_list: Vec::new(),
                    }
                } else {
                    let from = Self::get_implicit_address(engine_account);
                    let nonce = Self::get_implicit_nonce(
                        &from,
                        block_height,
                        transaction_position,
                        storage,
                    );
                    let to = storage
                        .with_engine_access(block_height, transaction_position, &[], |_| {
                            aurora_engine::engine::get_erc20_from_nep141(&Runtime, caller)
                        })
                        .result
                        .unwrap_or_default();
                    let erc20_recipient = hex::decode(&args.msg.as_bytes()[0..40])
                        .ok()
                        .and_then(|bytes| Address::try_from_slice(&bytes).ok())
                        .unwrap_or_default();
                    let data = aurora_engine::engine::setup_receive_erc20_tokens_input(
                        &erc20_recipient,
                        args.amount.as_u128(),
                    );
                    NormalizedEthTransaction {
                        address: from,
                        chain_id: None,
                        nonce,
                        gas_limit: U256::from(u64::MAX),
                        max_priority_fee_per_gas: U256::zero(),
                        max_fee_per_gas: U256::zero(),
                        to: Some(to),
                        value: Wei::zero(),
                        data,
                        access_list: Vec::new(),
                    }
                }
            }
            Self::ExitToNear(maybe_args) => {
                let method_name = "exit_to_near_precompile_callback";
                maybe_args.map_or_else(
                    || Self::no_evm_execution(method_name),
                    |args| {
                        args.refund.map_or_else(
                            || Self::no_evm_execution(method_name),
                            |args| {
                                args.erc20_address.map_or_else(|| {
                                    // ETH refund
                                    let value = Wei::new(U256::from_big_endian(&args.amount));
                                    let from = aurora_engine_precompiles::native::exit_to_near::ADDRESS;
                                    let nonce = Self::get_implicit_nonce(
                                        &from,
                                        block_height,
                                        transaction_position,
                                        storage,
                                    );
                                    NormalizedEthTransaction {
                                        address: from,
                                        chain_id: None,
                                        nonce,
                                        gas_limit: U256::from(u64::MAX),
                                        max_priority_fee_per_gas: U256::zero(),
                                        max_fee_per_gas: U256::zero(),
                                        to: Some(args.recipient_address),
                                        value,
                                        data: Vec::new(),
                                        access_list: Vec::new(),
                                    }
                                },
                                   |erc20_address| {
                                       // ERC-20 refund
                                       let from = Self::get_implicit_address(engine_account);
                                       let nonce = Self::get_implicit_nonce(
                                           &from,
                                           block_height,
                                           transaction_position,
                                           storage,
                                       );
                                       let to = erc20_address;
                                       let data = aurora_engine::engine::setup_refund_on_error_input(
                                           U256::from_big_endian(&args.amount),
                                           args.recipient_address,
                                       );
                                       NormalizedEthTransaction {
                                           address: from,
                                           chain_id: None,
                                           nonce,
                                           gas_limit: U256::from(u64::MAX),
                                           max_priority_fee_per_gas: U256::zero(),
                                           max_fee_per_gas: U256::zero(),
                                           to: Some(to),
                                           value: Wei::zero(),
                                           data,
                                           access_list: Vec::new(),
                                       }
                                   },
                                )
                            },
                        )
                    },
                )
            }
            Self::WithdrawWnearToRouter(args) => {
                let recipient = AccountId::try_from(format!(
                    "{}.{}",
                    args.target.encode(),
                    engine_account.as_ref()
                ))
                .unwrap_or_else(|_| engine_account.clone());
                let wnear_address = storage
                    .with_engine_access(block_height, transaction_position, &[], |_| {
                        aurora_engine_precompiles::xcc::state::get_wnear_address(&Runtime)
                    })
                    .result;
                let call_args = aurora_engine::xcc::withdraw_wnear_call_args(
                    &recipient,
                    args.amount,
                    wnear_address,
                );
                Self::Call(call_args).eth_repr(
                    engine_account,
                    caller,
                    block_height,
                    transaction_position,
                    storage,
                )
            }
            Self::Deposit(_) => Self::no_evm_execution("deposit"),
            Self::FtTransferCall(_) => Self::no_evm_execution("ft_transfer_call"),
            Self::FinishDeposit(_) => Self::no_evm_execution("finish_deposit"),
            Self::ResolveTransfer(_, _) => Self::no_evm_execution("resolve_transfer"),
            Self::FtTransfer(_) => Self::no_evm_execution("ft_transfer"),
            Self::Withdraw(_) => Self::no_evm_execution("withdraw"),
            Self::StorageDeposit(_) => Self::no_evm_execution("storage_deposit"),
            Self::StorageUnregister(_) => Self::no_evm_execution("storage_unregister"),
            Self::StorageWithdraw(_) => Self::no_evm_execution("storage_withdraw"),
            Self::SetPausedFlags(_) => Self::no_evm_execution("set_paused_flags"),
            Self::RegisterRelayer(_) => Self::no_evm_execution("register_relayer"),
            Self::SetConnectorData(_) => Self::no_evm_execution("set_connector_data"),
            Self::NewConnector(_) => Self::no_evm_execution("new_connector"),
            Self::SetEthConnectorContractAccount(_) => {
                Self::no_evm_execution("set_eth_connector_contract_account")
            }
            Self::NewEngine(_) => Self::no_evm_execution("new_engine"),
            Self::FactoryUpdate(_) => Self::no_evm_execution("factory_update"),
            Self::FactoryUpdateAddressVersion(_) => {
                Self::no_evm_execution("factory_update_address_version")
            }
            Self::FactorySetWNearAddress(_) => Self::no_evm_execution("factory_set_wnear_address"),
            Self::Unknown => Self::no_evm_execution("unknown"),
            Self::PausePrecompiles(_) => Self::no_evm_execution("pause_precompiles"),
            Self::ResumePrecompiles(_) => Self::no_evm_execution("resume_precompiles"),
            Self::SetOwner(_) => Self::no_evm_execution("set_owner"),
            Self::SetUpgradeDelayBlocks(_) => Self::no_evm_execution("set_upgrade_delay_blocks"),
            Self::FundXccSubAccount(_) => Self::no_evm_execution("fund_xcc_sub_account"),
            Self::PauseContract => Self::no_evm_execution("pause_contract"),
            Self::ResumeContract => Self::no_evm_execution("resume_contract"),
            Self::SetKeyManager(_) => Self::no_evm_execution("set_key_manager"),
            Self::AddRelayerKey(_) => Self::no_evm_execution("add_relayer_key"),
            Self::StoreRelayerKeyCallback(_) => {
                Self::no_evm_execution("store_relayer_key_callback")
            }
            Self::RemoveRelayerKey(_) => Self::no_evm_execution("remove_relayer_key"),
            Self::StartHashchain(_) => Self::no_evm_execution("start_hashchain"),
            Self::SetErc20Metadata(_) => Self::no_evm_execution("set_erc20_metadata"),
            Self::SetFixedGas(_) => Self::no_evm_execution("set_fixed_gas"),
            Self::SetErc20FallbackAddress(_) => {
                Self::no_evm_execution("set_erc20_fallback_address")
            }
            Self::SetSiloParams(_) => Self::no_evm_execution("set_silo_params"),
            Self::AddEntryToWhitelist(_) => Self::no_evm_execution("add_entry_to_whitelist"),
            Self::AddEntryToWhitelistBatch(_) => {
                Self::no_evm_execution("add_entry_to_whitelist_batch")
            }
            Self::RemoveEntryFromWhitelist(_) => {
                Self::no_evm_execution("remove_entry_from_whitelist")
            }
            Self::SetWhitelistStatus(_) => Self::no_evm_execution("set_whitelist_status"),
            Self::SetWhitelistsStatuses(_) => Self::no_evm_execution("set_whitelists_statuses"),
            Self::MirrorErc20TokenCallback(_) => {
                // TODO: Basically, the transaction involves EVM execution because it deploys
                // TODO: ERC-20 contract, therefore it should be fixed before using in the
                // TODO: borealis-refiner.
                Self::no_evm_execution("mirror_erc20_token_callback")
            }
        }
    }

    /// There are many cases where a receipt on NEAR can change the Aurora contract state,
    /// but no EVM execution actually occurs. In these cases we have a sentinel Ethereum transaction
    /// from the zero address to itself with input equal to the method name.
    fn no_evm_execution(method_name: &str) -> NormalizedEthTransaction {
        NormalizedEthTransaction {
            address: Address::from_array([0; 20]),
            chain_id: None,
            nonce: U256::zero(),
            gas_limit: U256::zero(),
            max_priority_fee_per_gas: U256::zero(),
            max_fee_per_gas: U256::zero(),
            to: Some(Address::from_array([0; 20])),
            value: Wei::zero(),
            data: method_name.as_bytes().to_vec(),
            access_list: Vec::new(),
        }
    }

    fn get_implicit_address(caller: &AccountId) -> Address {
        aurora_engine_sdk::types::near_account_to_evm_address(caller.as_bytes())
    }

    fn get_implicit_nonce(
        from: &Address,
        block_height: u64,
        transaction_position: u16,
        storage: &Storage,
    ) -> U256 {
        storage
            .with_engine_access(block_height, transaction_position, &[], |_| {
                aurora_engine::engine::get_nonce(&Runtime, from)
            })
            .result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString)]
pub enum TransactionKindTag {
    #[strum(serialize = "submit")]
    Submit,
    #[strum(serialize = "call")]
    Call,
    #[strum(serialize = "pause_precompiles")]
    PausePrecompiles,
    #[strum(serialize = "resume_precompiles")]
    ResumePrecompiles,
    #[strum(serialize = "deploy_code")]
    Deploy,
    #[strum(serialize = "deploy_erc20_token")]
    DeployErc20,
    #[strum(serialize = "deploy_erc20_token_callback")]
    DeployErc20Callback,
    #[strum(serialize = "ft_on_transfer")]
    FtOnTransfer,
    #[strum(serialize = "deposit")]
    Deposit,
    #[strum(serialize = "ft_transfer_call")]
    FtTransferCall,
    #[strum(serialize = "finish_deposit")]
    FinishDeposit,
    #[strum(serialize = "ft_resolve_transfer")]
    ResolveTransfer,
    #[strum(serialize = "ft_transfer")]
    FtTransfer,
    #[strum(serialize = "withdraw")]
    Withdraw,
    #[strum(serialize = "storage_deposit")]
    StorageDeposit,
    #[strum(serialize = "storage_unregister")]
    StorageUnregister,
    #[strum(serialize = "storage_withdraw")]
    StorageWithdraw,
    #[strum(serialize = "set_paused_flags")]
    SetPausedFlags,
    #[strum(serialize = "register_relayer")]
    RegisterRelayer,
    #[strum(serialize = "exit_to_near_precompile_callback")]
    ExitToNear,
    #[strum(serialize = "set_eth_connector_contract_data")]
    SetConnectorData,
    #[strum(serialize = "new_eth_connector")]
    NewConnector,
    #[strum(serialize = "new")]
    NewEngine,
    #[strum(serialize = "factory_update")]
    FactoryUpdate,
    #[strum(serialize = "factory_update_address_version")]
    FactoryUpdateAddressVersion,
    #[strum(serialize = "factory_set_wnear_address")]
    FactorySetWNearAddress,
    #[strum(serialize = "set_owner")]
    SetOwner,
    #[strum(serialize = "submit_with_args")]
    SubmitWithArgs,
    #[strum(serialize = "set_upgrade_delay_blocks")]
    SetUpgradeDelayBlocks,
    #[strum(serialize = "fund_xcc_sub_account")]
    FundXccSubAccount,
    #[strum(serialize = "pause_contract")]
    PauseContract,
    #[strum(serialize = "resume_contract")]
    ResumeContract,
    #[strum(serialize = "set_key_manager")]
    SetKeyManager,
    #[strum(serialize = "add_relayer_key")]
    AddRelayerKey,
    #[strum(serialize = "store_relayer_key_callback")]
    StoreRelayerKeyCallback,
    #[strum(serialize = "remove_relayer_key")]
    RemoveRelayerKey,
    #[strum(serialize = "start_hashchain")]
    StartHashchain,
    #[strum(serialize = "set_erc20_metadata")]
    SetErc20Metadata,
    #[strum(serialize = "set_eth_connector_contract_account")]
    SetEthConnectorContractAccount,
    #[strum(serialize = "set_fixed_gas")]
    SetFixedGas,
    #[strum(serialize = "set_erc20_fallback_address")]
    SetErc20FallbackAddress,
    #[strum(serialize = "set_silo_params")]
    SetSiloParams,
    #[strum(serialize = "set_whitelist_status")]
    SetWhitelistStatus,
    #[strum(serialize = "set_whitelists_statuses")]
    SetWhitelistsStatuses,
    #[strum(serialize = "add_entry_to_whitelist")]
    AddEntryToWhitelist,
    #[strum(serialize = "add_entry_to_whitelist_batch")]
    AddEntryToWhitelistBatch,
    #[strum(serialize = "remove_entry_from_whitelist")]
    RemoveEntryFromWhitelist,
    #[strum(serialize = "mirror_erc20_token_callback")]
    MirrorErc20TokenCallback,
    #[strum(serialize = "withdraw_wnear_to_router")]
    WithdrawWnearToRouter,
    Unknown,
}

impl TransactionKind {
    #[must_use]
    pub fn raw_bytes(&self) -> Vec<u8> {
        match self {
            Self::Submit(tx) => tx.into(),
            Self::SubmitWithArgs(args) => to_borsh(args),
            Self::Call(args) => to_borsh(args),
            Self::PausePrecompiles(args) | Self::ResumePrecompiles(args) => to_borsh(args),
            Self::Deploy(bytes) | Self::Deposit(bytes) | Self::FactoryUpdate(bytes) => {
                bytes.clone()
            }
            Self::DeployErc20(args) => to_borsh(args),
            Self::DeployErc20Callback(args) => to_borsh(args),
            Self::FtOnTransfer(args) => to_json(args),
            Self::FtTransferCall(args) => to_json(args),
            Self::FinishDeposit(args) => to_borsh(args),
            Self::ResolveTransfer(args, _) => to_borsh(args),
            Self::FtTransfer(args) => to_json(args),
            Self::Withdraw(args) => to_borsh(args),
            Self::StorageDeposit(args) => to_json(args),
            Self::StorageUnregister(args) => to_json(args),
            Self::StorageWithdraw(args) => to_json(args),
            Self::SetOwner(args) => to_borsh(args),
            Self::SetUpgradeDelayBlocks(args) => to_borsh(args),
            Self::SetPausedFlags(args) => to_borsh(args),
            Self::RegisterRelayer(address) | Self::FactorySetWNearAddress(address) => {
                address.as_bytes().to_vec()
            }
            Self::ExitToNear(maybe_args) => maybe_args
                .as_ref()
                .and_then(|args| borsh::to_vec(&args).ok())
                .unwrap_or_default(),
            Self::NewConnector(args) | Self::SetConnectorData(args) => to_borsh(args),
            Self::NewEngine(args) => to_borsh(args),
            Self::FactoryUpdateAddressVersion(args) => to_borsh(args),
            Self::FundXccSubAccount(args) => to_borsh(args),
            Self::WithdrawWnearToRouter(args) => to_borsh(args),
            Self::PauseContract | Self::ResumeContract | Self::Unknown => Vec::new(),
            Self::SetKeyManager(args) => to_borsh(args),
            Self::AddRelayerKey(args)
            | Self::RemoveRelayerKey(args)
            | Self::StoreRelayerKeyCallback(args) => to_json(args),
            Self::StartHashchain(args) => to_borsh(args),
            Self::SetErc20Metadata(args) => to_json(args),
            Self::SetFixedGas(args) => to_borsh(args),
            Self::SetErc20FallbackAddress(args) => to_borsh(args),
            Self::SetSiloParams(args) => to_borsh(args),
            Self::AddEntryToWhitelist(args) | Self::RemoveEntryFromWhitelist(args) => {
                to_borsh(args)
            }
            Self::AddEntryToWhitelistBatch(args) => to_borsh(args),
            Self::SetWhitelistStatus(args) => to_borsh(args),
            Self::SetWhitelistsStatuses(args) => to_borsh(args),
            Self::SetEthConnectorContractAccount(args) => to_borsh(args),
            Self::MirrorErc20TokenCallback(args) => to_borsh(args),
        }
    }
}

fn to_borsh<T: BorshSerialize>(args: &T) -> Vec<u8> {
    borsh::to_vec(args).unwrap_or_default()
}

fn to_json<T: Serialize>(args: &T) -> Vec<u8> {
    serde_json::to_vec(args).unwrap_or_default()
}

/// Used to make sure `TransactionKindTag` is kept in sync with `TransactionKind`
impl From<&TransactionKind> for TransactionKindTag {
    fn from(tx: &TransactionKind) -> Self {
        match tx {
            TransactionKind::Submit(_) => Self::Submit,
            TransactionKind::Call(_) => Self::Call,
            TransactionKind::PausePrecompiles(_) => Self::PausePrecompiles,
            TransactionKind::ResumePrecompiles(_) => Self::ResumePrecompiles,
            TransactionKind::Deploy(_) => Self::Deploy,
            TransactionKind::DeployErc20(_) => Self::DeployErc20,
            TransactionKind::DeployErc20Callback(_) => Self::DeployErc20Callback,
            TransactionKind::FtOnTransfer(_) => Self::FtOnTransfer,
            TransactionKind::Deposit(_) => Self::Deposit,
            TransactionKind::FtTransferCall(_) => Self::FtTransferCall,
            TransactionKind::FinishDeposit(_) => Self::FinishDeposit,
            TransactionKind::ResolveTransfer(_, _) => Self::ResolveTransfer,
            TransactionKind::FtTransfer(_) => Self::FtTransfer,
            TransactionKind::Withdraw(_) => Self::Withdraw,
            TransactionKind::StorageDeposit(_) => Self::StorageDeposit,
            TransactionKind::StorageUnregister(_) => Self::StorageUnregister,
            TransactionKind::StorageWithdraw(_) => Self::StorageWithdraw,
            TransactionKind::SetPausedFlags(_) => Self::SetPausedFlags,
            TransactionKind::RegisterRelayer(_) => Self::RegisterRelayer,
            TransactionKind::ExitToNear(_) => Self::ExitToNear,
            TransactionKind::SetConnectorData(_) => Self::SetConnectorData,
            TransactionKind::NewConnector(_) => Self::NewConnector,
            TransactionKind::NewEngine(_) => Self::NewEngine,
            TransactionKind::FactoryUpdate(_) => Self::FactoryUpdate,
            TransactionKind::FactoryUpdateAddressVersion(_) => Self::FactoryUpdateAddressVersion,
            TransactionKind::FactorySetWNearAddress(_) => Self::FactorySetWNearAddress,
            TransactionKind::WithdrawWnearToRouter(_) => Self::WithdrawWnearToRouter,
            TransactionKind::SetOwner(_) => Self::SetOwner,
            TransactionKind::SubmitWithArgs(_) => Self::SubmitWithArgs,
            TransactionKind::SetUpgradeDelayBlocks(_) => Self::SetUpgradeDelayBlocks,
            TransactionKind::FundXccSubAccount(_) => Self::FundXccSubAccount,
            TransactionKind::PauseContract => Self::PauseContract,
            TransactionKind::ResumeContract => Self::ResumeContract,
            TransactionKind::SetKeyManager(_) => Self::SetKeyManager,
            TransactionKind::AddRelayerKey(_) => Self::AddRelayerKey,
            TransactionKind::StoreRelayerKeyCallback(_) => Self::StoreRelayerKeyCallback,
            TransactionKind::RemoveRelayerKey(_) => Self::RemoveRelayerKey,
            TransactionKind::StartHashchain(_) => Self::StartHashchain,
            TransactionKind::SetErc20Metadata(_) => Self::SetErc20Metadata,
            TransactionKind::SetEthConnectorContractAccount(_) => {
                Self::SetEthConnectorContractAccount
            }
            TransactionKind::SetFixedGas(_) => Self::SetFixedGas,
            TransactionKind::SetErc20FallbackAddress(_) => Self::SetErc20FallbackAddress,
            TransactionKind::SetSiloParams(_) => Self::SetSiloParams,
            TransactionKind::AddEntryToWhitelist(_) => Self::AddEntryToWhitelist,
            TransactionKind::AddEntryToWhitelistBatch(_) => Self::AddEntryToWhitelistBatch,
            TransactionKind::RemoveEntryFromWhitelist(_) => Self::RemoveEntryFromWhitelist,
            TransactionKind::SetWhitelistStatus(_) => Self::SetWhitelistStatus,
            TransactionKind::SetWhitelistsStatuses(_) => Self::SetWhitelistsStatuses,
            TransactionKind::Unknown => Self::Unknown,
            TransactionKind::MirrorErc20TokenCallback(_) => Self::MirrorErc20TokenCallback,
        }
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
    pub raw_input: Cow<'a, Vec<u8>>,
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
            raw_input: Cow::Borrowed(&t.raw_input),
        })
    }
}

impl<'a> TryFrom<BorshableTransactionMessage<'a>> for TransactionMessage {
    type Error = aurora_engine_transactions::Error;

    fn try_from(t: BorshableTransactionMessage<'a>) -> Result<Self, Self::Error> {
        match t {
            BorshableTransactionMessage::V1(t) => {
                let transaction: TransactionKind = t.transaction.try_into()?;
                let raw_input = transaction.raw_bytes();
                Ok(Self {
                    block_hash: H256(t.block_hash),
                    near_receipt_id: H256(t.near_receipt_id),
                    position: t.position,
                    succeeded: t.succeeded,
                    signer: t.signer.into_owned(),
                    caller: t.caller.into_owned(),
                    attached_near: t.attached_near,
                    transaction,
                    promise_data: Vec::new(),
                    raw_input,
                    action_hash: H256::default(),
                })
            }
            BorshableTransactionMessage::V2(t) => {
                let transaction: TransactionKind = t.transaction.try_into()?;
                let raw_input = transaction.raw_bytes();
                Ok(Self {
                    block_hash: H256(t.block_hash),
                    near_receipt_id: H256(t.near_receipt_id),
                    position: t.position,
                    succeeded: t.succeeded,
                    signer: t.signer.into_owned(),
                    caller: t.caller.into_owned(),
                    attached_near: t.attached_near,
                    transaction,
                    promise_data: t.promise_data.into_owned(),
                    raw_input,
                    action_hash: H256::default(),
                })
            }
            BorshableTransactionMessage::V3(t) => Ok(Self {
                block_hash: H256(t.block_hash),
                near_receipt_id: H256(t.near_receipt_id),
                position: t.position,
                succeeded: t.succeeded,
                signer: t.signer.into_owned(),
                caller: t.caller.into_owned(),
                attached_near: t.attached_near,
                transaction: t.transaction.try_into()?,
                promise_data: t.promise_data.into_owned(),
                raw_input: t.raw_input.into_owned(),
                action_hash: H256::default(),
            }),
            BorshableTransactionMessage::V4(t) => Ok(Self {
                block_hash: H256(t.block_hash),
                near_receipt_id: H256(t.near_receipt_id),
                position: t.position,
                succeeded: t.succeeded,
                signer: t.signer.into_owned(),
                caller: t.caller.into_owned(),
                attached_near: t.attached_near,
                transaction: t.transaction.try_into()?,
                promise_data: t.promise_data.into_owned(),
                raw_input: t.raw_input.into_owned(),
                action_hash: H256(t.action_hash),
            }),
        }
    }
}

/// Same as `TransactionKind`, but with `Submit` variant replaced with raw bytes
/// so that it can derive the Borsh traits. All non-copy elements are `Cow` also
/// so that this type can be cheaply created from a `TransactionKind` reference.
/// !!!!! New types of transactions must be added at the end of the enum. !!!!!!
#[derive(BorshDeserialize, BorshSerialize, Clone)]
#[borsh(crate = "aurora_engine_types::borsh")]
enum BorshableTransactionKind<'a> {
    Submit(Cow<'a, Vec<u8>>),
    Call(Cow<'a, engine::CallArgs>),
    Deploy(Cow<'a, Vec<u8>>),
    DeployErc20(Cow<'a, engine::DeployErc20TokenArgs>),
    FtOnTransfer(Cow<'a, connector::FtOnTransferArgs>),
    Deposit(Cow<'a, Vec<u8>>),
    FtTransferCall(Cow<'a, connector::FtTransferCallArgs>),
    FinishDeposit(Cow<'a, connector::FinishDepositArgs>),
    ResolveTransfer(
        Cow<'a, connector::FtResolveTransferArgs>,
        Cow<'a, types::PromiseResult>,
    ),
    FtTransfer(Cow<'a, connector::FtTransferArgs>),
    Withdraw(Cow<'a, connector::WithdrawCallArgs>),
    StorageDeposit(Cow<'a, connector::StorageDepositArgs>),
    StorageUnregister(Option<bool>),
    StorageWithdraw(Cow<'a, connector::StorageWithdrawArgs>),
    SetPausedFlags(Cow<'a, connector::PauseEthConnectorArgs>),
    RegisterRelayer(Cow<'a, Address>),
    ExitToNear(Cow<'a, Option<connector::ExitToNearPrecompileCallbackArgs>>),
    SetConnectorData(Cow<'a, connector::SetContractDataCallArgs>),
    NewConnector(Cow<'a, connector::InitCallArgs>),
    NewEngine(Cow<'a, engine::NewCallArgs>),
    FactoryUpdate(Cow<'a, Vec<u8>>),
    FactoryUpdateAddressVersion(Cow<'a, AddressVersionUpdateArgs>),
    FactorySetWNearAddress(Address),
    PausePrecompiles(Cow<'a, engine::PausePrecompilesCallArgs>),
    ResumePrecompiles(Cow<'a, engine::PausePrecompilesCallArgs>),
    Unknown,
    SetOwner(Cow<'a, engine::SetOwnerArgs>),
    SubmitWithArgs(Cow<'a, engine::SubmitArgs>),
    FundXccSubAccount(Cow<'a, FundXccArgs>),
    SetUpgradeDelayBlocks(Cow<'a, engine::SetUpgradeDelayBlocksArgs>),
    PauseContract,
    ResumeContract,
    SetKeyManager(Cow<'a, engine::RelayerKeyManagerArgs>),
    AddRelayerKey(Cow<'a, engine::RelayerKeyArgs>),
    RemoveRelayerKey(Cow<'a, engine::RelayerKeyArgs>),
    StartHashchain(Cow<'a, engine::StartHashchainArgs>),
    SetErc20Metadata(Cow<'a, connector::SetErc20MetadataArgs>),
    SetFixedGas(Cow<'a, silo::FixedGasArgs>),
    SetSiloParams(Cow<'a, Option<silo::SiloParamsArgs>>),
    AddEntryToWhitelist(Cow<'a, silo::WhitelistArgs>),
    AddEntryToWhitelistBatch(Cow<'a, Vec<silo::WhitelistArgs>>),
    RemoveEntryFromWhitelist(Cow<'a, silo::WhitelistArgs>),
    SetWhitelistStatus(Cow<'a, silo::WhitelistStatusArgs>),
    SetEthConnectorContractAccount(Cow<'a, connector::SetEthConnectorContractAccountArgs>),
    MirrorErc20TokenCallback(Cow<'a, connector::MirrorErc20TokenArgs>),
    WithdrawWnearToRouter(Cow<'a, WithdrawWnearToRouterArgs>),
    StoreRelayerKeyCallback(Cow<'a, engine::RelayerKeyArgs>),
    SetWhitelistsStatuses(Cow<'a, Vec<silo::WhitelistStatusArgs>>),
    SetErc20FallbackAddress(Cow<'a, silo::Erc20FallbackAddressArgs>),
    DeployErc20Callback(Cow<'a, AccountId>),
}

impl<'a> From<&'a TransactionKind> for BorshableTransactionKind<'a> {
    fn from(t: &'a TransactionKind) -> Self {
        match t {
            TransactionKind::Submit(eth_tx) => {
                let tx_bytes = eth_tx.into();
                Self::Submit(Cow::Owned(tx_bytes))
            }
            TransactionKind::SubmitWithArgs(x) => Self::SubmitWithArgs(Cow::Borrowed(x)),
            TransactionKind::Call(x) => Self::Call(Cow::Borrowed(x)),
            TransactionKind::Deploy(x) => Self::Deploy(Cow::Borrowed(x)),
            TransactionKind::DeployErc20(x) => Self::DeployErc20(Cow::Borrowed(x)),
            TransactionKind::DeployErc20Callback(x) => Self::DeployErc20Callback(Cow::Borrowed(x)),
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
            TransactionKind::ExitToNear(x) => Self::ExitToNear(Cow::Borrowed(x)),
            TransactionKind::SetConnectorData(x) => Self::SetConnectorData(Cow::Borrowed(x)),
            TransactionKind::NewConnector(x) => Self::NewConnector(Cow::Borrowed(x)),
            TransactionKind::NewEngine(x) => Self::NewEngine(Cow::Borrowed(x)),
            TransactionKind::FactoryUpdate(x) => Self::FactoryUpdate(Cow::Borrowed(x)),
            TransactionKind::FactoryUpdateAddressVersion(x) => {
                Self::FactoryUpdateAddressVersion(Cow::Borrowed(x))
            }
            TransactionKind::FactorySetWNearAddress(address) => {
                Self::FactorySetWNearAddress(*address)
            }
            TransactionKind::WithdrawWnearToRouter(x) => {
                Self::WithdrawWnearToRouter(Cow::Borrowed(x))
            }
            TransactionKind::Unknown => Self::Unknown,
            TransactionKind::PausePrecompiles(x) => Self::PausePrecompiles(Cow::Borrowed(x)),
            TransactionKind::ResumePrecompiles(x) => Self::ResumePrecompiles(Cow::Borrowed(x)),
            TransactionKind::SetEthConnectorContractAccount(x) => {
                Self::SetEthConnectorContractAccount(Cow::Borrowed(x))
            }
            TransactionKind::SetOwner(x) => Self::SetOwner(Cow::Borrowed(x)),
            TransactionKind::FundXccSubAccount(x) => Self::FundXccSubAccount(Cow::Borrowed(x)),
            TransactionKind::SetUpgradeDelayBlocks(x) => {
                Self::SetUpgradeDelayBlocks(Cow::Borrowed(x))
            }
            TransactionKind::PauseContract => Self::PauseContract,
            TransactionKind::ResumeContract => Self::ResumeContract,
            TransactionKind::SetKeyManager(x) => Self::SetKeyManager(Cow::Borrowed(x)),
            TransactionKind::AddRelayerKey(x) => Self::AddRelayerKey(Cow::Borrowed(x)),
            TransactionKind::StoreRelayerKeyCallback(x) => {
                Self::StoreRelayerKeyCallback(Cow::Borrowed(x))
            }
            TransactionKind::RemoveRelayerKey(x) => Self::RemoveRelayerKey(Cow::Borrowed(x)),
            TransactionKind::StartHashchain(x) => Self::StartHashchain(Cow::Borrowed(x)),
            TransactionKind::SetErc20Metadata(x) => Self::SetErc20Metadata(Cow::Borrowed(x)),
            TransactionKind::SetFixedGas(x) => Self::SetFixedGas(Cow::Borrowed(x)),
            TransactionKind::SetErc20FallbackAddress(x) => {
                Self::SetErc20FallbackAddress(Cow::Borrowed(x))
            }
            TransactionKind::SetSiloParams(x) => Self::SetSiloParams(Cow::Borrowed(x)),
            TransactionKind::AddEntryToWhitelist(x) => Self::AddEntryToWhitelist(Cow::Borrowed(x)),
            TransactionKind::AddEntryToWhitelistBatch(x) => {
                Self::AddEntryToWhitelistBatch(Cow::Borrowed(x))
            }
            TransactionKind::RemoveEntryFromWhitelist(x) => {
                Self::RemoveEntryFromWhitelist(Cow::Borrowed(x))
            }
            TransactionKind::SetWhitelistStatus(x) => Self::SetWhitelistStatus(Cow::Borrowed(x)),
            TransactionKind::MirrorErc20TokenCallback(x) => {
                Self::MirrorErc20TokenCallback(Cow::Borrowed(x))
            }
            TransactionKind::SetWhitelistsStatuses(x) => {
                Self::SetWhitelistsStatuses(Cow::Borrowed(x))
            }
        }
    }
}

impl<'a> TryFrom<BorshableTransactionKind<'a>> for TransactionKind {
    type Error = aurora_engine_transactions::Error;

    #[allow(clippy::too_many_lines)]
    fn try_from(t: BorshableTransactionKind<'a>) -> Result<Self, Self::Error> {
        match t {
            BorshableTransactionKind::Submit(tx_bytes) => {
                // `BorshableTransactionKind` is an internal type, so we will
                // assume the conversion is infallible. If the conversion were to
                // fail then something has gone very wrong.
                let eth_tx = tx_bytes.as_slice().try_into()?;
                Ok(Self::Submit(eth_tx))
            }
            BorshableTransactionKind::SubmitWithArgs(x) => Ok(Self::SubmitWithArgs(x.into_owned())),
            BorshableTransactionKind::Call(x) => Ok(Self::Call(x.into_owned())),
            BorshableTransactionKind::Deploy(x) => Ok(Self::Deploy(x.into_owned())),
            BorshableTransactionKind::DeployErc20(x) => Ok(Self::DeployErc20(x.into_owned())),
            BorshableTransactionKind::DeployErc20Callback(x) => {
                Ok(Self::DeployErc20Callback(x.into_owned()))
            }
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
            BorshableTransactionKind::ExitToNear(x) => Ok(Self::ExitToNear(x.into_owned())),
            BorshableTransactionKind::SetConnectorData(x) => {
                Ok(Self::SetConnectorData(x.into_owned()))
            }
            BorshableTransactionKind::NewConnector(x) => Ok(Self::NewConnector(x.into_owned())),
            BorshableTransactionKind::NewEngine(x) => Ok(Self::NewEngine(x.into_owned())),
            BorshableTransactionKind::FactoryUpdate(x) => Ok(Self::FactoryUpdate(x.into_owned())),
            BorshableTransactionKind::FactoryUpdateAddressVersion(x) => {
                Ok(Self::FactoryUpdateAddressVersion(x.into_owned()))
            }
            BorshableTransactionKind::FactorySetWNearAddress(address) => {
                Ok(Self::FactorySetWNearAddress(address))
            }
            BorshableTransactionKind::Unknown => Ok(Self::Unknown),
            BorshableTransactionKind::PausePrecompiles(x) => {
                Ok(Self::PausePrecompiles(x.into_owned()))
            }
            BorshableTransactionKind::ResumePrecompiles(x) => {
                Ok(Self::ResumePrecompiles(x.into_owned()))
            }
            BorshableTransactionKind::SetEthConnectorContractAccount(x) => {
                Ok(Self::SetEthConnectorContractAccount(x.into_owned()))
            }
            BorshableTransactionKind::SetOwner(x) => Ok(Self::SetOwner(x.into_owned())),
            BorshableTransactionKind::FundXccSubAccount(x) => {
                Ok(Self::FundXccSubAccount(x.into_owned()))
            }
            BorshableTransactionKind::SetUpgradeDelayBlocks(x) => {
                Ok(Self::SetUpgradeDelayBlocks(x.into_owned()))
            }
            BorshableTransactionKind::PauseContract => Ok(Self::PauseContract),
            BorshableTransactionKind::ResumeContract => Ok(Self::ResumeContract),
            BorshableTransactionKind::SetKeyManager(x) => Ok(Self::SetKeyManager(x.into_owned())),
            BorshableTransactionKind::AddRelayerKey(x) => Ok(Self::AddRelayerKey(x.into_owned())),
            BorshableTransactionKind::StoreRelayerKeyCallback(x) => {
                Ok(Self::StoreRelayerKeyCallback(x.into_owned()))
            }
            BorshableTransactionKind::RemoveRelayerKey(x) => {
                Ok(Self::RemoveRelayerKey(x.into_owned()))
            }
            BorshableTransactionKind::StartHashchain(x) => Ok(Self::StartHashchain(x.into_owned())),
            BorshableTransactionKind::SetErc20Metadata(x) => {
                Ok(Self::SetErc20Metadata(x.into_owned()))
            }
            BorshableTransactionKind::SetFixedGas(x) => Ok(Self::SetFixedGas(x.into_owned())),
            BorshableTransactionKind::SetSiloParams(x) => Ok(Self::SetSiloParams(x.into_owned())),
            BorshableTransactionKind::AddEntryToWhitelist(x) => {
                Ok(Self::AddEntryToWhitelist(x.into_owned()))
            }
            BorshableTransactionKind::AddEntryToWhitelistBatch(x) => {
                Ok(Self::AddEntryToWhitelistBatch(x.into_owned()))
            }
            BorshableTransactionKind::RemoveEntryFromWhitelist(x) => {
                Ok(Self::RemoveEntryFromWhitelist(x.into_owned()))
            }
            BorshableTransactionKind::SetWhitelistStatus(x) => {
                Ok(Self::SetWhitelistStatus(x.into_owned()))
            }
            BorshableTransactionKind::MirrorErc20TokenCallback(x) => {
                Ok(Self::MirrorErc20TokenCallback(x.into_owned()))
            }
            BorshableTransactionKind::WithdrawWnearToRouter(x) => {
                Ok(Self::WithdrawWnearToRouter(x.into_owned()))
            }
            BorshableTransactionKind::SetWhitelistsStatuses(x) => {
                Ok(Self::SetWhitelistsStatuses(x.into_owned()))
            }
            BorshableTransactionKind::SetErc20FallbackAddress(x) => {
                Ok(Self::SetErc20FallbackAddress(x.into_owned()))
            }
        }
    }
}
