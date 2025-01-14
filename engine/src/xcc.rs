use crate::engine::{Engine, EngineResult};
use crate::errors::ERR_SERIALIZE;
use crate::parameters::{CallArgs, FunctionCallArgsV2, SubmitResult};
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_precompiles::xcc::state::ERR_MISSING_WNEAR_ADDRESS;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::promise::{PromiseHandler, PromiseId};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh::{self, BorshDeserialize, BorshSerialize};
use aurora_engine_types::parameters::xcc::WithdrawWnearToRouterArgs;
use aurora_engine_types::parameters::{PromiseAction, PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::storage::{self, KeyPrefix};
use aurora_engine_types::types::{Address, NearGas, Yocto, ZERO_YOCTO};
use aurora_engine_types::{format, Cow, Vec};

pub use aurora_engine_types::parameters::xcc::{AddressVersionUpdateArgs, FundXccArgs};

pub const ERR_NO_ROUTER_CODE: &str = "ERR_MISSING_XCC_BYTECODE";
pub const ERR_INVALID_ACCOUNT: &str = "ERR_INVALID_XCC_ACCOUNT";
pub const ERR_ATTACHED_NEAR: &str = "ERR_ATTACHED_XCC_NEAR";
pub const ERR_UPGRADE_ARG_SERIALIZATION: &str = "ERR_UPGRADE_ARG_SERIALIZATION";
pub const CODE_KEY: &[u8] = b"router_code";
/// Gas costs estimated from simulation tests.
pub const VERSION_UPDATE_GAS: NearGas = NearGas::new(5_000_000_000_000);
pub const INITIALIZE_GAS: NearGas = NearGas::new(15_000_000_000_000);
pub const UPGRADE_GAS: NearGas = NearGas::new(20_000_000_000_000);
pub const REFUND_GAS: NearGas = NearGas::new(5_000_000_000_000);
pub const WITHDRAW_GAS: NearGas = NearGas::new(40_000_000_000_000);
/// Solidity selector for the `withdrawToNear` function
/// `https://www.4byte.directory/signatures/?bytes4_signature=0x6b351848`
pub const WITHDRAW_TO_NEAR_SELECTOR: [u8; 4] = [0x6b, 0x35, 0x18, 0x48];
// Key for storing the XCC router version where upgradability was first introduced.
// (The initial version of the router was not upgradable, see
// https://github.com/aurora-is-near/aurora-engine/pull/866)
const FIRST_UPGRADABLE: &[u8] = b"first_upgrd";

pub use aurora_engine_precompiles::xcc::state::{
    get_code_version_of_address, get_latest_code_version, get_wnear_address, ERR_CORRUPTED_STORAGE,
    STORAGE_AMOUNT, VERSION_KEY, WNEAR_KEY,
};
pub use aurora_engine_types::parameters::xcc::CodeVersion;

/// Type wrapper for router bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterCode<'a>(pub Cow<'a, [u8]>);

impl<'a> RouterCode<'a> {
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(Cow::Owned(bytes))
    }

    #[must_use]
    pub const fn borrowed(bytes: &'a [u8]) -> Self {
        Self(Cow::Borrowed(bytes))
    }
}

/// Same as the corresponding struct in the xcc-router
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
pub struct DeployUpgradeParams {
    pub code: Vec<u8>,
    pub initialize_args: Vec<u8>,
}

pub fn fund_xcc_sub_account<I, P, E>(
    io: &I,
    handler: &mut P,
    env: &E,
    args: FundXccArgs,
) -> Result<(), FundXccError>
where
    P: PromiseHandler,
    I: IO + Copy,
    E: Env,
{
    let current_account_id = env.current_account_id();
    let target_account_id = AccountId::try_from(format!(
        "{}.{}",
        args.target.encode(),
        current_account_id.as_ref()
    ))?;

    let latest_code_version = get_latest_code_version(io);
    let target_code_version = get_code_version_of_address(io, &args.target);
    let deploy_needed = AddressVersionStatus::new(io, latest_code_version, target_code_version);

    let fund_amount = Yocto::new(env.attached_deposit());

    let mut promise_actions = Vec::with_capacity(4);

    // If account needs to be created and/or updated then include those actions.
    if let AddressVersionStatus::DeployNeeded { create_needed } = deploy_needed {
        let code = get_router_code(io).0.into_owned();
        // wnear_account is needed for initialization so we must assume it is set
        // in the Engine, or we need to accept it as input.
        let wnear_account = if let Some(wnear_account) = args.wnear_account_id {
            wnear_account
        } else {
            // If the wnear account is not specified then we must look it up based on the
            // bridged token registry for the engine.
            let wnear_address = get_wnear_address(io);
            crate::engine::nep141_erc20_map(*io)
                .lookup_right(&crate::engine::ERC20Address(wnear_address))
                .ok_or(FundXccError::MissingWNearAddress)?
                .0
        };
        let init_args = format!(
            r#"{{"wnear_account": "{}", "must_register": {}}}"#,
            wnear_account.as_ref(),
            create_needed,
        );
        if create_needed {
            if fund_amount < STORAGE_AMOUNT {
                return Err(FundXccError::InsufficientBalance);
            }

            promise_actions.push(PromiseAction::CreateAccount);
            promise_actions.push(PromiseAction::Transfer {
                amount: fund_amount,
            });
            promise_actions.push(PromiseAction::DeployContract { code });
            promise_actions.push(PromiseAction::FunctionCall {
                name: "initialize".into(),
                args: init_args.into_bytes(),
                attached_yocto: ZERO_YOCTO,
                gas: INITIALIZE_GAS,
            });
        } else {
            let deploy_args = DeployUpgradeParams {
                code,
                initialize_args: init_args.into_bytes(),
            };
            promise_actions.push(PromiseAction::FunctionCall {
                name: "deploy_upgrade".into(),
                args: borsh::to_vec(&deploy_args).expect(ERR_UPGRADE_ARG_SERIALIZATION),
                attached_yocto: fund_amount,
                gas: UPGRADE_GAS + INITIALIZE_GAS,
            });
        }
    } else {
        // No matter what include the transfer of the funding amount
        promise_actions.push(PromiseAction::Transfer {
            amount: fund_amount,
        });
    }

    let batch = PromiseBatchAction {
        target_account_id,
        actions: promise_actions,
    };
    // Safety: same as safety in `handle_precompile_promise`
    let promise_id = unsafe { handler.promise_create_batch(&batch) };

    if let AddressVersionStatus::DeployNeeded { .. } = deploy_needed {
        // If a create and/or deploy was needed then we must attach a callback to update
        // the Engine's record of the account.

        let args = AddressVersionUpdateArgs {
            address: args.target,
            version: latest_code_version,
        };
        let callback = PromiseCreateArgs {
            target_account_id: current_account_id,
            method: "factory_update_address_version".into(),
            args: borsh::to_vec(&args).map_err(|_| FundXccError::SerializationFailure)?,
            attached_balance: ZERO_YOCTO,
            attached_gas: VERSION_UPDATE_GAS,
        };
        // Safety: same as safety in `handle_precompile_promise`
        let _promise_id = unsafe { handler.promise_attach_callback(promise_id, &callback) };
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn handle_precompile_promise<I, P>(
    io: &I,
    handler: &mut P,
    base_id: Option<PromiseId>,
    promise: &PromiseCreateArgs,
    required_near: Yocto,
    current_account_id: &AccountId,
) -> PromiseId
where
    P: PromiseHandler,
    I: IO + Copy,
{
    let target_account: &str = promise.target_account_id.as_ref();
    let sender = Address::decode(&target_account[0..40]).expect(ERR_INVALID_ACCOUNT);

    // Confirm target_account is of the form `{address}.{aurora}`
    // Address prefix parsed above, so only need to check `.{aurora}`
    assert_eq!(&target_account[40..41], ".", "{ERR_INVALID_ACCOUNT}");
    assert_eq!(
        &target_account[41..],
        current_account_id.as_ref(),
        "{ERR_INVALID_ACCOUNT}"
    );
    // Confirm there is 0 NEAR attached to the promise
    // (the precompile should not drain the engine's balance).
    assert_eq!(promise.attached_balance, ZERO_YOCTO, "{ERR_ATTACHED_NEAR}");

    let latest_code_version = get_latest_code_version(io);
    let sender_code_version = get_code_version_of_address(io, &sender);
    let deploy_needed = AddressVersionStatus::new(io, latest_code_version, sender_code_version);
    // 1. If the router contract account does not exist or is out of date then we start
    //    with a batch transaction to deploy the router. This batch also has an attached
    //    callback to update the engine's storage with the new version of that router account.
    let setup_id = match &deploy_needed {
        AddressVersionStatus::DeployNeeded { create_needed } => {
            let mut promise_actions = Vec::with_capacity(4);
            let code = get_router_code(io).0.into_owned();
            // After the deployment we will call the contract's initialize function
            let wnear_address = get_wnear_address(io);
            let wnear_account = crate::engine::nep141_erc20_map(*io)
                .lookup_right(&crate::engine::ERC20Address(wnear_address))
                .expect("wnear account not found");
            let init_args = format!(
                r#"{{"wnear_account": "{}", "must_register": {}}}"#,
                wnear_account.0.as_ref(),
                create_needed,
            );
            if *create_needed {
                promise_actions.push(PromiseAction::CreateAccount);
                promise_actions.push(PromiseAction::Transfer {
                    amount: STORAGE_AMOUNT,
                });
                promise_actions.push(PromiseAction::DeployContract { code });
                promise_actions.push(PromiseAction::FunctionCall {
                    name: "initialize".into(),
                    args: init_args.into_bytes(),
                    attached_yocto: ZERO_YOCTO,
                    gas: INITIALIZE_GAS,
                });
            } else {
                let deploy_args = DeployUpgradeParams {
                    code,
                    initialize_args: init_args.into_bytes(),
                };
                promise_actions.push(PromiseAction::FunctionCall {
                    name: "deploy_upgrade".into(),
                    args: borsh::to_vec(&deploy_args).expect(ERR_UPGRADE_ARG_SERIALIZATION),
                    attached_yocto: ZERO_YOCTO,
                    gas: UPGRADE_GAS + INITIALIZE_GAS,
                });
            }

            let batch = PromiseBatchAction {
                target_account_id: promise.target_account_id.clone(),
                actions: promise_actions,
            };
            // Safety: This batch creation is safe because it only acts on the router sub-account
            // (not the main engine account), and the actions performed are only (1) create it
            // for the first time and/or (2) deploy the code from our storage (i.e. the deployed
            // code is controlled by us, not the user).
            let promise_id = unsafe {
                match base_id {
                    Some(id) => handler.promise_attach_batch_callback(id, &batch),
                    None => handler.promise_create_batch(&batch),
                }
            };
            // Add a callback here to update the version of the account
            let args = AddressVersionUpdateArgs {
                address: sender,
                version: latest_code_version,
            };
            let callback = PromiseCreateArgs {
                target_account_id: current_account_id.clone(),
                method: "factory_update_address_version".into(),
                args: borsh::to_vec(&args).unwrap(),
                attached_balance: ZERO_YOCTO,
                attached_gas: VERSION_UPDATE_GAS,
            };

            // Safety: A call from the engine to the engine's `factory_update_address_version`
            // method is safe because that method only writes the specific router sub-account
            // metadata that has just been deployed above.
            unsafe { Some(handler.promise_attach_callback(promise_id, &callback)) }
        }
        AddressVersionStatus::UpToDate => base_id,
    };
    // 2. If some NEAR is required for this call (from storage staking for a new account
    //    and/or attached NEAR to the call the user wants to make), then we need to have the
    //    engine withdraw that amount of wNEAR to the router account and then have the router
    //    unwrap it into actual NEAR. In the case of storage staking, the engine contract
    //    covered the cost initially (see setup batch above), so the unwrapping also sends
    //    a refund back to the engine.
    let withdraw_id = if required_near == ZERO_YOCTO {
        setup_id
    } else {
        let withdraw_call_args = WithdrawWnearToRouterArgs {
            target: sender,
            amount: required_near,
        };
        let withdraw_call = PromiseCreateArgs {
            target_account_id: current_account_id.clone(),
            method: "withdraw_wnear_to_router".into(),
            args: borsh::to_vec(&withdraw_call_args).unwrap(),
            attached_balance: ZERO_YOCTO,
            attached_gas: WITHDRAW_GAS,
        };
        // Safety: This promise is safe. Even though this is a call from the engine account to
        // itself invoking the `call` method (which could be dangerous), the argument to `call`
        // is controlled entirely by us (not any user). This call will only execute the wnear
        // exit precompile, and only for the necessary amount. Note that this amount will always
        // be present, otherwise the user's call to the xcc precompile would have failed.
        let id = unsafe {
            match setup_id {
                None => handler.promise_create_call(&withdraw_call),
                Some(setup_id) => handler.promise_attach_callback(setup_id, &withdraw_call),
            }
        };
        let refund_needed = match deploy_needed {
            AddressVersionStatus::DeployNeeded { create_needed } => create_needed,
            AddressVersionStatus::UpToDate => false,
        };
        if refund_needed {
            let refund_call = PromiseCreateArgs {
                target_account_id: promise.target_account_id.clone(),
                method: "send_refund".into(),
                args: Vec::new(),
                attached_balance: ZERO_YOCTO,
                attached_gas: REFUND_GAS,
            };
            // Safety: This call is safe because the router's `send_refund` method
            // does not violate any security invariants. It only sends NEAR back to this contract.
            unsafe { Some(handler.promise_attach_callback(id, &refund_call)) }
        } else {
            Some(id)
        }
    };
    // 3. Finally we can do the call the user wanted to do.

    // Safety: this call is safe because the promise comes from the XCC precompile, not the
    // user directly. The XCC precompile will only construct promises that target the `execute`
    // and `schedule` methods of the user's router contract. Therefore, the user cannot have
    // the engine make arbitrary calls.
    unsafe {
        match withdraw_id {
            None => handler.promise_create_call(promise),
            Some(withdraw_id) => handler.promise_attach_callback(withdraw_id, promise),
        }
    }
}

/// Read the current wasm bytecode for the router contracts
pub fn get_router_code<I: IO>(io: &I) -> RouterCode {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, CODE_KEY);
    let bytes = io.read_storage(&key).expect(ERR_NO_ROUTER_CODE).to_vec();
    RouterCode::new(bytes)
}

/// Set new router bytecode, and update increment the version by 1.
pub fn update_router_code<I: IO>(io: &mut I, code: &RouterCode) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, CODE_KEY);
    io.write_storage(&key, &code.0);

    let current_version = get_latest_code_version(io);
    let latest_version = current_version.increment();

    // Store the latest version, this will be the first one where the
    // router contract is upgradable.
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, FIRST_UPGRADABLE);
    if io.read_storage(&key).is_none() {
        let version_bytes = latest_version.0.to_le_bytes();
        io.write_storage(&key, &version_bytes);
    }

    set_latest_code_version(io, latest_version);
}

/// Set the address of the `wNEAR` ERC-20 contract
pub fn set_wnear_address<I: IO>(io: &mut I, address: &Address) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, WNEAR_KEY);
    io.write_storage(&key, address.as_bytes());
}

/// Set the version of the router contract deployed for the given address.
pub fn set_code_version_of_address<I: IO>(io: &mut I, address: &Address, version: CodeVersion) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, address.as_bytes());
    let value_bytes = version.0.to_le_bytes();
    io.write_storage(&key, &value_bytes);
}

pub fn withdraw_wnear_to_router<I: IO + Copy, E: Env, M: ModExpAlgorithm, H: PromiseHandler>(
    recipient: &AccountId,
    amount: Yocto,
    wnear_address: Address,
    engine: &mut Engine<I, E, M>,
    handler: &mut H,
) -> EngineResult<(SubmitResult, Vec<PromiseId>)> {
    let mut interceptor = PromiseInterceptor::new(handler);
    let withdraw_call_args = withdraw_wnear_call_args(recipient, amount, wnear_address);
    let result = engine.call_with_args(withdraw_call_args, &mut interceptor)?;
    Ok((result, interceptor.promises))
}

#[must_use]
pub fn withdraw_wnear_call_args(
    recipient: &AccountId,
    amount: Yocto,
    wnear_address: Address,
) -> CallArgs {
    CallArgs::V2(FunctionCallArgsV2 {
        contract: wnear_address,
        value: [0u8; 32],
        input: withdraw_to_near_args(recipient, amount),
    })
}

#[derive(Debug, Clone, Copy)]
pub enum FundXccError {
    InsufficientBalance,
    InvalidAccount,
    MissingWNearAddress,
    SerializationFailure,
}

impl From<aurora_engine_types::account_id::ParseAccountError> for FundXccError {
    fn from(_: aurora_engine_types::account_id::ParseAccountError) -> Self {
        Self::InvalidAccount
    }
}

impl AsRef<[u8]> for FundXccError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::InsufficientBalance => b"ERR_INSUFFICIENT_FUNDING_OF_NEW_XCC_ACCOUNT",
            Self::InvalidAccount => ERR_INVALID_ACCOUNT.as_bytes(),
            Self::MissingWNearAddress => ERR_MISSING_WNEAR_ADDRESS.as_bytes(),
            Self::SerializationFailure => ERR_SERIALIZE.as_bytes(),
        }
    }
}

/// Sets the latest router contract version. This function is intentionally private because
/// it should never be set manually. The version is managed automatically by `update_router_code`.
fn set_latest_code_version<I: IO>(io: &mut I, version: CodeVersion) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, VERSION_KEY);
    let value_bytes = version.0.to_le_bytes();
    io.write_storage(&key, &value_bytes);
}

fn get_first_upgradable_version<I: IO>(io: &I) -> Option<CodeVersion> {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, FIRST_UPGRADABLE);
    io.read_u32(&key)
        .map_or(None, |value| Some(CodeVersion(value)))
}

/// Private enum used for bookkeeping what actions are needed in the call to the router contract.
enum AddressVersionStatus {
    UpToDate,
    DeployNeeded { create_needed: bool },
}

impl AddressVersionStatus {
    fn new<I: IO>(
        io: &I,
        latest_code_version: CodeVersion,
        target_code_version: Option<CodeVersion>,
    ) -> Self {
        let first_upgradable_version =
            get_first_upgradable_version(io).unwrap_or(CodeVersion::ZERO);
        match target_code_version {
            None => Self::DeployNeeded {
                create_needed: true,
            },
            Some(version) if version < first_upgradable_version => {
                // It is impossible to upgrade the initial XCC routers because
                // they lack the upgrade method.
                Self::UpToDate
            }
            Some(version) if version < latest_code_version => Self::DeployNeeded {
                create_needed: false,
            },
            Some(_version) => Self::UpToDate,
        }
    }
}

fn withdraw_to_near_args(recipient: &AccountId, amount: Yocto) -> Vec<u8> {
    let recipient_with_msg = format!("{recipient}:unwrap");
    let args = ethabi::encode(&[
        ethabi::Token::Bytes(recipient_with_msg.into_bytes()),
        ethabi::Token::Uint(amount.as_u128().into()),
    ]);
    [&WITHDRAW_TO_NEAR_SELECTOR, args.as_slice()].concat()
}

/// A `PromiseHandler` that remembers all the `PromiseIds` it creates.
/// This is used to make a promise the return value of a function even
/// if the promise was not captured in the code where the handler is used.
/// For example, this can capture the promises created by the exit precompiles.
struct PromiseInterceptor<'a, H> {
    inner: &'a mut H,
    promises: Vec<PromiseId>,
}

impl<'a, H> PromiseInterceptor<'a, H> {
    fn new(inner: &'a mut H) -> Self {
        Self {
            inner,
            promises: Vec::new(),
        }
    }
}

impl<'a, H: PromiseHandler> PromiseHandler for PromiseInterceptor<'a, H> {
    type ReadOnly = H::ReadOnly;

    fn promise_results_count(&self) -> u64 {
        self.inner.promise_results_count()
    }

    fn promise_result(&self, index: u64) -> Option<aurora_engine_types::types::PromiseResult> {
        self.inner.promise_result(index)
    }

    unsafe fn promise_create_call(&mut self, args: &PromiseCreateArgs) -> PromiseId {
        let id = self.inner.promise_create_call(args);
        self.promises.push(id);
        id
    }

    unsafe fn promise_create_and_combine(&mut self, args: &[PromiseCreateArgs]) -> PromiseId {
        let id = self.inner.promise_create_and_combine(args);
        self.promises.push(id);
        id
    }

    unsafe fn promise_attach_callback(
        &mut self,
        base: PromiseId,
        callback: &PromiseCreateArgs,
    ) -> PromiseId {
        let id = self.inner.promise_attach_callback(base, callback);
        self.promises.push(id);
        id
    }

    unsafe fn promise_create_batch(&mut self, args: &PromiseBatchAction) -> PromiseId {
        let id = self.inner.promise_create_batch(args);
        self.promises.push(id);
        id
    }

    unsafe fn promise_attach_batch_callback(
        &mut self,
        base: PromiseId,
        args: &PromiseBatchAction,
    ) -> PromiseId {
        let id = self.inner.promise_attach_batch_callback(base, args);
        self.promises.push(id);
        id
    }

    fn promise_return(&mut self, promise: PromiseId) {
        self.inner.promise_return(promise);
    }

    fn read_only(&self) -> Self::ReadOnly {
        self.inner.read_only()
    }
}

#[cfg(test)]
mod tests {
    use aurora_engine_types::{account_id::AccountId, types::Yocto};

    #[test]
    fn test_withdraw_to_near_encoding() {
        let recipient: AccountId = "some_account.near".parse().unwrap();
        let recipient_with_msg = format!("{recipient}:unwrap");
        let amount = Yocto::new(1332654);
        #[allow(deprecated)]
        let withdraw_function = ethabi::Function {
            name: "withdrawToNear".into(),
            inputs: vec![
                ethabi::Param {
                    name: "recipient".into(),
                    kind: ethabi::ParamType::Bytes,
                    internal_type: None,
                },
                ethabi::Param {
                    name: "amount".into(),
                    kind: ethabi::ParamType::Uint(256),
                    internal_type: None,
                },
            ],
            outputs: vec![],
            constant: None,
            state_mutability: ethabi::StateMutability::NonPayable,
        };
        let expected_tx_data = withdraw_function
            .encode_input(&[
                ethabi::Token::Bytes(recipient_with_msg.into_bytes()),
                ethabi::Token::Uint(amount.as_u128().into()),
            ])
            .unwrap();

        assert_eq!(
            super::withdraw_to_near_args(&recipient, amount),
            expected_tx_data
        );
    }
}
