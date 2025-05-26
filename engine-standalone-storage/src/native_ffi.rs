use std::{
    ffi::{self, OsStr},
    ops::Deref,
};

use aurora_engine_types::parameters::PromiseOrValue;

use aurora_engine_types::{
    parameters::silo::{SiloParamsArgs, WhitelistArgs, WhitelistStatusArgs},
    types::{Address, EthGas},
};
use libloading::os::unix::{self, Library};
use parking_lot::{Mutex, MutexGuard};
use thiserror::Error;

use aurora_engine::{contract_methods::ContractError, parameters::SubmitResult};

static CONTRACT: Mutex<Option<DynamicContractImpl>> = Mutex::new(None);

pub fn state() -> &'static super::state::State {
    super::state::STATE.get().expect("state is not initialized")
}

#[inline]
pub fn lock() -> impl Deref<Target = DynamicContractImpl> {
    MutexGuard::map(CONTRACT.lock(), |x| {
        x.as_mut().expect("must load library before use")
    })
}

pub struct DynamicContractImpl {
    library: Library,
    get_version_fn: extern "C" fn() -> *mut ffi::c_void,
    submit_fn: extern "C" fn() -> *mut ffi::c_void,
    submit_with_args_fn: extern "C" fn() -> *mut ffi::c_void,
    call_fn: extern "C" fn() -> *mut ffi::c_void,
    deploy_code_fn: extern "C" fn() -> *mut ffi::c_void,
    deploy_erc20_token_fn: extern "C" fn() -> *mut ffi::c_void,
    deploy_erc20_token_callback_fn: extern "C" fn() -> *mut ffi::c_void,
    ft_on_transfer_fn: extern "C" fn() -> *mut ffi::c_void,
    register_relayer_fn: extern "C" fn() -> *mut ffi::c_void,
    exit_to_near_precompile_callback_fn: extern "C" fn() -> *mut ffi::c_void,
    new_fn: extern "C" fn() -> *mut ffi::c_void,
    set_eth_connector_contract_account_fn: extern "C" fn() -> *mut ffi::c_void,
    factory_update_fn: extern "C" fn() -> *mut ffi::c_void,
    factory_update_address_version_fn: extern "C" fn() -> *mut ffi::c_void,
    factory_set_wnear_address_fn: extern "C" fn() -> *mut ffi::c_void,
    fund_xcc_sub_account_fn: extern "C" fn() -> *mut ffi::c_void,
    withdraw_wnear_to_router_fn: extern "C" fn() -> *mut ffi::c_void,
    pause_precompiles_fn: extern "C" fn() -> *mut ffi::c_void,
    resume_precompiles_fn: extern "C" fn() -> *mut ffi::c_void,
    set_owner_fn: extern "C" fn() -> *mut ffi::c_void,
    set_upgrade_delay_blocks_fn: extern "C" fn() -> *mut ffi::c_void,
    pause_contract_fn: extern "C" fn() -> *mut ffi::c_void,
    resume_contract_fn: extern "C" fn() -> *mut ffi::c_void,
    set_key_manager_fn: extern "C" fn() -> *mut ffi::c_void,
    add_relayer_key_fn: extern "C" fn() -> *mut ffi::c_void,
    store_relayer_key_callback_fn: extern "C" fn() -> *mut ffi::c_void,
    remove_relayer_key_fn: extern "C" fn() -> *mut ffi::c_void,
    start_hashchain_fn: extern "C" fn() -> *mut ffi::c_void,
    set_erc20_metadata_fn: extern "C" fn() -> *mut ffi::c_void,
    mirror_erc20_token_callback_fn: extern "C" fn() -> *mut ffi::c_void,
    silo_set_fixed_gas_fn: extern "C" fn(*mut ffi::c_void),
    silo_set_erc20_fallback_address_fn: extern "C" fn(*mut ffi::c_void),
    silo_set_silo_params_fn: extern "C" fn(*mut ffi::c_void),
    silo_add_entry_to_whitelist_fn: extern "C" fn(*mut ffi::c_void),
    silo_add_entry_to_whitelist_batch_fn: extern "C" fn(*mut ffi::c_void),
    silo_remove_entry_from_whitelist_fn: extern "C" fn(*mut ffi::c_void),
    silo_set_whitelist_status_fn: extern "C" fn(*mut ffi::c_void),
    silo_set_whitelists_statuses_fn: extern "C" fn(*mut ffi::c_void),
}

#[derive(Debug, Error)]
pub enum LibLoadingError {
    #[error("shared object unloading: {0}")]
    Unload(libloading::Error),
    #[error("shared object loading: {0}")]
    Loading(libloading::Error),
    #[error("loading function {name} from shared object error {err}")]
    Function {
        name: &'static str,
        err: libloading::Error,
    },
}

/// Returns version of the loaded contract
pub fn load<P>(path: P) -> Result<(), LibLoadingError>
where
    P: AsRef<OsStr>,
{
    if let Some(old) = CONTRACT.lock().take() {
        old.library.close().map_err(LibLoadingError::Unload)?;
    }

    let library = unsafe { Library::open(Some(path), unix::RTLD_GLOBAL | unix::RTLD_LAZY) }
        .map_err(LibLoadingError::Loading)?;
    let name = "_native_get_version";
    let get_version_fn =
        *unsafe { library.get::<extern "C" fn() -> *mut ffi::c_void>(name.as_bytes()) }
            .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_submit";
    let submit_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_submit_with_args";
    let submit_with_args_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_call";
    let call_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_deploy_code";
    let deploy_code_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_deploy_erc20_token";
    let deploy_erc20_token_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_deploy_erc20_token_callback";
    let deploy_erc20_token_callback_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_ft_on_transfer";
    let ft_on_transfer_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_register_relayer";
    let register_relayer_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_exit_to_near_precompile_callback";
    let exit_to_near_precompile_callback_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_new";
    let new_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_set_eth_connector_contract_account";
    let set_eth_connector_contract_account_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_factory_update";
    let factory_update_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_factory_update_address_version";
    let factory_update_address_version_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_factory_set_wnear_address";
    let factory_set_wnear_address_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_fund_xcc_sub_account";
    let fund_xcc_sub_account_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_withdraw_wnear_to_router";
    let withdraw_wnear_to_router_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_pause_precompiles";
    let pause_precompiles_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_resume_precompiles";
    let resume_precompiles_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_set_owner";
    let set_owner_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_set_upgrade_delay_blocks";
    let set_upgrade_delay_blocks_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_pause_contract";
    let pause_contract_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_resume_contract";
    let resume_contract_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_set_key_manager";
    let set_key_manager_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_add_relayer_key";
    let add_relayer_key_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_store_relayer_key_callback";
    let store_relayer_key_callback_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_remove_relayer_key";
    let remove_relayer_key_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_start_hashchain";
    let start_hashchain_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_set_erc20_metadata";
    let set_erc20_metadata_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_mirror_erc20_token_callback";
    let mirror_erc20_token_callback_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_set_fixed_gas";
    let silo_set_fixed_gas_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_set_erc20_fallback_address";
    let silo_set_erc20_fallback_address_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_set_silo_params";
    let silo_set_silo_params_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_add_entry_to_whitelist";
    let silo_add_entry_to_whitelist_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_add_entry_to_whitelist_batch";
    let silo_add_entry_to_whitelist_batch_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_remove_entry_from_whitelist";
    let silo_remove_entry_from_whitelist_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_set_whitelist_status";
    let silo_set_whitelist_status_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    let name = "_native_silo_set_whitelists_statuses";
    let silo_set_whitelists_statuses_fn = *unsafe { library.get(name.as_bytes()) }
        .map_err(|err| LibLoadingError::Function { name, err })?;
    *CONTRACT.lock() = Some(DynamicContractImpl {
        library,
        get_version_fn,
        submit_fn,
        submit_with_args_fn,
        call_fn,
        deploy_code_fn,
        deploy_erc20_token_fn,
        deploy_erc20_token_callback_fn,
        ft_on_transfer_fn,
        register_relayer_fn,
        exit_to_near_precompile_callback_fn,
        new_fn,
        set_eth_connector_contract_account_fn,
        factory_update_fn,
        factory_update_address_version_fn,
        factory_set_wnear_address_fn,
        fund_xcc_sub_account_fn,
        withdraw_wnear_to_router_fn,
        pause_precompiles_fn,
        resume_precompiles_fn,
        set_owner_fn,
        set_upgrade_delay_blocks_fn,
        pause_contract_fn,
        resume_contract_fn,
        set_key_manager_fn,
        add_relayer_key_fn,
        store_relayer_key_callback_fn,
        remove_relayer_key_fn,
        start_hashchain_fn,
        set_erc20_metadata_fn,
        mirror_erc20_token_callback_fn,
        silo_set_fixed_gas_fn,
        silo_set_erc20_fallback_address_fn,
        silo_set_silo_params_fn,
        silo_add_entry_to_whitelist_fn,
        silo_add_entry_to_whitelist_batch_fn,
        silo_remove_entry_from_whitelist_fn,
        silo_set_whitelist_status_fn,
        silo_set_whitelists_statuses_fn,
    });

    Ok(())
}

impl DynamicContractImpl {
    pub fn get_version(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.get_version_fn)().cast()) }
    }

    pub(crate) fn submit(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.submit_fn)().cast()) }
    }

    pub(crate) fn submit_with_args(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.submit_with_args_fn)().cast()) }
    }

    pub(crate) fn call(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.call_fn)().cast()) }
    }

    pub(crate) fn deploy_code(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.deploy_code_fn)().cast()) }
    }

    pub(crate) fn deploy_erc20_token(&self) -> Result<PromiseOrValue<Address>, ContractError> {
        *unsafe { Box::from_raw((self.deploy_erc20_token_fn)().cast()) }
    }

    pub(crate) fn deploy_erc20_token_callback(&self) -> Result<Address, ContractError> {
        *unsafe { Box::from_raw((self.deploy_erc20_token_callback_fn)().cast()) }
    }

    pub(crate) fn ft_on_transfer(&self) -> Result<Option<SubmitResult>, ContractError> {
        *unsafe { Box::from_raw((self.ft_on_transfer_fn)().cast()) }
    }

    pub(crate) fn register_relayer(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.register_relayer_fn)().cast()) }
    }

    pub(crate) fn exit_to_near_precompile_callback(
        &self,
    ) -> Result<Option<SubmitResult>, ContractError> {
        *unsafe { Box::from_raw((self.exit_to_near_precompile_callback_fn)().cast()) }
    }

    pub(crate) fn new(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.new_fn)().cast()) }
    }

    pub(crate) fn set_eth_connector_contract_account(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.set_eth_connector_contract_account_fn)().cast()) }
    }

    pub(crate) fn factory_update(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.factory_update_fn)().cast()) }
    }

    pub(crate) fn factory_update_address_version(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.factory_update_address_version_fn)().cast()) }
    }

    pub(crate) fn factory_set_wnear_address(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.factory_set_wnear_address_fn)().cast()) }
    }

    pub(crate) fn fund_xcc_sub_account(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.fund_xcc_sub_account_fn)().cast()) }
    }

    pub(crate) fn withdraw_wnear_to_router(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.withdraw_wnear_to_router_fn)().cast()) }
    }

    pub(crate) fn pause_precompiles(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.pause_precompiles_fn)().cast()) }
    }

    pub(crate) fn resume_precompiles(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.resume_precompiles_fn)().cast()) }
    }

    pub(crate) fn set_owner(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.set_owner_fn)().cast()) }
    }

    pub(crate) fn set_upgrade_delay_blocks(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.set_upgrade_delay_blocks_fn)().cast()) }
    }

    pub(crate) fn pause_contract(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.pause_contract_fn)().cast()) }
    }

    pub(crate) fn resume_contract(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.resume_contract_fn)().cast()) }
    }

    pub(crate) fn set_key_manager(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.set_key_manager_fn)().cast()) }
    }

    pub(crate) fn add_relayer_key(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.add_relayer_key_fn)().cast()) }
    }

    pub(crate) fn store_relayer_key_callback(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.store_relayer_key_callback_fn)().cast()) }
    }

    pub(crate) fn remove_relayer_key(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.remove_relayer_key_fn)().cast()) }
    }

    pub(crate) fn start_hashchain(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.start_hashchain_fn)().cast()) }
    }

    pub(crate) fn set_erc20_metadata(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.set_erc20_metadata_fn)().cast()) }
    }

    pub(crate) fn mirror_erc20_token_callback(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.mirror_erc20_token_callback_fn)().cast()) }
    }

    pub(crate) fn silo_set_fixed_gas(&self, fixed_gas: Option<EthGas>) {
        (self.silo_set_fixed_gas_fn)(Box::into_raw(Box::new(fixed_gas)).cast());
    }

    pub(crate) fn silo_set_erc20_fallback_address(&self, address: Option<Address>) {
        (self.silo_set_erc20_fallback_address_fn)(Box::into_raw(Box::new(address)).cast());
    }

    pub(crate) fn silo_set_silo_params(&self, params: Option<SiloParamsArgs>) {
        (self.silo_set_silo_params_fn)(Box::into_raw(Box::new(params)).cast());
    }

    pub(crate) fn silo_add_entry_to_whitelist(&self, args: WhitelistArgs) {
        (self.silo_add_entry_to_whitelist_fn)(Box::into_raw(Box::new(args)).cast());
    }

    pub(crate) fn silo_add_entry_to_whitelist_batch(&self, args: Vec<WhitelistArgs>) {
        (self.silo_add_entry_to_whitelist_batch_fn)(Box::into_raw(Box::new(args)).cast());
    }

    pub(crate) fn silo_remove_entry_from_whitelist(&self, args: WhitelistArgs) {
        (self.silo_remove_entry_from_whitelist_fn)(Box::into_raw(Box::new(args)).cast());
    }

    pub(crate) fn silo_set_whitelist_status(&self, args: WhitelistStatusArgs) {
        (self.silo_set_whitelist_status_fn)(Box::into_raw(Box::new(args)).cast());
    }

    pub(crate) fn silo_set_whitelists_statuses(&self, args: Vec<WhitelistStatusArgs>) {
        (self.silo_set_whitelists_statuses_fn)(Box::into_raw(Box::new(args)).cast());
    }
}
