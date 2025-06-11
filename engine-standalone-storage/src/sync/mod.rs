use crate::{
    native_ffi::{self, DynamicContractImpl},
    state,
};

use aurora_engine::parameters::SubmitResult;
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::{
    env::{self, DEFAULT_PREPAID_GAS},
    io::IO,
};
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::parameters::{connector, engine, PromiseOrValue};
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::BorshDeserialize,
    parameters::{silo as silo_params, xcc, PromiseWithCallbackArgs},
    types::Address,
    H256,
};
use std::ops::Deref;
use std::{io, str::FromStr};

pub mod types;

use crate::{error::ParseTransactionKindError, BlockMetadata, Diff, Storage};
use types::{Message, TransactionKind, TransactionKindTag, TransactionMessage};

/// Try to parse an Aurora transaction from raw information available in a Near action
/// (method name, input bytes, data returned from promises).
#[allow(clippy::too_many_lines)]
pub fn parse_transaction_kind(
    method_name: &str,
    bytes: Vec<u8>,
    promise_data: &[Option<Vec<u8>>],
) -> Result<TransactionKind, ParseTransactionKindError> {
    let tx_kind_tag = TransactionKindTag::from_str(method_name).map_err(|_| {
        ParseTransactionKindError::UnknownMethodName {
            name: method_name.into(),
        }
    })?;
    let f = |e: io::Error| ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e));

    let tx_kind = match tx_kind_tag {
        TransactionKindTag::Submit => {
            let eth_tx = EthTransactionKind::try_from(bytes.as_slice()).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::Submit(eth_tx)
        }
        TransactionKindTag::SubmitWithArgs => {
            let args = engine::SubmitArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SubmitWithArgs(args)
        }
        TransactionKindTag::Call => {
            let call_args = engine::CallArgs::deserialize(&bytes).ok_or_else(|| {
                ParseTransactionKindError::failed_deserialization::<io::Error>(tx_kind_tag, None)
            })?;
            TransactionKind::Call(call_args)
        }
        TransactionKindTag::PausePrecompiles => {
            let args = engine::PausePrecompilesCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::PausePrecompiles(args)
        }
        TransactionKindTag::ResumePrecompiles => {
            let args = engine::PausePrecompilesCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::ResumePrecompiles(args)
        }
        TransactionKindTag::SetOwner => {
            let args = engine::SetOwnerArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetOwner(args)
        }
        TransactionKindTag::Deploy => TransactionKind::Deploy(bytes),
        TransactionKindTag::DeployErc20 => {
            let deploy_args = engine::DeployErc20TokenArgs::deserialize(&bytes).map_err(f)?;
            TransactionKind::DeployErc20(deploy_args)
        }
        TransactionKindTag::DeployErc20Callback => {
            let args = AccountId::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::DeployErc20Callback(args)
        }
        TransactionKindTag::FtOnTransfer => {
            let transfer_args: connector::FtOnTransferArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtOnTransfer(transfer_args)
        }
        TransactionKindTag::Deposit => TransactionKind::Deposit(bytes),
        TransactionKindTag::FtTransferCall => {
            let transfer_args: connector::FtTransferCallArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtTransferCall(transfer_args)
        }
        TransactionKindTag::FinishDeposit => {
            let args = connector::FinishDepositArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FinishDeposit(args)
        }
        TransactionKindTag::ResolveTransfer => {
            let args = connector::FtResolveTransferArgs::try_from_slice(&bytes).map_err(f)?;
            let promise_result = promise_data
                .first()
                .and_then(Option::as_ref)
                .map_or(aurora_engine_types::types::PromiseResult::Failed, |bytes| {
                    aurora_engine_types::types::PromiseResult::Successful(bytes.clone())
                });
            TransactionKind::ResolveTransfer(args, promise_result)
        }
        TransactionKindTag::FtTransfer => {
            let args: connector::FtTransferArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtTransfer(args)
        }
        TransactionKindTag::Withdraw => {
            let args = connector::WithdrawCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::Withdraw(args)
        }
        TransactionKindTag::StorageDeposit => {
            let args: connector::StorageDepositArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::StorageDeposit(args)
        }
        TransactionKindTag::StorageUnregister => {
            let json_args: serde_json::Value =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            let force = json_args
                .as_object()
                .and_then(|x| x.get("force"))
                .and_then(serde_json::Value::as_bool);

            TransactionKind::StorageUnregister(force)
        }
        TransactionKindTag::StorageWithdraw => {
            let args: connector::StorageWithdrawArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::StorageWithdraw(args)
        }
        TransactionKindTag::SetPausedFlags => {
            let args = connector::PauseEthConnectorArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetPausedFlags(args)
        }
        TransactionKindTag::RegisterRelayer => {
            let address = Address::try_from_slice(&bytes).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::RegisterRelayer(address)
        }
        TransactionKindTag::ExitToNear => {
            if promise_data.first().and_then(Option::as_ref).is_none() {
                TransactionKind::ExitToNear(None)
            } else {
                let args = connector::ExitToNearPrecompileCallbackArgs::try_from_slice(&bytes)
                    .map_err(f)?;
                TransactionKind::ExitToNear(Some(args))
            }
        }
        TransactionKindTag::SetConnectorData => {
            let args = connector::SetContractDataCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetConnectorData(args)
        }
        TransactionKindTag::NewConnector => {
            let args = connector::InitCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::NewConnector(args)
        }
        TransactionKindTag::NewEngine => {
            let args = engine::NewCallArgs::deserialize(&bytes).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::NewEngine(args)
        }
        TransactionKindTag::FactoryUpdate => TransactionKind::FactoryUpdate(bytes),
        TransactionKindTag::FactoryUpdateAddressVersion => {
            let args = xcc::AddressVersionUpdateArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FactoryUpdateAddressVersion(args)
        }
        TransactionKindTag::FactorySetWNearAddress => {
            let address = Address::try_from_slice(&bytes).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::FactorySetWNearAddress(address)
        }
        TransactionKindTag::WithdrawWnearToRouter => {
            let args = xcc::WithdrawWnearToRouterArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::WithdrawWnearToRouter(args)
        }
        TransactionKindTag::SetUpgradeDelayBlocks => {
            let args = engine::SetUpgradeDelayBlocksArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetUpgradeDelayBlocks(args)
        }
        TransactionKindTag::FundXccSubAccount => {
            let args = xcc::FundXccArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FundXccSubAccount(args)
        }
        TransactionKindTag::PauseContract => TransactionKind::PauseContract,
        TransactionKindTag::ResumeContract => TransactionKind::ResumeContract,
        TransactionKindTag::SetKeyManager => {
            let args: engine::RelayerKeyManagerArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            TransactionKind::SetKeyManager(args)
        }
        TransactionKindTag::AddRelayerKey => {
            let args = engine::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddRelayerKey(args)
        }
        TransactionKindTag::StoreRelayerKeyCallback => {
            let args = engine::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::StoreRelayerKeyCallback(args)
        }
        TransactionKindTag::RemoveRelayerKey => {
            let args = engine::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::RemoveRelayerKey(args)
        }
        TransactionKindTag::StartHashchain => {
            let args = engine::StartHashchainArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::StartHashchain(args)
        }
        TransactionKindTag::SetErc20Metadata => {
            let args: connector::SetErc20MetadataArgs =
                serde_json::from_slice(&bytes).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            TransactionKind::SetErc20Metadata(args)
        }
        TransactionKindTag::SetFixedGas => {
            let args = silo_params::FixedGasArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetFixedGas(args)
        }
        TransactionKindTag::SetErc20FallbackAddress => {
            let args = silo_params::Erc20FallbackAddressArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetErc20FallbackAddress(args)
        }
        TransactionKindTag::SetSiloParams => {
            let args: Option<silo_params::SiloParamsArgs> =
                BorshDeserialize::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetSiloParams(args)
        }
        TransactionKindTag::SetWhitelistStatus => {
            let args = silo_params::WhitelistStatusArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetWhitelistStatus(args)
        }
        TransactionKindTag::SetWhitelistsStatuses => {
            let args: Vec<silo_params::WhitelistStatusArgs> =
                BorshDeserialize::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetWhitelistsStatuses(args)
        }
        TransactionKindTag::AddEntryToWhitelist => {
            let args = silo_params::WhitelistArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddEntryToWhitelist(args)
        }
        TransactionKindTag::AddEntryToWhitelistBatch => {
            let args: Vec<silo_params::WhitelistArgs> =
                BorshDeserialize::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddEntryToWhitelistBatch(args)
        }
        TransactionKindTag::RemoveEntryFromWhitelist => {
            let args = silo_params::WhitelistArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::RemoveEntryFromWhitelist(args)
        }
        TransactionKindTag::SetEthConnectorContractAccount => {
            let args =
                connector::SetEthConnectorContractAccountArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetEthConnectorContractAccount(args)
        }
        TransactionKindTag::MirrorErc20TokenCallback => {
            let args = connector::MirrorErc20TokenArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::MirrorErc20TokenCallback(args)
        }
        TransactionKindTag::Unknown => {
            return Err(ParseTransactionKindError::UnknownMethodName {
                name: method_name.into(),
            });
        }
    };
    Ok(tx_kind)
}

/// Note: this function does not automatically commit transaction messages to the storage.
/// If you want the transaction diff committed then you must call the `commit` method on
/// the outcome of this function.
pub fn consume_message<M: ModExpAlgorithm + 'static>(
    storage: &mut Storage,
    message: Message,
) -> Result<ConsumeMessageOutcome, crate::Error> {
    match message {
        Message::Block(block_message) => {
            let block_hash = block_message.hash;
            let block_height = block_message.height;
            let block_metadata = block_message.metadata;
            storage
                .set_block_data(block_hash, block_height, &block_metadata)
                .map_err(crate::Error::Rocksdb)?;
            Ok(ConsumeMessageOutcome::BlockAdded)
        }

        Message::Transaction(transaction_message) => {
            // Failed transactions have no impact on the state of our database.
            if !transaction_message.succeeded {
                return Ok(ConsumeMessageOutcome::FailedTransactionIgnored);
            }

            let transaction_position = transaction_message.position;
            let block_hash = transaction_message.block_hash;
            let block_height = storage.get_block_height_by_hash(block_hash)?;
            let block_metadata = storage.get_block_metadata(block_hash)?;
            let engine_account_id = storage.get_engine_account_id()?;

            let (tx_hash, diff, result) = storage
                .with_engine_access(
                    block_height,
                    transaction_position,
                    &transaction_message.raw_input,
                    |io| {
                        execute_transaction::<_, M, _>(
                            transaction_message.as_ref(),
                            block_height,
                            &block_metadata,
                            engine_account_id,
                            io,
                            |s| s.get_transaction_diff(),
                        )
                    },
                )
                .result;
            let outcome = TransactionIncludedOutcome {
                hash: tx_hash,
                info: *transaction_message,
                diff,
                maybe_result: result,
            };
            Ok(ConsumeMessageOutcome::TransactionIncluded(Box::new(
                outcome,
            )))
        }
    }
}

pub fn execute_transaction_message<M: ModExpAlgorithm + 'static>(
    storage: &Storage,
    transaction_message: TransactionMessage,
) -> Result<TransactionIncludedOutcome, crate::Error> {
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let engine_account_id = storage.get_engine_account_id()?;
    let result = storage.with_engine_access(
        block_height,
        transaction_position,
        &transaction_message.raw_input,
        |io| {
            execute_transaction::<_, M, _>(
                &transaction_message,
                block_height,
                &block_metadata,
                engine_account_id,
                io,
                |s| s.get_transaction_diff(),
            )
        },
    );
    let (tx_hash, diff, maybe_result) = result.result;
    let outcome = TransactionIncludedOutcome {
        hash: tx_hash,
        info: transaction_message,
        diff,
        maybe_result,
    };
    Ok(outcome)
}

pub fn execute_transaction<I, M, F>(
    transaction_message: &TransactionMessage,
    block_height: u64,
    block_metadata: &BlockMetadata,
    engine_account_id: AccountId,
    io: I,
    get_diff: F,
) -> (
    H256,
    Diff,
    Result<Option<TransactionExecutionResult>, error::Error>,
)
where
    I: IO + Copy,
    M: ModExpAlgorithm + 'static,
    F: FnOnce(&I) -> Diff,
{
    let signer_account_id = transaction_message.signer.clone();
    let predecessor_account_id = transaction_message.caller.clone();
    let near_receipt_id = transaction_message.near_receipt_id;
    let current_account_id = engine_account_id;
    let random_seed = compute_random_seed(
        &transaction_message.action_hash,
        &block_metadata.random_seed,
    );
    let env = env::Fixed {
        signer_account_id,
        current_account_id,
        predecessor_account_id,
        block_height,
        block_timestamp: block_metadata.timestamp,
        attached_deposit: transaction_message.attached_near,
        random_seed,
        prepaid_gas: DEFAULT_PREPAID_GAS,
        used_gas: NearGas::new(0),
    };

    state::STATE.with_borrow(|state| {
        state.set_env(env);
        // We can ignore promises in the standalone engine because it processes each receipt
        // separately, and it is fed a stream of receipts (it does not schedule them)
        state.set_promise_handler(transaction_message.promise_data.clone().into_boxed_slice());
        state.store_dbg_info((&transaction_message.transaction).into());
    });

    let contract_lock = native_ffi::lock();
    let (tx_hash, result) = match &transaction_message.transaction {
        TransactionKind::Submit(tx) => {
            let tx_data: Vec<u8> = tx.into();
            let tx_hash = aurora_engine_sdk::keccak(&tx_data);
            let result = contract_lock
                .submit()
                .map(|submit_result| Some(TransactionExecutionResult::Submit(Ok(submit_result))))
                .map_err(Into::into);

            (tx_hash, result)
        }
        TransactionKind::SubmitWithArgs(args) => {
            let tx_hash = aurora_engine_sdk::keccak(&args.tx_data);
            let result = contract_lock
                .submit_with_args()
                .map(|submit_result| Some(TransactionExecutionResult::Submit(Ok(submit_result))))
                .map_err(Into::into);

            (tx_hash, result)
        }
        other => {
            let result = non_submit_execute(other, contract_lock);
            (near_receipt_id, result)
        }
    };

    let diff = get_diff(&io);

    (tx_hash, diff, result)
}

/// Based on nearcore implementation:
/// <https://github.com/near/nearcore/blob/00ca2f3f73e2a547ba881f76ecc59450dbbef6e2/core/primitives/src/utils.rs#L295>
fn compute_random_seed(action_hash: &H256, block_random_value: &H256) -> H256 {
    const BYTES_LEN: usize = 32 + 32;
    let mut bytes: Vec<u8> = Vec::with_capacity(BYTES_LEN);
    bytes.extend_from_slice(action_hash.as_bytes());
    bytes.extend_from_slice(block_random_value.as_bytes());
    aurora_engine_sdk::sha256(&bytes)
}

/// Handles all transaction kinds other than `submit`.
/// The `submit` transaction kind is special because it is the only one where the transaction hash
/// differs from the NEAR receipt hash.
#[allow(
    clippy::too_many_lines,
    clippy::match_same_arms,
    clippy::cognitive_complexity
)]
fn non_submit_execute(
    transaction: &TransactionKind,
    contract_lock: impl Deref<Target = DynamicContractImpl>,
) -> Result<Option<TransactionExecutionResult>, error::Error> {
    let result = match transaction {
        TransactionKind::Call(_) => {
            let result = contract_lock.call()?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }

        TransactionKind::Deploy(_) => {
            let result = contract_lock.deploy_code()?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::DeployErc20(_) => {
            let result = contract_lock.deploy_erc20_token()?;

            Some(match result {
                PromiseOrValue::Value(address) => TransactionExecutionResult::DeployErc20(address),
                PromiseOrValue::Promise(promise_args) => {
                    TransactionExecutionResult::Promise(promise_args)
                }
            })
        }
        TransactionKind::DeployErc20Callback(_) => {
            // No promises can be created by `deploy_erc20_token_callback`
            let result = contract_lock.deploy_erc20_token_callback()?;

            Some(TransactionExecutionResult::DeployErc20(result))
        }
        TransactionKind::FtOnTransfer(_) => {
            let maybe_output = contract_lock.ft_on_transfer()?;

            maybe_output.map(|result| TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::FtTransferCall(_) => None,
        TransactionKind::ResolveTransfer(_, _) => None,
        TransactionKind::FtTransfer(_) => None,
        TransactionKind::Withdraw(_) => None,
        TransactionKind::Deposit(_) => None,
        TransactionKind::FinishDeposit(_) => None,
        TransactionKind::StorageDeposit(_) => None,
        TransactionKind::StorageUnregister(_) => None,
        TransactionKind::StorageWithdraw(_) => None,
        TransactionKind::SetPausedFlags(_) => None,
        TransactionKind::RegisterRelayer(_) => {
            contract_lock.register_relayer()?;
            None
        }
        TransactionKind::ExitToNear(_) => {
            let maybe_result = contract_lock.exit_to_near_precompile_callback()?;

            maybe_result.map(|submit_result| TransactionExecutionResult::Submit(Ok(submit_result)))
        }
        TransactionKind::SetConnectorData(_) => None,
        TransactionKind::NewConnector(_) => None,
        TransactionKind::NewEngine(_) => {
            contract_lock.new_engine()?;
            None
        }
        TransactionKind::SetEthConnectorContractAccount(_) => {
            contract_lock.set_eth_connector_contract_account()?;

            None
        }
        TransactionKind::FactoryUpdate(_) => {
            contract_lock.factory_update()?;

            None
        }
        TransactionKind::FactoryUpdateAddressVersion(_) => {
            contract_lock.factory_update_address_version()?;

            None
        }
        TransactionKind::FactorySetWNearAddress(_) => {
            contract_lock.factory_set_wnear_address()?;

            None
        }
        TransactionKind::FundXccSubAccount(_) => {
            contract_lock.fund_xcc_sub_account()?;

            None
        }
        TransactionKind::WithdrawWnearToRouter(_) => {
            let result = contract_lock.withdraw_wnear_to_router()?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::Unknown => None,
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKind::Submit(_) | TransactionKind::SubmitWithArgs(_) => unreachable!(),
        TransactionKind::PausePrecompiles(_) => {
            contract_lock.pause_precompiles()?;

            None
        }
        TransactionKind::ResumePrecompiles(_) => {
            contract_lock.resume_precompiles()?;

            None
        }
        TransactionKind::SetOwner(_) => {
            contract_lock.set_owner()?;

            None
        }
        TransactionKind::SetUpgradeDelayBlocks(_) => {
            contract_lock.set_upgrade_delay_blocks()?;

            None
        }
        TransactionKind::PauseContract => {
            contract_lock.pause_contract()?;

            None
        }
        TransactionKind::ResumeContract => {
            contract_lock.resume_contract()?;

            None
        }
        TransactionKind::SetKeyManager(_) => {
            contract_lock.set_key_manager()?;

            None
        }
        TransactionKind::AddRelayerKey(_) => {
            contract_lock.add_relayer_key()?;

            None
        }
        TransactionKind::StoreRelayerKeyCallback(_) => {
            contract_lock.store_relayer_key_callback()?;

            None
        }
        TransactionKind::RemoveRelayerKey(_) => {
            contract_lock.remove_relayer_key()?;

            None
        }
        TransactionKind::StartHashchain(_) => {
            contract_lock.start_hashchain()?;

            None
        }
        TransactionKind::SetErc20Metadata(_) => {
            contract_lock.set_erc20_metadata()?;

            None
        }
        TransactionKind::SetFixedGas(args) => {
            contract_lock.silo_set_fixed_gas(args.fixed_gas);

            None
        }
        TransactionKind::SetErc20FallbackAddress(args) => {
            contract_lock.silo_set_erc20_fallback_address(args.address);

            None
        }
        TransactionKind::SetSiloParams(args) => {
            contract_lock.silo_set_silo_params(args.clone());

            None
        }
        TransactionKind::AddEntryToWhitelist(args) => {
            contract_lock.silo_add_entry_to_whitelist(args.clone());

            None
        }
        TransactionKind::AddEntryToWhitelistBatch(args) => {
            contract_lock.silo_add_entry_to_whitelist_batch(args.clone());

            None
        }
        TransactionKind::RemoveEntryFromWhitelist(args) => {
            contract_lock.silo_remove_entry_from_whitelist(args.clone());

            None
        }
        TransactionKind::SetWhitelistStatus(args) => {
            contract_lock.silo_set_whitelist_status(args.clone());

            None
        }
        TransactionKind::SetWhitelistsStatuses(args) => {
            contract_lock.silo_set_whitelists_statuses(args.clone());

            None
        }
        TransactionKind::MirrorErc20TokenCallback(_) => {
            contract_lock.mirror_erc20_token_callback()?;

            None
        }
    };

    Ok(result)
}

#[derive(Debug)]
pub enum ConsumeMessageOutcome {
    BlockAdded,
    FailedTransactionIgnored,
    TransactionIncluded(Box<TransactionIncludedOutcome>),
}

impl ConsumeMessageOutcome {
    pub fn commit(&self, storage: &mut Storage) -> Result<(), crate::error::Error> {
        if let Self::TransactionIncluded(x) = self {
            x.commit(storage)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TransactionIncludedOutcome {
    pub hash: H256,
    pub info: TransactionMessage,
    pub diff: Diff,
    pub maybe_result: Result<Option<TransactionExecutionResult>, error::Error>,
}

impl TransactionIncludedOutcome {
    pub fn commit(&self, storage: &mut Storage) -> Result<(), crate::error::Error> {
        match self.maybe_result.as_ref() {
            Err(_) | Ok(Some(TransactionExecutionResult::Submit(Err(_)))) => (), // do not persist if Engine encounters an error
            _ => storage.set_transaction_included(self.hash, &self.info, &self.diff)?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionExecutionResult {
    Submit(aurora_engine::engine::EngineResult<SubmitResult>),
    DeployErc20(Address),
    Promise(PromiseWithCallbackArgs),
}

pub mod error {
    use aurora_engine::{contract_methods, engine};

    #[derive(Debug)]
    pub enum Error {
        Engine(engine::EngineError),
        ContractError(contract_methods::ContractError),
    }

    impl From<engine::EngineError> for Error {
        fn from(e: engine::EngineError) -> Self {
            Self::Engine(e)
        }
    }

    impl From<contract_methods::ContractError> for Error {
        fn from(e: contract_methods::ContractError) -> Self {
            Self::ContractError(e)
        }
    }
}
