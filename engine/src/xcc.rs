use aurora_engine_sdk::error::ReadU32Error;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage::{self, KeyPrefix};
use aurora_engine_types::types::Address;
use aurora_engine_types::Vec;

pub const ERR_NO_ROUTER_CODE: &str = "ERR_MISSING_XCC_BYTECODE";
pub const ERR_CORRUPTED_STORAGE: &str = "ERR_CORRUPTED_XCC_STORAGE";
pub const VERSION_KEY: &[u8] = b"version";
pub const CODE_KEY: &[u8] = b"router_code";

/// Type wrapper for version of router contracts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CodeVersion(pub u32);

impl CodeVersion {
    pub fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Type wrapper for router bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterCode(pub Vec<u8>);

/// Read the current wasm bytecode for the router contracts
pub fn get_router_code<I: IO>(io: &I) -> RouterCode {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, CODE_KEY);
    let bytes = io.read_storage(&key).expect(ERR_NO_ROUTER_CODE).to_vec();
    RouterCode(bytes)
}

/// Set new router bytecode, and update increment the version by 1.
pub fn update_router_code<I: IO>(io: &mut I, code: &RouterCode) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, CODE_KEY);
    io.write_storage(&key, &code.0);

    let current_version = get_latest_code_version(io);
    set_latest_code_version(io, current_version.increment());
}

/// Get the latest router contract version.
pub fn get_latest_code_version<I: IO>(io: &I) -> CodeVersion {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, VERSION_KEY);
    read_version(io, &key).unwrap_or_default()
}

/// Get the version of the currently deploy router for the given address (if it exists).
pub fn get_code_version_of_address<I: IO>(io: &I, address: &Address) -> Option<CodeVersion> {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, address.as_bytes());
    read_version(io, &key)
}

/// Set the version of the router contract deployed for the given address.
pub fn set_code_version_of_address<I: IO>(io: &mut I, address: &Address, version: CodeVersion) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, address.as_bytes());
    let value_bytes = version.0.to_le_bytes();
    io.write_storage(&key, &value_bytes);
}

/// Sets the latest router contract version. This function is intentionally private because
/// it should never be set manually. The version is managed automatically by `update_router_code`.
fn set_latest_code_version<I: IO>(io: &mut I, version: CodeVersion) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, VERSION_KEY);
    let value_bytes = version.0.to_le_bytes();
    io.write_storage(&key, &value_bytes);
}

/// Private utility method for reading code version from storage.
fn read_version<I: IO>(io: &I, key: &[u8]) -> Option<CodeVersion> {
    match io.read_u32(key) {
        Ok(value) => Some(CodeVersion(value)),
        Err(ReadU32Error::MissingValue) => None,
        Err(ReadU32Error::InvalidU32) => panic!("{}", ERR_CORRUPTED_STORAGE),
    }
}
