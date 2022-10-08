use crate::parameters::{CallArgs, FunctionCallArgsV2};
use aurora_engine_precompiles::xcc::state;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::{PromiseAction, PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_types::storage::{self, KeyPrefix};
use aurora_engine_types::types::{Address, NearGas, Yocto, ZERO_YOCTO};
use aurora_engine_types::{format, Cow, Vec, U256};
use borsh::{BorshDeserialize, BorshSerialize};

pub const ERR_NO_ROUTER_CODE: &str = "ERR_MISSING_XCC_BYTECODE";
pub const ERR_INVALID_ACCOUNT: &str = "ERR_INVALID_XCC_ACCOUNT";
pub const ERR_ATTACHED_NEAR: &str = "ERR_ATTACHED_XCC_NEAR";
pub const CODE_KEY: &[u8] = b"router_code";
/// Gas costs estimated from simulation tests.
pub const VERSION_UPDATE_GAS: NearGas = NearGas::new(5_000_000_000_000);
pub const INITIALIZE_GAS: NearGas = NearGas::new(15_000_000_000_000);
pub const UNWRAP_AND_REFUND_GAS: NearGas = NearGas::new(25_000_000_000_000);
pub const WITHDRAW_GAS: NearGas = NearGas::new(30_000_000_000_000);
/// Solidity selector for the withdrawToNear function
/// https://www.4byte.directory/signatures/?bytes4_signature=0x6b351848
pub const WITHDRAW_TO_NEAR_SELECTOR: [u8; 4] = [0x6b, 0x35, 0x18, 0x48];

pub use aurora_engine_precompiles::xcc::state::{
    get_code_version_of_address, get_latest_code_version, get_wnear_address, CodeVersion,
    ERR_CORRUPTED_STORAGE, STORAGE_AMOUNT, VERSION_KEY, WNEAR_KEY,
};

/// Type wrapper for router bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterCode<'a>(pub Cow<'a, [u8]>);

impl<'a> RouterCode<'a> {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Cow::Owned(bytes))
    }

    pub fn borrowed(bytes: &'a [u8]) -> Self {
        Self(Cow::Borrowed(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct AddressVersionUpdateArgs {
    pub address: Address,
    pub version: CodeVersion,
}

pub fn handle_precompile_promise<I, P>(
    io: &I,
    handler: &mut P,
    promise: PromiseCreateArgs,
    required_near: Yocto,
    current_account_id: &AccountId,
) where
    P: PromiseHandler,
    I: IO + Copy,
{
    let target_account: &str = promise.target_account_id.as_ref();
    let sender = Address::decode(&target_account[0..40]).expect(ERR_INVALID_ACCOUNT);

    // Confirm target_account is of the form `{address}.{aurora}`
    // Address prefix parsed above, so only need to check `.{aurora}`
    assert_eq!(&target_account[40..41], ".", "{}", ERR_INVALID_ACCOUNT);
    assert_eq!(
        &target_account[41..],
        current_account_id.as_ref(),
        "{}",
        ERR_INVALID_ACCOUNT
    );
    // Confirm there is 0 NEAR attached to the promise
    // (the precompile should not drain the engine's balance).
    assert_eq!(
        promise.attached_balance, ZERO_YOCTO,
        "{}",
        ERR_ATTACHED_NEAR
    );

    let latest_code_version = get_latest_code_version(io);
    let sender_code_version = get_code_version_of_address(io, &sender);
    let deploy_needed = match sender_code_version {
        None => AddressVersionStatus::DeployNeeded {
            create_needed: true,
        },
        Some(version) if version < latest_code_version => AddressVersionStatus::DeployNeeded {
            create_needed: false,
        },
        Some(_version) => AddressVersionStatus::UpToDate,
    };
    // 1. If the router contract account does not exist or is out of date then we start
    //    with a batch transaction to deploy the router. This batch also has an attached
    //    callback to update the engine's storage with the new version of that router account.
    let setup_id = match &deploy_needed {
        AddressVersionStatus::DeployNeeded { create_needed } => {
            let mut promise_actions = Vec::with_capacity(4);
            if *create_needed {
                promise_actions.push(PromiseAction::CreateAccount);
                promise_actions.push(PromiseAction::Transfer {
                    amount: STORAGE_AMOUNT,
                });
            }
            promise_actions.push(PromiseAction::DeployContract {
                code: get_router_code(io).0.into_owned(),
            });
            // After a deploy we call the contract's initialize function
            let wnear_address = state::get_wnear_address(io);
            let wnear_account = crate::engine::nep141_erc20_map(*io)
                .lookup_right(&crate::engine::ERC20Address(wnear_address))
                .unwrap();
            let init_args = format!(
                r#"{{"wnear_account": "{}", "must_register": {}}}"#,
                wnear_account.0.as_ref(),
                create_needed,
            );
            promise_actions.push(PromiseAction::FunctionCall {
                name: "initialize".into(),
                args: init_args.into_bytes(),
                attached_yocto: ZERO_YOCTO,
                gas: INITIALIZE_GAS,
            });
            let batch = PromiseBatchAction {
                target_account_id: promise.target_account_id.clone(),
                actions: promise_actions,
            };
            // Safety: This batch creation is safe because it only acts on the router sub-account
            // (not the main engine account), and the actions performed are only (1) create it
            // for the first time and/or (2) deploy the code from our storage (i.e. the deployed
            // code is controlled by us, not the user).
            let promise_id = unsafe { handler.promise_create_batch(&batch) };
            // Add a callback here to update the version of the account
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

            // Safety: A call from the engine to the engine's `factory_update_address_version`
            // method is safe because that method only writes the specific router sub-account
            // metadata that has just been deployed above.
            unsafe { Some(handler.promise_attach_callback(promise_id, &callback)) }
        }
        AddressVersionStatus::UpToDate => None,
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
        let wnear_address = state::get_wnear_address(io);
        let withdraw_call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: wnear_address,
            value: [0u8; 32],
            input: withdraw_to_near_args(&promise.target_account_id, required_near),
        });
        let withdraw_call = PromiseCreateArgs {
            target_account_id: current_account_id.clone(),
            method: "call".into(),
            args: withdraw_call_args.try_to_vec().unwrap(),
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
        let args = format!(
            r#"{{"amount": "{}", "refund_needed": {}}}"#,
            required_near.as_u128(),
            refund_needed,
        );
        let unwrap_call = PromiseCreateArgs {
            target_account_id: promise.target_account_id.clone(),
            method: "unwrap_and_refund_storage".into(),
            args: args.into_bytes(),
            attached_balance: ZERO_YOCTO,
            attached_gas: UNWRAP_AND_REFUND_GAS,
        };
        // Safety: This call is safe because the router's `unwrap_and_refund_storage` method
        // does not violate any security invariants. It only interacts with the wrap.near contract
        // to obtain NEAR from WNEAR.
        unsafe { Some(handler.promise_attach_callback(id, &unwrap_call)) }
    };
    // 3. Finally we can do the call the user wanted to do.

    // Safety: this call is safe because the promise comes from the XCC precompile, not the
    // user directly. The XCC precompile will only construct promises that target the `execute`
    // and `schedule` methods of the user's router contract. Therefore, the user cannot have
    // the engine make arbitrary calls.
    let _promise_id = unsafe {
        match withdraw_id {
            None => handler.promise_create_call(&promise),
            Some(withdraw_id) => handler.promise_attach_callback(withdraw_id, &promise),
        }
    };
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
    set_latest_code_version(io, current_version.increment());
}

/// Set the address of the wNEAR ERC-20 contract
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

/// Sets the latest router contract version. This function is intentionally private because
/// it should never be set manually. The version is managed automatically by `update_router_code`.
fn set_latest_code_version<I: IO>(io: &mut I, version: CodeVersion) {
    let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, VERSION_KEY);
    let value_bytes = version.0.to_le_bytes();
    io.write_storage(&key, &value_bytes);
}

/// Private enum used for bookkeeping what actions are needed in the call to the router contract.
enum AddressVersionStatus {
    UpToDate,
    DeployNeeded { create_needed: bool },
}

fn withdraw_to_near_args(recipient: &AccountId, amount: Yocto) -> Vec<u8> {
    let args = ethabi::encode(&[
        ethabi::Token::Bytes(recipient.as_bytes().to_vec()),
        ethabi::Token::Uint(U256::from(amount.as_u128())),
    ]);
    [&WITHDRAW_TO_NEAR_SELECTOR, args.as_slice()].concat()
}

#[cfg(test)]
mod tests {
    use aurora_engine_types::{account_id::AccountId, types::Yocto, U256};

    #[test]
    fn test_withdraw_to_near_encoding() {
        let recipient: AccountId = "some_account.near".parse().unwrap();
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
                ethabi::Token::Bytes(recipient.as_bytes().to_vec()),
                ethabi::Token::Uint(U256::from(amount.as_u128())),
            ])
            .unwrap();

        assert_eq!(
            super::withdraw_to_near_args(&recipient, amount),
            expected_tx_data
        );
    }
}
