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

pub fn read_state<F, T>(f: F) -> T
where
    F: FnOnce(&super::state::State) -> T,
{
    super::state::STATE.with_borrow(|state| f(state))
}

#[inline]
pub fn lock() -> impl Deref<Target = DynamicContractImpl> {
    MutexGuard::map(CONTRACT.lock(), |x| {
        x.as_mut().expect("must load a library before use")
    })
}

pub struct DynamicContractImpl {
    library: Library,
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
pub fn load_once<P>(path: P)
where
    P: AsRef<OsStr>,
{
    CONTRACT.lock().get_or_insert_with(|| {
        let library = unsafe {
            Library::open(Some(path), unix::RTLD_GLOBAL | unix::RTLD_LAZY)
                .map_err(LibLoadingError::Loading)
                .unwrap()
        };
        DynamicContractImpl { library }
    });
}

/// Returns version of the loaded contract
pub fn load<P>(path: P) -> Result<(), LibLoadingError>
where
    P: AsRef<OsStr>,
{
    let mut lock = CONTRACT.lock();
    if let Some(old) = lock.take() {
        old.library.close().map_err(LibLoadingError::Unload)?;
    }
    drop(lock);

    let library = unsafe { Library::open(Some(path), unix::RTLD_GLOBAL | unix::RTLD_LAZY) }
        .map_err(LibLoadingError::Loading)?;
    *CONTRACT.lock() = Some(DynamicContractImpl { library });

    Ok(())
}

impl DynamicContractImpl {
    fn e<T>(&self, name: &str) -> Result<T, ContractError> {
        *unsafe {
            Box::from_raw(
                self.library
                    .get::<extern "C" fn() -> *mut ffi::c_void>(name.as_bytes())
                    .unwrap_or_else(|_| panic!("symbol {name} not found"))()
                .cast(),
            )
        }
    }

    fn a<T>(&self, name: &str, arg: T) {
        unsafe {
            self.library
                .get::<extern "C" fn(*mut ffi::c_void)>(name.as_bytes())
                .unwrap_or_else(|_| panic!("symbol {name} not found"))(
                Box::into_raw(Box::new(arg)).cast(),
            );
        }
    }

    pub fn get_version(&self) -> Result<(), ContractError> {
        self.e("_native_get_version")
    }

    pub(crate) fn submit(&self) -> Result<SubmitResult, ContractError> {
        self.e("_native_submit")
    }

    pub(crate) fn submit_with_args(&self) -> Result<SubmitResult, ContractError> {
        self.e("_native_submit_with_args")
    }

    pub(crate) fn call(&self) -> Result<SubmitResult, ContractError> {
        self.e("_native_call")
    }

    pub(crate) fn deploy_code(&self) -> Result<SubmitResult, ContractError> {
        self.e("_native_deploy_code")
    }

    pub(crate) fn deploy_erc20_token(&self) -> Result<PromiseOrValue<Address>, ContractError> {
        self.e("_native_deploy_erc20_token")
    }

    pub(crate) fn deploy_erc20_token_callback(&self) -> Result<Address, ContractError> {
        self.e("_native_deploy_erc20_token_callback")
    }

    pub(crate) fn ft_on_transfer(&self) -> Result<Option<SubmitResult>, ContractError> {
        self.e("_native_ft_on_transfer")
    }

    pub(crate) fn register_relayer(&self) -> Result<(), ContractError> {
        self.e("_native_register_relayer")
    }

    pub(crate) fn exit_to_near_precompile_callback(
        &self,
    ) -> Result<Option<SubmitResult>, ContractError> {
        self.e("_native_exit_to_near_precompile_callback")
    }

    pub(crate) fn new_engine(&self) -> Result<(), ContractError> {
        self.e("_native_new")
    }

    pub(crate) fn set_eth_connector_contract_account(&self) -> Result<(), ContractError> {
        self.e("_native_set_eth_connector_contract_account")
    }

    pub(crate) fn factory_update(&self) -> Result<(), ContractError> {
        self.e("_native_factory_update")
    }

    pub(crate) fn factory_update_address_version(&self) -> Result<(), ContractError> {
        self.e("_native_factory_update_address_version")
    }

    pub(crate) fn factory_set_wnear_address(&self) -> Result<(), ContractError> {
        self.e("_native_factory_set_wnear_address")
    }

    pub(crate) fn fund_xcc_sub_account(&self) -> Result<(), ContractError> {
        self.e("_native_fund_xcc_sub_account")
    }

    pub(crate) fn withdraw_wnear_to_router(&self) -> Result<SubmitResult, ContractError> {
        self.e("_native_withdraw_wnear_to_router")
    }

    pub(crate) fn pause_precompiles(&self) -> Result<(), ContractError> {
        self.e("_native_pause_precompiles")
    }

    pub(crate) fn resume_precompiles(&self) -> Result<(), ContractError> {
        self.e("_native_resume_precompiles")
    }

    pub(crate) fn set_owner(&self) -> Result<(), ContractError> {
        self.e("_native_set_owner")
    }

    pub(crate) fn set_upgrade_delay_blocks(&self) -> Result<(), ContractError> {
        self.e("_native_set_upgrade_delay_blocks")
    }

    pub(crate) fn pause_contract(&self) -> Result<(), ContractError> {
        self.e("_native_pause_contract")
    }

    pub(crate) fn resume_contract(&self) -> Result<(), ContractError> {
        self.e("_native_resume_contract")
    }

    pub(crate) fn set_key_manager(&self) -> Result<(), ContractError> {
        self.e("_native_set_key_manager")
    }

    pub(crate) fn add_relayer_key(&self) -> Result<(), ContractError> {
        self.e("_native_add_relayer_key")
    }

    pub(crate) fn store_relayer_key_callback(&self) -> Result<(), ContractError> {
        self.e("_native_store_relayer_key_callback")
    }

    pub(crate) fn remove_relayer_key(&self) -> Result<(), ContractError> {
        self.e("_native_remove_relayer_key")
    }

    pub(crate) fn start_hashchain(&self) -> Result<(), ContractError> {
        self.e("_native_start_hashchain")
    }

    pub(crate) fn set_erc20_metadata(&self) -> Result<SubmitResult, ContractError> {
        self.e("_native_set_erc20_metadata")
    }

    pub(crate) fn mirror_erc20_token_callback(&self) -> Result<(), ContractError> {
        self.e("_native_mirror_erc20_token_callback")
    }

    pub(crate) fn silo_set_fixed_gas(&self, fixed_gas: Option<EthGas>) {
        self.a("_native_silo_set_fixed_gas", fixed_gas);
    }

    pub(crate) fn silo_set_erc20_fallback_address(&self, address: Option<Address>) {
        self.a("_native_silo_set_erc20_fallback_address", address);
    }

    pub(crate) fn silo_set_silo_params(&self, params: Option<SiloParamsArgs>) {
        self.a("_native_silo_set_silo_params", params);
    }

    pub(crate) fn silo_add_entry_to_whitelist(&self, args: WhitelistArgs) {
        self.a("_native_silo_add_entry_to_whitelist", args);
    }

    pub(crate) fn silo_add_entry_to_whitelist_batch(&self, args: Vec<WhitelistArgs>) {
        self.a("_native_silo_add_entry_to_whitelist_batch", args);
    }

    pub(crate) fn silo_remove_entry_from_whitelist(&self, args: WhitelistArgs) {
        self.a("_native_silo_remove_entry_from_whitelist", args);
    }

    pub(crate) fn silo_set_whitelist_status(&self, args: WhitelistStatusArgs) {
        self.a("_native_silo_set_whitelist_status", args);
    }

    pub(crate) fn silo_set_whitelists_statuses(&self, args: Vec<WhitelistStatusArgs>) {
        self.a("_native_silo_set_whitelists_statuses", args);
    }
}
