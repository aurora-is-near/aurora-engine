use aurora_engine_sdk::error::ReadU32Error;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::{PromiseAction, PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::storage::{self, KeyPrefix};
use aurora_engine_types::types::{Address, NearGas, ZERO_YOCTO};
use aurora_engine_types::Vec;
use borsh::{BorshDeserialize, BorshSerialize};

pub const ERR_NO_ROUTER_CODE: &str = "ERR_MISSING_XCC_BYTECODE";
pub const ERR_CORRUPTED_STORAGE: &str = "ERR_CORRUPTED_XCC_STORAGE";
pub const ERR_INVALID_ACCOUNT: &str = "ERR_INVALID_XCC_ACCOUNT";
pub const VERSION_KEY: &[u8] = b"version";
pub const CODE_KEY: &[u8] = b"router_code";
// TODO: estimate gas
pub const VERSION_UPDATE_GAS: NearGas = NearGas::new(0);

/// Type wrapper for version of router contracts.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, BorshDeserialize, BorshSerialize,
)]
pub struct CodeVersion(pub u32);

impl CodeVersion {
    pub fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Type wrapper for router bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterCode(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct AddressVersionUpdateArgs {
    pub address: Address,
    pub version: CodeVersion,
}

pub fn handle_precomile_promise<I, P>(
    io: &I,
    handler: &mut P,
    promise: PromiseCreateArgs,
    current_account_id: &AccountId,
) where
    P: PromiseHandler,
    I: IO,
{
    let target_account: &str = promise.target_account_id.as_ref();
    let sender = Address::decode(&target_account[0..40]).expect(ERR_INVALID_ACCOUNT);
    let latest_code_version = get_latest_code_version(io);
    let sender_code_version = get_code_version_of_address(io, &sender);
    let mut promise_actions = Vec::new();
    let mut deploy_needed = false;
    match sender_code_version {
        None => {
            // Need to create the account
            promise_actions.push(PromiseAction::CreateAccount);
            // Then deploy the contract
            promise_actions.push(PromiseAction::DeployConotract {
                code: get_router_code(io).0,
            });
            deploy_needed = true;
        }
        Some(version) if version < latest_code_version => {
            // Account exist, but with outdated version, so deploy new one
            promise_actions.push(PromiseAction::DeployConotract {
                code: get_router_code(io).0,
            });
            deploy_needed = true;
        }
        Some(_version) => {
            // if the version match then we do not need to deploy, it already up to date
        }
    };
    // Regardless of whether a deploy is needed or not, we want to make a call to the account
    promise_actions.push(PromiseAction::FunctionCall {
        name: promise.method,
        args: promise.args,
        attached_yocto: promise.attached_balance,
        gas: promise.attached_gas,
    });
    let batch = PromiseBatchAction {
        target_account_id: promise.target_account_id,
        actions: promise_actions,
    };
    let promise_id = handler.promise_create_batch(&batch);
    if deploy_needed {
        // If a deploy was needed then we want there to be a callback here to update the version of the account
        let args = AddressVersionUpdateArgs {
            address: sender,
            version: latest_code_version,
        };
        let callback = PromiseCreateArgs {
            target_account_id: current_account_id.clone(),
            method: "factory_update_address_version".into(),
            args: args.try_to_vec().unwrap(),
            attached_balance: ZERO_YOCTO,
            attached_gas: VERSION_UPDATE_GAS,
        };

        handler.promise_attach_callback(promise_id, &callback);
    }
}

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
