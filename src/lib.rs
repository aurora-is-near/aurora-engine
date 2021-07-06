#![feature(array_methods)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![cfg_attr(feature = "log", feature(panic_info_message))]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

use crate::parameters::PromiseCreateArgs;

mod map;
#[cfg(feature = "meta-call")]
pub mod meta_parsing;
pub mod parameters;
pub mod prelude;
pub mod storage;
pub mod transaction;
pub mod types;

mod admin_controlled;
#[cfg_attr(not(feature = "contract"), allow(dead_code))]
mod connector;
mod deposit_event;
pub mod engine;
mod fungible_token;
mod json;
mod log_entry;
mod precompiles;
pub mod sdk;

#[cfg(test)]
mod benches;
mod state;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[cfg(target_arch = "wasm32")]
#[panic_handler]
#[cfg_attr(not(feature = "log"), allow(unused_variables))]
#[no_mangle]
pub unsafe fn on_panic(info: &::core::panic::PanicInfo) -> ! {
    #[cfg(feature = "log")]
    {
        use alloc::{format, string::ToString};
        if let Some(msg) = info.message() {
            let msg = if let Some(log) = info.location() {
                format!("{} [{}]", msg, log)
            } else {
                msg.to_string()
            };
            sdk::panic_utf8(msg.as_bytes());
        } else if let Some(log) = info.location() {
            sdk::panic_utf8(log.to_string().as_bytes());
        }
    }

    ::core::arch::wasm32::unreachable();
}

#[cfg(target_arch = "wasm32")]
#[alloc_error_handler]
#[no_mangle]
pub unsafe fn on_alloc_error(_: core::alloc::Layout) -> ! {
    ::core::arch::wasm32::unreachable();
}

#[cfg(feature = "contract")]
mod contract {
    use borsh::{BorshDeserialize, BorshSerialize};

    use crate::connector::EthConnectorContract;
    use crate::engine::{Engine, EngineState};
    #[cfg(feature = "evm_bully")]
    use crate::parameters::{BeginBlockArgs, BeginChainArgs};
    use crate::parameters::{
        DeployErc20TokenArgs, ExpectUtf8, FunctionCallArgs, GetStorageAtArgs, InitCallArgs,
        IsUsedProofCallArgs, NEP141FtOnTransferArgs, NewCallArgs, PauseEthConnectorCallArgs,
        SetContractDataCallArgs, TransferCallCallArgs, ViewCallArgs,
    };

    use crate::json::parse_json;
    use crate::prelude::{format, Address, ToString, TryInto, H160, H256, U256};
    use crate::sdk;
    use crate::storage::{bytes_to_key, KeyPrefix};
    use crate::types::{
        near_account_to_evm_address, u256_to_arr, SdkExpect, SdkProcess, SdkUnwrap,
        ERR_FAILED_PARSE,
    };

    const CODE_KEY: &[u8; 4] = b"CODE";
    const CODE_STAGE_KEY: &[u8; 10] = b"CODE_STAGE";
    const GAS_OVERFLOW: &str = "ERR_GAS_OVERFLOW";

    ///
    /// ADMINISTRATIVE METHODS
    ///

    /// Sets the configuration for the Engine.
    /// Should be called on deployment.
    #[no_mangle]
    pub extern "C" fn new() {
        if let Ok(state) = Engine::get_state() {
            require_owner_only(&state);
        }

        let args: NewCallArgs = sdk::read_input_borsh().sdk_unwrap();
        Engine::set_state(args.into());
    }

    /// Get version of the contract.
    #[no_mangle]
    pub extern "C" fn get_version() {
        let version = match option_env!("NEAR_EVM_VERSION") {
            Some(v) => v.as_bytes(),
            None => include_bytes!("../VERSION"),
        };
        sdk::return_output(version)
    }

    /// Get owner account id for this contract.
    #[no_mangle]
    pub extern "C" fn get_owner() {
        let state = Engine::get_state().sdk_unwrap();
        sdk::return_output(state.owner_id.as_bytes());
    }

    /// Get bridge prover id for this contract.
    #[no_mangle]
    pub extern "C" fn get_bridge_prover() {
        let state = Engine::get_state().sdk_unwrap();
        sdk::return_output(state.bridge_prover_id.as_bytes());
    }

    /// Get chain id for this contract.
    #[no_mangle]
    pub extern "C" fn get_chain_id() {
        sdk::return_output(&Engine::get_state().sdk_unwrap().chain_id)
    }

    #[no_mangle]
    pub extern "C" fn get_upgrade_index() {
        let state = Engine::get_state().sdk_unwrap();
        let index = internal_get_upgrade_index();
        sdk::return_output(&(index + state.upgrade_delay_blocks).to_le_bytes())
    }

    /// Stage new code for deployment.
    #[no_mangle]
    pub extern "C" fn stage_upgrade() {
        let state = Engine::get_state().sdk_unwrap();
        require_owner_only(&state);
        sdk::read_input_and_store(&bytes_to_key(KeyPrefix::Config, CODE_KEY));
        sdk::write_storage(
            &bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY),
            &sdk::block_index().to_le_bytes(),
        );
    }

    /// Deploy staged upgrade.
    #[no_mangle]
    pub extern "C" fn deploy_upgrade() {
        let state = Engine::get_state().sdk_unwrap();
        let index = internal_get_upgrade_index();
        if sdk::block_index() <= index + state.upgrade_delay_blocks {
            sdk::panic_utf8(b"ERR_NOT_ALLOWED:TOO_EARLY");
        }
        sdk::self_deploy(&bytes_to_key(KeyPrefix::Config, CODE_KEY));
    }

    /// Called as part of the upgrade process (see `sdk::self_deploy`). This function is meant
    /// to make any necessary changes to the state such that it aligns with the newly deployed
    /// code.
    #[no_mangle]
    pub extern "C" fn state_migration() {
        // This function is purposely left empty because we do not have any state migration
        // to do.
    }

    ///
    /// MUTATIVE METHODS
    ///

    /// Deploy code into the EVM.
    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let input = sdk::read_input();
        let mut engine = Engine::new(predecessor_address()).sdk_unwrap();
        Engine::deploy_code_with_input(&mut engine, input)
            .map(|res| res.try_to_vec().sdk_expect("ERR_SERIALIZE"))
            .sdk_process();
        // TODO: charge for storage
    }

    /// Call method on the EVM contract.
    #[no_mangle]
    pub extern "C" fn call() {
        let args: FunctionCallArgs = sdk::read_input_borsh().sdk_unwrap();
        let mut engine = Engine::new(predecessor_address()).sdk_unwrap();
        Engine::call_with_args(&mut engine, args)
            .map(|res| res.try_to_vec().sdk_expect("ERR_SERIALIZE"))
            .sdk_process();
        // TODO: charge for storage
    }

    /// Process signed Ethereum transaction.
    /// Must match CHAIN_ID to make sure it's signed for given chain vs replayed from another chain.
    #[no_mangle]
    pub extern "C" fn submit() {
        use crate::prelude::TryFrom;
        use crate::transaction::EthTransaction;

        let input = sdk::read_input();

        let EthTransaction::Legacy(signed_transaction) =
            EthTransaction::try_from(input.as_slice()).sdk_unwrap();

        let state = Engine::get_state().sdk_unwrap();

        // Validate the chain ID, if provided inside the signature:
        if let Some(chain_id) = signed_transaction.chain_id() {
            if U256::from(chain_id) != U256::from(state.chain_id) {
                sdk::panic_utf8(b"ERR_INVALID_CHAIN_ID");
            }
        }

        // Retrieve the signer of the transaction:
        let sender = signed_transaction
            .sender()
            .sdk_expect("ERR_INVALID_ECDSA_SIGNATURE");

        Engine::check_nonce(&sender, &signed_transaction.transaction.nonce).sdk_unwrap();

        // Check intrinsic gas is covered by transaction gas limit
        match signed_transaction
            .transaction
            .intrinsic_gas(crate::engine::CONFIG)
        {
            None => sdk::panic_utf8(GAS_OVERFLOW.as_bytes()),
            Some(intrinsic_gas) => {
                if signed_transaction.transaction.gas < intrinsic_gas.into() {
                    sdk::panic_utf8(b"ERR_INTRINSIC_GAS")
                }
            }
        }

        // Figure out what kind of a transaction this is, and execute it:
        let mut engine = Engine::new_with_state(state, sender);
        let value = signed_transaction.transaction.value;
        let gas_limit = signed_transaction
            .transaction
            .get_gas_limit()
            .sdk_expect(GAS_OVERFLOW);
        let data = signed_transaction.transaction.data;
        let result = if let Some(receiver) = signed_transaction.transaction.to {
            Engine::call(&mut engine, sender, receiver, value, data, gas_limit)
            // TODO: charge for storage
        } else {
            // Execute a contract deployment:
            Engine::deploy_code(&mut engine, sender, value, data, gas_limit)
            // TODO: charge for storage
        };
        result
            .map(|res| res.try_to_vec().sdk_expect("ERR_SERIALIZE"))
            .sdk_process();
    }

    #[cfg(feature = "meta-call")]
    #[no_mangle]
    pub extern "C" fn meta_call() {
        let input = sdk::read_input();
        let state = Engine::get_state().sdk_unwrap();
        let domain_separator = crate::meta_parsing::near_erc712_domain(U256::from(state.chain_id));
        let meta_call_args = crate::meta_parsing::parse_meta_call(
            &domain_separator,
            &sdk::current_account_id(),
            input,
        )
        .sdk_expect("ERR_META_TX_PARSE");

        Engine::check_nonce(&meta_call_args.sender, &meta_call_args.nonce).sdk_unwrap();

        let mut engine = Engine::new_with_state(state, meta_call_args.sender);
        let result = engine.call(
            meta_call_args.sender,
            meta_call_args.contract_address,
            meta_call_args.value,
            meta_call_args.input,
            u64::MAX, // TODO: is there a gas limit with meta calls?
        );
        result
            .map(|res| res.try_to_vec().sdk_expect("ERR_SERIALIZE"))
            .sdk_process();
    }

    #[no_mangle]
    pub extern "C" fn register_relayer() {
        let relayer_address = sdk::read_input_arr20().sdk_unwrap();

        let mut engine = Engine::new(predecessor_address()).sdk_unwrap();
        engine.register_relayer(
            sdk::predecessor_account_id().as_slice(),
            Address(relayer_address),
        );
    }

    /// Allow receiving NEP141 tokens to the EVM contract.
    ///
    /// This function returns the amount of tokens to return to the sender.
    /// Either all tokens are transferred tokens are returned in case of an
    /// error, or no token is returned if tx was successful.
    #[no_mangle]
    pub extern "C" fn ft_on_transfer() {
        let mut engine = Engine::new(predecessor_address()).sdk_unwrap();

        let args: NEP141FtOnTransferArgs = parse_json(sdk::read_input().as_slice())
            .sdk_unwrap()
            .try_into()
            .map_err(|err| format!("ERR_JSON_{:?}", err))
            .sdk_unwrap();

        if sdk::predecessor_account_id() == sdk::current_account_id() {
            let engine = Engine::new(predecessor_address()).sdk_unwrap();
            EthConnectorContract::get_instance().ft_on_transfer(&engine, &args);
        } else {
            engine.receive_erc20_tokens(&args);
        }
    }

    /// Deploy ERC20 token mapped to a NEP141
    #[no_mangle]
    pub extern "C" fn deploy_erc20_token() {
        // Id of the NEP141 token in Near
        let args: DeployErc20TokenArgs =
            DeployErc20TokenArgs::try_from_slice(&sdk::read_input()).sdk_expect("ERR_ARG_PARSE");

        let mut engine = Engine::new(predecessor_address()).sdk_unwrap();

        let erc20_contract = include_bytes!("../etc/eth-contracts/res/EvmErc20.bin");
        let deploy_args = ethabi::encode(&[
            ethabi::Token::String("Empty".to_string()),
            ethabi::Token::String("EMPTY".to_string()),
            ethabi::Token::Uint(ethabi::Uint::from(0)),
            ethabi::Token::Address(current_address()),
        ]);

        Engine::deploy_code_with_input(
            &mut engine,
            (&[erc20_contract, deploy_args.as_slice()].concat()).to_vec(),
        )
        .map(|res| {
            let address = H160(res.result.as_slice().try_into().unwrap());
            engine.register_token(address.as_bytes(), args.nep141.as_bytes());
            res.result.try_to_vec().sdk_expect("ERR_SERIALIZE")
        })
        .sdk_process();

        // TODO: charge for storage
    }

    ///
    /// NONMUTATIVE METHODS
    ///

    #[no_mangle]
    pub extern "C" fn view() {
        let args: ViewCallArgs = sdk::read_input_borsh().sdk_unwrap();
        let engine = Engine::new(Address::from_slice(&args.sender)).sdk_unwrap();
        let result = Engine::view_with_args(&engine, args);
        result.sdk_process()
    }

    #[no_mangle]
    pub extern "C" fn get_code() {
        let address = sdk::read_input_arr20().sdk_unwrap();
        let code = Engine::get_code(&Address(address));
        sdk::return_output(&code)
    }

    #[no_mangle]
    pub extern "C" fn get_balance() {
        let address = sdk::read_input_arr20().sdk_unwrap();
        let balance = Engine::get_balance(&Address(address));
        sdk::return_output(&balance.to_bytes())
    }

    #[no_mangle]
    pub extern "C" fn get_nonce() {
        let address = sdk::read_input_arr20().sdk_unwrap();
        let nonce = Engine::get_nonce(&Address(address));
        sdk::return_output(&u256_to_arr(&nonce))
    }

    #[no_mangle]
    pub extern "C" fn get_storage_at() {
        let args: GetStorageAtArgs = sdk::read_input_borsh().sdk_unwrap();
        let address = Address(args.address);
        let generation = Engine::get_generation(&address);
        let value = Engine::get_storage(&Address(args.address), &H256(args.key), generation);
        sdk::return_output(&value.0)
    }

    ///
    /// BENCHMARKING METHODS
    ///

    #[cfg(feature = "evm_bully")]
    #[no_mangle]
    pub extern "C" fn begin_chain() {
        let mut state = Engine::get_state().sdk_unwrap();
        require_owner_only(&state);
        let args: BeginChainArgs = sdk::read_input_borsh().sdk_unwrap();
        state.chain_id = args.chain_id;
        Engine::set_state(state);
        // set genesis block balances
        for account_balance in args.genesis_alloc {
            Engine::set_balance(
                &Address(account_balance.address),
                &crate::types::Wei::new(U256::from(account_balance.balance)),
            )
        }
        // return new chain ID
        sdk::return_output(&Engine::get_state().sdk_unwrap().chain_id)
    }

    #[cfg(feature = "evm_bully")]
    #[no_mangle]
    pub extern "C" fn begin_block() {
        let state = Engine::get_state().sdk_unwrap();
        require_owner_only(&state);
        let _args: BeginBlockArgs = sdk::read_input_borsh().sdk_unwrap();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/2
    }

    #[no_mangle]
    pub extern "C" fn new_eth_connector() {
        // Only the owner can initialize the EthConnector
        sdk::assert_private_call();

        let args = InitCallArgs::try_from_slice(&sdk::read_input()).expect(ERR_FAILED_PARSE);

        EthConnectorContract::init_contract(args);
    }

    #[no_mangle]
    pub extern "C" fn set_eth_connector_contract_data() {
        // Only the owner can set the EthConnector contract data
        sdk::assert_private_call();

        let args =
            SetContractDataCallArgs::try_from_slice(&sdk::read_input()).expect(ERR_FAILED_PARSE);

        EthConnectorContract::set_contract_data(args);
    }

    #[no_mangle]
    pub extern "C" fn withdraw() {
        EthConnectorContract::get_instance().withdraw_eth_from_near()
    }

    #[no_mangle]
    pub extern "C" fn deposit() {
        EthConnectorContract::get_instance().deposit()
    }

    #[no_mangle]
    pub extern "C" fn finish_deposit() {
        EthConnectorContract::get_instance().finish_deposit();
    }

    #[no_mangle]
    pub extern "C" fn is_used_proof() {
        let args = IsUsedProofCallArgs::try_from_slice(&sdk::read_input()).expect(ERR_FAILED_PARSE);

        let is_used_proof = EthConnectorContract::get_instance().is_used_proof(args.proof);
        let res = is_used_proof.try_to_vec().unwrap();
        sdk::return_output(&res[..]);
    }

    #[no_mangle]
    pub extern "C" fn ft_total_supply() {
        EthConnectorContract::get_instance().ft_total_eth_supply_on_near();
    }

    #[no_mangle]
    pub extern "C" fn ft_total_eth_supply_on_near() {
        EthConnectorContract::get_instance().ft_total_eth_supply_on_near();
    }

    #[no_mangle]
    pub extern "C" fn ft_total_eth_supply_on_aurora() {
        EthConnectorContract::get_instance().ft_total_eth_supply_on_aurora();
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of() {
        EthConnectorContract::get_instance().ft_balance_of();
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of_eth() {
        EthConnectorContract::get_instance().ft_balance_of_eth_on_aurora();
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer() {
        EthConnectorContract::get_instance().ft_transfer();
    }

    #[no_mangle]
    pub extern "C" fn ft_resolve_transfer() {
        EthConnectorContract::get_instance().ft_resolve_transfer();
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer_call() {
        // Check is payable
        sdk::assert_one_yocto();

        let args = TransferCallCallArgs::from(
            parse_json(&sdk::read_input()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        );
        EthConnectorContract::get_instance().ft_transfer_call(args);
    }

    #[no_mangle]
    pub extern "C" fn storage_deposit() {
        EthConnectorContract::get_instance().storage_deposit()
    }

    #[no_mangle]
    pub extern "C" fn storage_withdraw() {
        EthConnectorContract::get_instance().storage_withdraw()
    }

    #[no_mangle]
    pub extern "C" fn storage_balance_of() {
        EthConnectorContract::get_instance().storage_balance_of()
    }

    #[no_mangle]
    pub extern "C" fn get_paused_flags() {
        let paused_flags = EthConnectorContract::get_instance().get_paused_flags();
        let data = paused_flags.try_to_vec().expect(ERR_FAILED_PARSE);
        sdk::return_output(&data[..]);
    }

    #[no_mangle]
    pub extern "C" fn set_paused_flags() {
        sdk::assert_private_call();

        let args =
            PauseEthConnectorCallArgs::try_from_slice(&sdk::read_input()).expect(ERR_FAILED_PARSE);
        EthConnectorContract::get_instance().set_paused_flags(args);
    }

    #[no_mangle]
    pub extern "C" fn get_accounts_counter() {
        EthConnectorContract::get_instance().get_accounts_counter()
    }

    #[no_mangle]
    pub extern "C" fn get_erc20_from_nep141() {
        sdk::return_output(
            Engine::nep141_erc20_map()
                .lookup_left(sdk::read_input().as_slice())
                .sdk_expect("NEP141_NOT_FOUND")
                .as_slice(),
        );
    }

    #[no_mangle]
    pub extern "C" fn get_nep141_from_erc20() {
        sdk::return_output(
            Engine::nep141_erc20_map()
                .lookup_right(sdk::read_input().as_slice())
                .sdk_expect("ERC20_NOT_FOUND")
                .as_slice(),
        );
    }

    #[cfg(feature = "integration-test")]
    #[no_mangle]
    pub extern "C" fn verify_log_entry() {
        crate::log!("Call from verify_log_entry");
        let data = true.try_to_vec().unwrap();
        sdk::return_output(&data[..]);
    }

    ///
    /// Utility methods.
    ///

    fn internal_get_upgrade_index() -> u64 {
        match sdk::read_u64(&bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY)) {
            Ok(index) => index,
            Err(sdk::ReadU64Error::InvalidU64) => sdk::panic_utf8(b"ERR_INVALID_UPGRADE"),
            Err(sdk::ReadU64Error::MissingValue) => sdk::panic_utf8(b"ERR_NO_UPGRADE"),
        }
    }

    fn require_owner_only(state: &EngineState) {
        if state.owner_id.as_bytes() != sdk::predecessor_account_id() {
            sdk::panic_utf8(b"ERR_NOT_ALLOWED");
        }
    }

    fn predecessor_address() -> Address {
        near_account_to_evm_address(&sdk::predecessor_account_id())
    }

    pub fn current_address() -> Address {
        near_account_to_evm_address(&sdk::current_account_id())
    }
}

pub trait AuroraState {
    fn add_promise(&mut self, promise: PromiseCreateArgs);
}
