use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::{FungibleTokenMetadata, WithdrawResult};
use aurora_engine_types::parameters::engine::{StorageBalance, SubmitResult, TransactionStatus};
use aurora_engine_types::parameters::silo::{
    FixedGasCostArgs, SiloParamsArgs, WhitelistStatusArgs,
};
use aurora_engine_types::types::Address;
use aurora_engine_types::{H256, U256};
use near_sdk::json_types::U128;
use near_sdk::PromiseOrValue;

use crate::contract::RawContract;
use crate::result::{ExecutionResult, ViewResult};
use crate::transaction::{CallTransaction, ViewTransaction};
use crate::{impl_call_return, impl_view_return};

impl_call_return![
    (CallNew, Call::New),
    (CallNewEthConnector, Call::NewEthConnector),
    (CallFtTransfer, Call::FtTransfer),
    (CallDeposit, Call::Deposit),
    (
        CallSetEthConnectorContractData,
        Call::SetEthConnectorContractData
    ),
    (
        CallFactoryUpdateAddressVersion,
        Call::FactoryUpdateAddressVersion
    ),
    (CallRegisterRelayer, Call::RegisterRelayer),
    (CallRefundOnError, Call::RefundOnError),
    (CallFactoryUpdate, Call::FactoryUpdate),
    (CallFundXccSubAccount, Call::FundXccSubAccount),
    (CallFactorySetWNearAddress, Call::FactorySetWNearAddress),
    (CallDeployUpgrade, Call::DeployUpgrade),
    (CallResumePrecompiles, Call::ResumePrecompiles),
    (CallPausePrecompiles, Call::PausePrecompiles),
    (CallStageUpgrade, Call::StageUpgrade),
    (CallStateMigration, Call::StateMigration),
    (CallMintAccount, Call::MintAccount),
    (CallSetPausedFlags, Call::SetPausedFlags),
    (CallSetKeyManager, Call::SetKeyManager),
    (CallAddRelayerKey, Call::AddRelayerKey),
    (CallRemoveRelayerKey, Call::RemoveRelayerKey),
    (
        CallSetEthConnectorContractAccount,
        Call::SetEthConnectorContractAccount
    ),
    (CallPauseContract, Call::PauseContract),
    (CallResumeContract, Call::ResumeContract),
    (CallSetFixedGasCost, Call::SetFixedGasCost),
    (CallSetSiloParams, Call::SetSiloParams),
    (CallSetWhitelistStatus, Call::SetWhitelistStatus),
    (CallAddEntryToWhitelist, Call::AddEntryToWhitelist),
    (CallAddEntryToWhitelistBatch, Call::AddEntryToWhitelistBatch),
    (CallRemoveEntryFromWhitelist, Call::RemoveEntryFromWhitelist)
];

impl_call_return![
    (CallFtTransferCall => PromiseOrValue<U128>, Call::FtTransferCall, try_from),
    (CallStorageDeposit => StorageBalance, Call::StorageDeposit, json),
    (CallStorageUnregister => bool, Call::StorageUnregister, json),
    (CallStorageWithdraw => StorageBalance, Call::StorageWithdraw, json),
    (CallWithdraw => WithdrawResult, Call::Withdraw, borsh),
    (CallDeployCode => SubmitResult, Call::DeployCode, borsh),
    (CallDeployErc20Token => Address, Call::DeployErc20Token, borsh_address),
    (CallCall => SubmitResult, Call::Call, borsh),
    (CallSubmit => SubmitResult, Call::Submit, borsh),
    (CallFtOnTransfer => U128, Call::FtOnTransfer, json),
];

impl_view_return![
    (ViewFtTotalSupply => U128, View::FtTotalSupply, json),
    (ViewFtBalanceOf => U128, View::FtBalanceOf, json),
    (ViewStorageBalanceOf => StorageBalance, View::StorageBalanceOf, json),
    (ViewFtMetadata => FungibleTokenMetadata, View::FtMetadata, json),
    (ViewVersion => String, View::Version, borsh),
    (ViewOwner => AccountId, View::Owner, borsh),
    (ViewBridgeProver => AccountId, View::BridgeProver, borsh),
    (ViewChainId => U256, View::ChainId, borsh_U256),
    (ViewUpgradeIndex => u64, View::UpgradeIndex, borsh),
    (ViewPausedPrecompiles => u32, View::PausedPrecompiles, borsh),
    (ViewBlockHash => H256, View::BlockHash, borsh_H256),
    (ViewCode => Vec<u8>, View::Code, vec),
    (ViewBalance => U256, View::Balance, borsh_U256),
    (ViewNonce => U256, View::Nonce, borsh_U256),
    (ViewStorageAt => H256, View::StorageAt, borsh_H256),
    (ViewView => TransactionStatus, View::View, borsh),
    (ViewIsUsedProof => bool, View::IsUsedProof, borsh),
    (ViewFtTotalEthSupplyOnAurora => U128, View::FtTotalEthSupplyOnAurora, json),
    (ViewFtTotalEthSupplyOnNear => U128, View::FtTotalEthSupplyOnNear, json),
    (ViewFtBalanceOfEth => U128, View::FtBalanceOfEth, json),
    (ViewErc20FromNep141 => Address, View::Erc20FromNep141, borsh),
    (ViewNep141FromErc20 => AccountId, View::Nep141FromErc20, borsh),
    (ViewPausedFlags => u8, View::PausedFlags, borsh),
    (ViewAccountsCounter => u64, View::AccountsCounter, borsh),
    (ViewGetEthConnectorContractAccount => AccountId, View::GetEthConnectorContractAccount, borsh),
    (ViewGetFixedGasCost => FixedGasCostArgs, View::GetFixedGasCost, borsh),
    (ViewGetSiloParams => SiloParamsArgs, View::GetSiloParams, borsh),
    (ViewGetWhitelistStatus => WhitelistStatusArgs, View::GetWhitelistStatus, borsh),
    (ViewFactoryWnearAddress => Address, View::FactoryWnearAddress, borsh)
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Call {
    New,
    NewEthConnector,
    DeployCode,
    DeployErc20Token,
    Call,
    Submit,
    RegisterRelayer,
    FtOnTransfer,
    Withdraw,
    Deposit,
    FtTransfer,
    FtTransferCall,
    StorageDeposit,
    StorageUnregister,
    StorageWithdraw,
    PausePrecompiles,
    StageUpgrade,
    DeployUpgrade,
    StateMigration,
    ResumePrecompiles,
    FactoryUpdate,
    FundXccSubAccount,
    FactorySetWNearAddress,
    SetEthConnectorContractData,
    SetEthConnectorContractAccount,
    FactoryUpdateAddressVersion,
    RefundOnError,
    MintAccount,
    SetPausedFlags,
    SetKeyManager,
    AddRelayerKey,
    RemoveRelayerKey,
    PauseContract,
    ResumeContract,
    SetFixedGasCost,
    SetSiloParams,
    SetWhitelistStatus,
    AddEntryToWhitelist,
    AddEntryToWhitelistBatch,
    RemoveEntryFromWhitelist,
}

impl AsRef<str> for Call {
    fn as_ref(&self) -> &str {
        match self {
            Call::New => "new",
            Call::NewEthConnector => "new_eth_connector",
            Call::DeployCode => "deploy_code",
            Call::DeployErc20Token => "deploy_erc20_token",
            Call::Call => "call",
            Call::Submit => "submit",
            Call::RegisterRelayer => "register_relayer",
            Call::FtOnTransfer => "ft_on_transfer",
            Call::Withdraw => "withdraw",
            Call::Deposit => "deposit",
            Call::FtTransfer => "ft_transfer",
            Call::FtTransferCall => "ft_transfer_call",
            Call::StorageDeposit => "storage_deposit",
            Call::StorageUnregister => "storage_unregister",
            Call::StorageWithdraw => "storage_withdraw",
            Call::PausePrecompiles => "pause_precompiles",
            Call::StageUpgrade => "stage_upgrade",
            Call::DeployUpgrade => "deploy_upgrade",
            Call::StateMigration => "state_migration",
            Call::ResumePrecompiles => "resume_precompiles",
            Call::FactoryUpdate => "factory_update",
            Call::FundXccSubAccount => "fund_xcc_sub_account",
            Call::FactorySetWNearAddress => "factory_set_wnear_address",
            Call::SetEthConnectorContractData => "set_eth_connector_contract_data",
            Call::SetEthConnectorContractAccount => "set_eth_connector_contract_account",
            Call::FactoryUpdateAddressVersion => "factory_update_address_version",
            Call::RefundOnError => "refund_on_error",
            Call::MintAccount => "mint_account",
            Call::SetPausedFlags => "set_paused_flags",
            Call::SetKeyManager => "set_key_manager",
            Call::AddRelayerKey => "add_relayer_key",
            Call::RemoveRelayerKey => "remove_relayer_key",
            Call::PauseContract => "pause_contract",
            Call::ResumeContract => "resume_contract",
            Call::SetFixedGasCost => "set_fixed_gas_cost",
            Call::SetSiloParams => "set_silo_params",
            Call::SetWhitelistStatus => "set_whitelist_status",
            Call::AddEntryToWhitelist => "add_entry_to_whitelist",
            Call::AddEntryToWhitelistBatch => "add_entry_to_whitelist_batch",
            Call::RemoveEntryFromWhitelist => "remove_entry_from_whitelist",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum View {
    Version,
    Owner,
    BridgeProver,
    ChainId,
    UpgradeIndex,
    PausedPrecompiles,
    BlockHash,
    Code,
    Balance,
    Nonce,
    StorageAt,
    View,
    IsUsedProof,
    FtTotalSupply,
    FtBalanceOf,
    FtBalanceOfEth,
    FtTotalEthSupplyOnAurora,
    FtTotalEthSupplyOnNear,
    FtMetadata,
    StorageBalanceOf,
    PausedFlags,
    Erc20FromNep141,
    Nep141FromErc20,
    AccountsCounter,
    GetEthConnectorContractAccount,
    GetFixedGasCost,
    GetSiloParams,
    GetWhitelistStatus,
    FactoryWnearAddress,
}

impl AsRef<str> for View {
    fn as_ref(&self) -> &str {
        match self {
            View::Version => "get_version",
            View::Owner => "get_owner",
            View::BridgeProver => "get_bridge_prover",
            View::ChainId => "get_chain_id",
            View::UpgradeIndex => "get_upgrade_index",
            View::PausedPrecompiles => "get_paused_precompiles",
            View::BlockHash => "get_block_hash",
            View::Code => "get_code",
            View::Balance => "get_balance",
            View::Nonce => "get_nonce",
            View::StorageAt => "get_storage_at",
            View::View => "get_view",
            View::IsUsedProof => "is_used_proof",
            View::FtTotalSupply => "ft_total_supply",
            View::FtBalanceOf => "ft_balance_of",
            View::FtBalanceOfEth => "ft_balance_of_eth",
            View::FtTotalEthSupplyOnAurora => "ft_total_eth_supply_on_aurora",
            View::FtTotalEthSupplyOnNear => "ft_total_eth_supply_on_near",
            View::FtMetadata => "ft_metadata",
            View::StorageBalanceOf => "storage_balance_of",
            View::PausedFlags => "get_paused_flags",
            View::Erc20FromNep141 => "get_erc20_from_nep141",
            View::Nep141FromErc20 => "get_nep141_from_erc20",
            View::AccountsCounter => "get_accounts_counter",
            View::GetEthConnectorContractAccount => "get_eth_connector_contract_account",
            View::GetFixedGasCost => "get_fixed_gas_cost",
            View::GetSiloParams => "get_silo_params",
            View::GetWhitelistStatus => "get_whitelist_status",
            View::FactoryWnearAddress => "factory_get_wnear_address",
        }
    }
}
