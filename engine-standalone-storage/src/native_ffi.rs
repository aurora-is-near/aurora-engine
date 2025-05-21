use std::{
    ffi::{self, OsStr},
    ops::Deref,
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

#[allow(dead_code)]
pub struct DynamicContractImpl {
    library: Library,
    get_version_fn: extern "C" fn() -> *mut ffi::c_void,
    submit_fn: extern "C" fn() -> *mut ffi::c_void,
    submit_with_args_fn: extern "C" fn() -> *mut ffi::c_void,
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
    *CONTRACT.lock() = Some(DynamicContractImpl {
        library,
        get_version_fn,
        submit_fn,
        submit_with_args_fn,
    });

    Ok(())
}

impl DynamicContractImpl {
    pub fn get_version(&self) -> Result<(), ContractError> {
        *unsafe { Box::from_raw((self.get_version_fn)().cast()) }
    }

    pub(crate) fn submit(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.get_version_fn)().cast()) }
    }

    pub(crate) fn submit_with_args(&self) -> Result<SubmitResult, ContractError> {
        *unsafe { Box::from_raw((self.get_version_fn)().cast()) }
    }
}
