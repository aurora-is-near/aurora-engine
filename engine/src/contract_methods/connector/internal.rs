use crate::contract_methods::connector::admin_controlled::AdminControlled;
use crate::contract_methods::connector::deposit_event::{
    DepositedEvent, FtTransferMessageData, TokenMessageData,
};
use crate::contract_methods::connector::errors;
use crate::contract_methods::connector::fungible_token::{FungibleToken, FungibleTokenOps};
use crate::contract_methods::connector::{
    construct_contract_key, proof_key, ZERO_ATTACHED_BALANCE,
};
use crate::contract_methods::{
    predecessor_address, require_owner_only, require_running, ContractError,
};
use crate::engine::Engine;
use crate::hashchain::with_hashchain;
use crate::prelude::{format, sdk, ToString, Vec};
use crate::state;
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::io::StorageIntermediate;
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_sdk::{env::Env, io::IO};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::{
    BalanceOfCallArgs, BalanceOfEthCallArgs, FinishDepositCallArgs, FungibleTokenMetadata,
    IsUsedProofCallArgs, PauseEthConnectorCallArgs, PausedMask, Proof, StorageBalanceOfCallArgs,
    WithdrawResult,
};
use aurora_engine_types::parameters::engine::errors::ParseArgsError;
use aurora_engine_types::parameters::engine::SubmitResult;
use aurora_engine_types::parameters::{PromiseBatchAction, PromiseCreateArgs, WithdrawCallArgs};
use aurora_engine_types::storage::EthConnectorStorageId;
use aurora_engine_types::types::address::error::AddressError;
use aurora_engine_types::types::error::BalanceOverflowError;
use aurora_engine_types::types::{NEP141Wei, NearGas, Wei, ZERO_NEP141_WEI};
use aurora_engine_types::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    parameters::{
        connector::{
            InitCallArgs, NEP141FtOnTransferArgs, ResolveTransferCallArgs, SetContractDataCallArgs,
            StorageDepositCallArgs, StorageWithdrawCallArgs, TransferCallArgs,
            TransferCallCallArgs,
        },
        PromiseWithCallbackArgs,
    },
    types::{Address, PromiseResult, Yocto},
};
use function_name::named;

/// NEAR Gas for calling `finish_deposit` promise. Used in the `deposit` logic.
pub const GAS_FOR_FINISH_DEPOSIT: NearGas = NearGas::new(50_000_000_000_000);
/// NEAR Gas for calling `verify_log_entry` promise. Used in the `deposit` logic.
// Note: Is 40 TGas always enough?
const GAS_FOR_VERIFY_LOG_ENTRY: NearGas = NearGas::new(40_000_000_000_000);

/// Admin control flow flag indicates that all control flow unpause (unblocked).
pub const UNPAUSE_ALL: PausedMask = 0;
/// Admin control flow flag indicates that the deposit is paused.
pub const PAUSE_DEPOSIT: PausedMask = 1 << 0;
/// Admin control flow flag indicates that withdrawal is paused.
pub const PAUSE_WITHDRAW: PausedMask = 1 << 1;
/// Admin control flow flag indicates that ft transfers are paused.
pub const PAUSE_FT: PausedMask = 1 << 2;

#[named]
pub fn new_eth_connector<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        // Only the owner can initialize the EthConnector
        let is_private = env.assert_private_call();
        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }

        let args: InitCallArgs = io.read_input_borsh()?;
        let owner_id = env.current_account_id();

        EthConnectorContract::create_contract(io, &owner_id, args)?;
        Ok(())
    })
}

#[named]
pub fn set_eth_connector_contract_data<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        // Only the owner can set the EthConnector contract data
        let is_private = env.assert_private_call();
        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }

        let args: SetContractDataCallArgs = io.read_input_borsh()?;
        set_contract_data(&mut io, args)?;
        Ok(())
    })
}

#[named]
pub fn withdraw<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_one_yocto()?;
        let args = io.read_input_borsh()?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let result = EthConnectorContract::init(io)?.withdraw_eth_from_near(
            &current_account_id,
            &predecessor_account_id,
            &args,
        )?;
        let result_bytes = borsh::to_vec(&result).map_err(|_| crate::errors::ERR_SERIALIZE)?;

        // We only return the output via IO in the case of standalone.
        // In the case of contract we intentionally avoid IO to call Wasm directly.
        #[cfg(not(feature = "contract"))]
        {
            let mut io = io;
            io.return_output(&result_bytes);
        }

        #[allow(clippy::as_conversions)]
        #[cfg(feature = "contract")]
        unsafe {
            crate::contract::exports::value_return(
                u64::try_from(result_bytes.len()).unwrap(), // sdk_unwrap(),
                result_bytes.as_ptr() as u64,
            );
        }

        Ok(())
    })
}

#[named]
pub fn deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        let raw_proof = io.read_input().to_vec();
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let promise_args = EthConnectorContract::init(io)?.deposit(
            raw_proof,
            current_account_id,
            predecessor_account_id,
        )?;
        // Safety: this call is safe because it comes from the eth-connector, not users.
        // The call is to verify the user-supplied proof for the deposit, with `finish_deposit`
        // as a callback.
        let promise_id = unsafe { handler.promise_create_with_callback(&promise_args) };
        handler.promise_return(promise_id);

        Ok(Some(promise_args))
    })
}

#[named]
pub fn ft_transfer<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_one_yocto()?;
        let predecessor_account_id = env.predecessor_account_id();
        let args: TransferCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseArgsError>::into)?;
        EthConnectorContract::init(io)?.ft_transfer(&predecessor_account_id, &args)?;
        Ok(())
    })
}

#[named]
pub fn ft_transfer_call<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        // Check is payable
        env.assert_one_yocto()?;

        let args: TransferCallCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseArgsError>::into)?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let promise_args = EthConnectorContract::init(io)?.ft_transfer_call(
            predecessor_account_id,
            current_account_id,
            args,
            env.prepaid_gas(),
        )?;
        // Safety: this call is safe. It is required by the NEP-141 spec that `ft_transfer_call`
        // creates a call to another contract's `ft_on_transfer` method.
        let promise_id = unsafe { handler.promise_create_with_callback(&promise_args) };
        handler.promise_return(promise_id);

        Ok(Some(promise_args))
    })
}

#[named]
pub fn ft_on_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<SubmitResult>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let mut engine: Engine<_, E, AuroraModExp> = Engine::new_with_state(
            state,
            predecessor_address(&predecessor_account_id),
            current_account_id.clone(),
            io,
            env,
        );

        let args: NEP141FtOnTransferArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseArgsError>::into)?;

        let output = if predecessor_account_id == current_account_id {
            EthConnectorContract::init(io)?.ft_on_transfer(&args)?;
            None
        } else {
            let result = engine.receive_erc20_tokens(
                &predecessor_account_id,
                &args,
                &current_account_id,
                handler,
            );
            result.ok()
        };
        Ok(output)
    })
}

#[named]
pub fn ft_resolve_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        env.assert_private_call()?;
        if handler.promise_results_count() != 1 {
            return Err(crate::errors::ERR_PROMISE_COUNT.into());
        }

        let args: ResolveTransferCallArgs = io.read_input().to_value()?;
        let promise_result = handler
            .promise_result(0)
            .ok_or(crate::errors::ERR_PROMISE_ENCODING)?;

        EthConnectorContract::init(io)?.ft_resolve_transfer(&args, promise_result);
        Ok(())
    })
}

#[named]
pub fn storage_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        let args: StorageDepositCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseArgsError>::into)?;
        let predecessor_account_id = env.predecessor_account_id();
        let amount = Yocto::new(env.attached_deposit());
        let maybe_promise = EthConnectorContract::init(io)?.storage_deposit(
            predecessor_account_id,
            amount,
            args,
        )?;
        if let Some(promise) = maybe_promise {
            // Safety: This call is safe. It is only a transfer back to the user in the case
            // that they over paid for their deposit.
            unsafe { handler.promise_create_batch(&promise) };
        }
        Ok(())
    })
}

#[named]
pub fn storage_unregister<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_one_yocto()?;
        let predecessor_account_id = env.predecessor_account_id();
        let force = serde_json::from_slice::<serde_json::Value>(&io.read_input().to_vec())
            .ok()
            .and_then(|args| args["force"].as_bool());
        let maybe_promise =
            EthConnectorContract::init(io)?.storage_unregister(predecessor_account_id, force)?;
        if let Some(promise) = maybe_promise {
            // Safety: This call is safe. It is only a transfer back to the user for their deposit.
            unsafe { handler.promise_create_batch(&promise) };
        }
        Ok(())
    })
}

#[named]
pub fn storage_withdraw<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_one_yocto()?;
        let args: StorageWithdrawCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseArgsError>::into)?;
        let predecessor_account_id = env.predecessor_account_id();
        EthConnectorContract::init(io)?.storage_withdraw(&predecessor_account_id, &args)?;
        Ok(())
    })
}

pub fn storage_balance_of<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    let args: StorageBalanceOfCallArgs =
        serde_json::from_slice(&io.read_input().to_vec()).map_err(Into::<ParseArgsError>::into)?;
    EthConnectorContract::init(io)?.storage_balance_of(&args);

    Ok(())
}

#[named]
pub fn set_paused_flags<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let is_private = env.assert_private_call();
        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }
        let args: PauseEthConnectorCallArgs = io.read_input_borsh()?;
        EthConnectorContract::init(io)?.set_paused_flags(&args);
        Ok(())
    })
}

pub fn get_paused_flags<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let paused_flags = EthConnectorContract::init(io)?.get_paused_flags();
    let data = borsh::to_vec(&paused_flags).unwrap();
    io.return_output(&data);

    Ok(())
}

pub fn is_used_proof<I: IO + Copy + PromiseHandler>(mut io: I) -> Result<(), ContractError> {
    let args: IsUsedProofCallArgs = io.read_input_borsh()?;

    let is_used_proof = EthConnectorContract::init(io)?.is_used_proof(&args.proof);
    let res = borsh::to_vec(&is_used_proof).unwrap();
    io.return_output(&res);

    Ok(())
}

pub fn ft_total_eth_supply_on_near<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    EthConnectorContract::init(io)?.ft_total_eth_supply_on_near();
    Ok(())
}

pub fn ft_balance_of<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    let args: BalanceOfCallArgs =
        serde_json::from_slice(&io.read_input().to_vec()).map_err(Into::<ParseArgsError>::into)?;
    EthConnectorContract::init(io)?.ft_balance_of(&args);
    Ok(())
}

#[cfg(not(feature = "ext-connector"))]
pub fn ft_balances_of<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    let accounts: Vec<AccountId> = io.read_input_borsh()?;
    EthConnectorContract::init(io)?.ft_balances_of(accounts);
    Ok(())
}

#[named]
pub fn finish_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_private_call()?;

        // Check result from proof verification call
        if handler.promise_results_count() != 1 {
            return Err(crate::errors::ERR_PROMISE_COUNT.into());
        }
        let promise_result = match handler.promise_result(0) {
            Some(PromiseResult::Successful(bytes)) => {
                bool::try_from_slice(&bytes).map_err(|_| crate::errors::ERR_PROMISE_ENCODING)?
            }
            _ => return Err(crate::errors::ERR_PROMISE_FAILED.into()),
        };
        if !promise_result {
            return Err(crate::errors::ERR_VERIFY_PROOF.into());
        }

        let data = io.read_input_borsh()?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let maybe_promise_args = EthConnectorContract::init(io)?.finish_deposit(
            predecessor_account_id,
            current_account_id,
            data,
            env.prepaid_gas(),
        )?;

        if let Some(promise_args) = maybe_promise_args.as_ref() {
            // Safety: this call is safe because it comes from the eth-connector, not users.
            // The call will be to the Engine's ft_transfer_call`, which is needed as part
            // of the bridge flow (if depositing ETH to an Aurora address).
            let promise_id = unsafe { handler.promise_create_with_callback(promise_args) };
            handler.promise_return(promise_id);
        }

        Ok(maybe_promise_args)
    })
}

pub fn get_accounts_counter<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    EthConnectorContract::init(io)?.get_accounts_counter();
    Ok(())
}

pub fn ft_metadata<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let metadata = get_metadata(&io).unwrap_or_default();
    io.return_output(&serde_json::to_vec(&metadata).unwrap_or_default());

    Ok(())
}

/// Sets the contract data and returns it back
fn set_contract_data<I: IO>(
    io: &mut I,
    args: SetContractDataCallArgs,
) -> Result<EthConnector, AddressError> {
    // Get initial contract arguments
    let contract_data = EthConnector {
        prover_account: args.prover_account,
        eth_custodian_address: Address::decode(&args.eth_custodian_address)?,
    };
    // Save eth-connector specific data
    io.write_borsh(
        &construct_contract_key(EthConnectorStorageId::Contract),
        &contract_data,
    );

    io.write_borsh(
        &construct_contract_key(EthConnectorStorageId::FungibleTokenMetadata),
        &args.metadata,
    );

    Ok(contract_data)
}

/// Return FT metadata.
fn get_metadata<I: IO>(io: &I) -> Option<FungibleTokenMetadata> {
    io.read_storage(&construct_contract_key(
        EthConnectorStorageId::FungibleTokenMetadata,
    ))
    .and_then(|data| data.to_value().ok())
}

/// Eth-connector contract data. It's stored in the storage.
/// Contains:
/// * connector specific data
/// * Fungible token data
/// * `paused_mask` - admin control flow data
/// * io - I/O trait handler
pub struct EthConnectorContract<I: IO> {
    contract: EthConnector,
    ft: FungibleTokenOps<I>,
    paused_mask: PausedMask,
    io: I,
}

/// Eth connector specific data. It always must contain `prover_account` - account id of the smart
/// contract which is used for verifying a proof used in the deposit flow.
#[derive(BorshSerialize, BorshDeserialize)]
#[borsh(crate = "aurora_engine_types::borsh")]
pub struct EthConnector {
    /// The account id of the Prover NEAR smart contract. It used in the Deposit flow for verifying
    /// a log entry from incoming proof.
    pub prover_account: AccountId,
    /// It is Ethereum address used in the Deposit and Withdraw logic.
    pub eth_custodian_address: Address,
}

impl<I: IO + Copy> EthConnectorContract<I> {
    /// Init Eth-connector contract instance.
    /// Load contract data from storage and init I/O handler.
    /// Used as single point of contract access for various contract actions
    pub fn init(io: I) -> Result<Self, errors::StorageReadError> {
        Ok(Self {
            contract: super::get_contract_data(&io, EthConnectorStorageId::Contract)?,
            ft: super::get_contract_data::<FungibleToken, I>(
                &io,
                EthConnectorStorageId::FungibleToken,
            )?
            .ops(io),
            paused_mask: super::get_contract_data(&io, EthConnectorStorageId::PausedMask)?,
            io,
        })
    }

    /// Create contract data - init eth-connector contract specific data.
    /// Used only once for first time initialization.
    /// Initialized contract data stored in the storage.
    pub fn create_contract(
        mut io: I,
        owner_id: &AccountId,
        args: InitCallArgs,
    ) -> Result<(), errors::InitContractError> {
        // Check is it already initialized
        let contract_key_exists =
            io.storage_has_key(&construct_contract_key(EthConnectorStorageId::Contract));
        if contract_key_exists {
            return Err(errors::InitContractError::AlreadyInitialized);
        }

        sdk::log!("[init contract]");

        let contract_data = set_contract_data(
            &mut io,
            SetContractDataCallArgs {
                prover_account: args.prover_account,
                eth_custodian_address: args.eth_custodian_address,
                metadata: args.metadata,
            },
        )
        .map_err(errors::InitContractError::InvalidCustodianAddress)?;

        let mut ft = FungibleTokenOps::new(io);
        // Register FT account for current contract
        ft.internal_register_account(owner_id);

        let paused_mask = UNPAUSE_ALL;
        io.write_borsh(
            &construct_contract_key(EthConnectorStorageId::PausedMask),
            &paused_mask,
        );

        Self {
            contract: contract_data,
            ft,
            paused_mask,
            io,
        }
        .save_ft_contract();

        Ok(())
    }

    /// Deposit all types of tokens.
    pub fn deposit(
        &self,
        raw_proof: Vec<u8>,
        current_account_id: AccountId,
        predecessor_account_id: AccountId,
    ) -> Result<PromiseWithCallbackArgs, errors::DepositError> {
        // Check if the current account is owner.
        let is_owner = current_account_id == predecessor_account_id;
        // Check if the deposit flow isn't paused. If it's owner just skip it.
        self.assert_not_paused(PAUSE_DEPOSIT, is_owner)
            .map_err(|_| errors::DepositError::Paused)?;

        sdk::log!("[Deposit tokens]");

        // Get incoming deposit arguments
        let proof = Proof::try_from_slice(&raw_proof)
            .map_err(|_| errors::DepositError::ProofParseFailed)?;
        // Fetch event data from Proof
        let event = DepositedEvent::from_log_entry_data(&proof.log_entry_data)
            .map_err(errors::DepositError::EventParseFailed)?;

        sdk::log!(
            "Deposit started: from {} to recipient {:?} with amount: {:?} and fee {:?}",
            event.sender.encode(),
            event.token_message_data.recipient(),
            event.amount,
            event.fee
        );

        sdk::log!(
            "Event's address {}, custodian address {}",
            event.eth_custodian_address.encode(),
            self.contract.eth_custodian_address.encode(),
        );

        if event.eth_custodian_address != self.contract.eth_custodian_address {
            return Err(errors::DepositError::CustodianAddressMismatch);
        }

        if NEP141Wei::new(event.fee.as_u128()) >= event.amount {
            return Err(errors::DepositError::InsufficientAmountForFee);
        }

        // Verify the proof data by sending cross-contract call to the prover smart contract.
        sdk::log!(
            "Deposit verify_log_entry for prover: {}",
            self.contract.prover_account,
        );

        // Do not skip bridge call. This is only used for development and diagnostics.
        let skip_bridge_call = borsh::to_vec(&false).unwrap();
        let proof_to_verify = [raw_proof, skip_bridge_call].concat();

        let verify_call = PromiseCreateArgs {
            target_account_id: self.contract.prover_account.clone(),
            method: "verify_log_entry".to_string(),
            args: proof_to_verify,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_VERIFY_LOG_ENTRY,
        };

        // Finalize deposit
        let data = match event.token_message_data {
            // Deposit to NEAR accounts
            TokenMessageData::Near(account_id) => borsh::to_vec(&FinishDepositCallArgs {
                new_owner_id: account_id,
                amount: event.amount,
                proof_key: proof_key(&proof),
                relayer_id: predecessor_account_id,
                fee: event.fee,
                msg: None,
            })
            .unwrap(),
            // Deposit to Eth accounts
            // fee is being minted in the `ft_on_transfer` callback method
            TokenMessageData::Eth {
                receiver_id,
                message,
            } => {
                // Transfer to self and then transfer ETH in `ft_on_transfer`
                // address - is NEAR account
                let transfer_data = borsh::to_vec(&TransferCallCallArgs {
                    receiver_id,
                    amount: event.amount,
                    memo: None,
                    msg: message.encode(),
                })
                .unwrap();

                // Send to self - current account id
                borsh::to_vec(&FinishDepositCallArgs {
                    new_owner_id: current_account_id.clone(),
                    amount: event.amount,
                    proof_key: proof_key(&proof),
                    relayer_id: predecessor_account_id,
                    fee: event.fee,
                    msg: Some(transfer_data),
                })
                .unwrap()
            }
        };

        let finish_call = PromiseCreateArgs {
            target_account_id: current_account_id,
            method: "finish_deposit".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH_DEPOSIT,
        };
        Ok(PromiseWithCallbackArgs {
            base: verify_call,
            callback: finish_call,
        })
    }

    /// Finish deposit flow (private method).
    /// NOTE: In the mint methods could occur an error while calculating the amount to be
    /// credited, and therefore we invoke `record_proof` after mint methods to avoid saving
    /// proof before minting which could be potentially finished with error.
    pub fn finish_deposit(
        &mut self,
        predecessor_account_id: AccountId,
        current_account_id: AccountId,
        data: FinishDepositCallArgs,
        prepaid_gas: NearGas,
    ) -> Result<Option<PromiseWithCallbackArgs>, errors::FinishDepositError> {
        sdk::log!("Finish deposit with the amount: {}", data.amount);

        // Mint tokens to recipient minus fee
        if let Some(msg) = data.msg {
            // Mint - calculate new balances
            self.mint_eth_on_near(&data.new_owner_id, data.amount)?;
            // Store proof only after `mint` calculations
            self.record_proof(&data.proof_key)?;
            // Save new contract data
            self.save_ft_contract();
            let transfer_call_args = TransferCallCallArgs::try_from_slice(&msg).unwrap();
            let promise = self.ft_transfer_call(
                predecessor_account_id,
                current_account_id,
                transfer_call_args,
                prepaid_gas,
            )?;
            Ok(Some(promise))
        } else {
            // Mint - calculate new balances
            self.mint_eth_on_near(
                &data.new_owner_id,
                // it's safe subtracting because we check that the amount is greater than fee in
                // the deposit method.
                data.amount - NEP141Wei::new(data.fee.as_u128()),
            )?;
            self.mint_eth_on_near(&data.relayer_id, NEP141Wei::new(data.fee.as_u128()))?;
            // Store proof only after `mint` calculations
            self.record_proof(&data.proof_key)?;
            // Save new contract data
            self.save_ft_contract();
            Ok(None)
        }
    }

    /// Internal `ETH` withdraw (ETH on Aurora).
    pub(crate) fn internal_remove_eth(&mut self, amount: Wei) -> Result<(), errors::WithdrawError> {
        self.burn_eth_on_aurora(amount)?;
        self.save_ft_contract();
        Ok(())
    }

    /// Record hash of the used proof in the storage.
    fn record_proof(&mut self, key: &str) -> Result<(), errors::ProofUsed> {
        sdk::log!("Record proof: {}", key);

        if self.is_used_event(key) {
            return Err(errors::ProofUsed);
        }

        self.save_used_event(key);
        Ok(())
    }

    ///  Mint `nETH` tokens (ETH on NEAR).
    fn mint_eth_on_near(
        &mut self,
        owner_id: &AccountId,
        amount: NEP141Wei,
    ) -> Result<(), errors::DepositError> {
        sdk::log!("Mint {} nETH tokens for: {}", amount, owner_id);

        if self.ft.get_account_eth_balance(owner_id).is_none() {
            self.ft.accounts_insert(owner_id, ZERO_NEP141_WEI);
        }
        self.ft.internal_deposit_eth_to_near(owner_id, amount)
    }

    ///  Mint `ETH` tokens (ETH on Aurora).
    fn mint_eth_on_aurora(
        &mut self,
        address: Address,
        amount: Wei,
    ) -> Result<(), errors::DepositError> {
        sdk::log!("Mint {} ETH tokens for: {}", amount, address.encode());
        self.ft.internal_deposit_eth_to_aurora(address, amount)
    }

    /// Burn `ETH` tokens (ETH on Aurora).
    fn burn_eth_on_aurora(&mut self, amount: Wei) -> Result<(), errors::WithdrawError> {
        self.ft.internal_withdraw_eth_from_aurora(amount)
    }

    /// Withdraw `nETH` from NEAR accounts
    /// NOTE: it should be without any log data
    pub fn withdraw_eth_from_near(
        &mut self,
        current_account_id: &AccountId,
        predecessor_account_id: &AccountId,
        args: &WithdrawCallArgs,
    ) -> Result<WithdrawResult, errors::WithdrawError> {
        // Check if the current account id is owner.
        let is_owner = current_account_id == predecessor_account_id;
        // Check if the withdraw flow is paused. If it's owner just skip the assertion.
        self.assert_not_paused(PAUSE_WITHDRAW, is_owner)
            .map_err(|_| errors::WithdrawError::Paused)?;

        // Burn tokens to recipient
        self.ft
            .internal_withdraw_eth_from_near(predecessor_account_id, args.amount)?;
        // Save new contract data
        self.save_ft_contract();

        Ok(WithdrawResult {
            recipient_id: args.recipient_address,
            amount: args.amount,
            eth_custodian_address: self.contract.eth_custodian_address,
        })
    }

    /// Returns total `nETH` supply (ETH on NEAR).
    pub fn ft_total_eth_supply_on_near(&mut self) {
        let total_supply = self.ft.ft_total_eth_supply_on_near();
        sdk::log!("Total ETH supply on NEAR: {}", total_supply);
        self.io
            .return_output(format!("\"{total_supply}\"").as_bytes());
    }

    /// Returns total `ETH` supply (ETH on Aurora).
    pub fn ft_total_eth_supply_on_aurora(&mut self) {
        let total_supply = self.ft.ft_total_eth_supply_on_aurora();
        sdk::log!("Total ETH supply on Aurora: {}", total_supply);
        self.io
            .return_output(format!("\"{total_supply}\"").as_bytes());
    }

    /// Return `nETH` balance (ETH on NEAR).
    pub fn ft_balance_of(&mut self, args: &BalanceOfCallArgs) {
        let balance = self.ft.ft_balance_of(&args.account_id);
        sdk::log!("Balance of nETH [{}]: {}", args.account_id, balance);

        self.io.return_output(format!("\"{balance}\"").as_bytes());
    }

    /// Return `nETH` balances for accounts (ETH on NEAR).
    pub fn ft_balances_of(&mut self, accounts: Vec<AccountId>) {
        let mut balances = aurora_engine_types::HashMap::new();
        for account_id in accounts {
            let balance = self.ft.ft_balance_of(&account_id);
            balances.insert(account_id, balance);
        }
        self.io.return_output(&borsh::to_vec(&balances).unwrap());
    }

    /// Return `ETH` balance (ETH on Aurora).
    pub fn ft_balance_of_eth_on_aurora(
        &mut self,
        args: &BalanceOfEthCallArgs,
    ) -> Result<(), BalanceOverflowError> {
        let balance = self
            .ft
            .internal_unwrap_balance_of_eth_on_aurora(&args.address);
        sdk::log!("Balance of ETH [{}]: {}", args.address.encode(), balance);
        self.io.return_output(format!("\"{balance}\"").as_bytes());
        Ok(())
    }

    /// Transfer `nETH` between NEAR accounts.
    pub fn ft_transfer(
        &mut self,
        predecessor_account_id: &AccountId,
        args: &TransferCallArgs,
    ) -> Result<(), errors::TransferError> {
        self.assert_not_paused(PAUSE_FT, false)
            .map_err(|_| errors::TransferError::Paused)?;

        self.ft.internal_transfer_eth_on_near(
            predecessor_account_id,
            &args.receiver_id,
            args.amount,
            &args.memo,
        )?;
        self.save_ft_contract();
        sdk::log!(
            "Transfer amount {} to {} success with memo: {:?}",
            args.amount,
            args.receiver_id,
            args.memo
        );
        Ok(())
    }

    /// FT resolve transfer logic
    pub fn ft_resolve_transfer(
        &mut self,
        args: &ResolveTransferCallArgs,
        promise_result: PromiseResult,
    ) {
        let amount = self.ft.ft_resolve_transfer(
            promise_result,
            &args.sender_id,
            &args.receiver_id,
            args.amount,
        );
        sdk::log!(
            "Resolve transfer from {} to {} success",
            args.sender_id,
            args.receiver_id
        );
        // `ft_resolve_transfer` can change `total_supply` so we should save the contract
        self.save_ft_contract();
        self.io.return_output(format!("\"{amount}\"").as_bytes());
    }

    /// FT transfer call from sender account (invoker account) to receiver.
    /// We start early checking for message data to avoid `ft_on_transfer` call panics.
    /// But we don't check if the relayer exists. If the relayer doesn't exist we simply
    /// do not mint/burn the fee amount.
    /// We allow empty messages for cases when `receiver_id =! current_account_id`.
    pub fn ft_transfer_call(
        &mut self,
        predecessor_account_id: AccountId,
        current_account_id: AccountId,
        args: TransferCallCallArgs,
        prepaid_gas: NearGas,
    ) -> Result<PromiseWithCallbackArgs, errors::FtTransferCallError> {
        self.assert_not_paused(PAUSE_FT, false)
            .map_err(|_| errors::FtTransferCallError::Paused)?;

        sdk::log!(
            "Transfer call to {} amount {}",
            args.receiver_id,
            args.amount,
        );

        // Verify message data before `ft_on_transfer` call to avoid verification panics
        // It's allowed empty message if `receiver_id =! current_account_id`
        if args.receiver_id == current_account_id {
            let message_data = FtTransferMessageData::parse_on_transfer_message(&args.msg)
                .map_err(errors::FtTransferCallError::MessageParseFailed)?;

            // Additional check for overflowing before `ft_on_transfer` calling.
            // But skip checking for overflowing for the relayer.
            // Note: It couldn't be overflowed because the total supply isn't changed during
            // the transfer.
            let amount_for_check = self
                .ft
                .internal_unwrap_balance_of_eth_on_aurora(&message_data.recipient);
            if amount_for_check
                .checked_add(Wei::from(args.amount))
                .is_none()
            {
                return Err(errors::FtTransferCallError::Transfer(
                    errors::TransferError::BalanceOverflow,
                ));
            }
            if self
                .ft
                .total_eth_supply_on_aurora
                .checked_add(Wei::from(args.amount))
                .is_none()
            {
                return Err(errors::FtTransferCallError::Transfer(
                    errors::TransferError::TotalSupplyOverflow,
                ));
            }
        }

        self.ft
            .ft_transfer_call(
                predecessor_account_id,
                args.receiver_id,
                args.amount,
                &args.memo,
                args.msg,
                current_account_id,
                prepaid_gas,
            )
            .map_err(Into::into)
    }

    /// FT storage deposit logic.
    pub fn storage_deposit(
        &mut self,
        predecessor_account_id: AccountId,
        amount: Yocto,
        args: StorageDepositCallArgs,
    ) -> Result<Option<PromiseBatchAction>, errors::StorageFundingError> {
        self.assert_not_paused(PAUSE_FT, false)
            .map_err(|_| errors::StorageFundingError::Paused)?;

        let account_id = args
            .account_id
            .unwrap_or_else(|| predecessor_account_id.clone());
        let (res, maybe_promise) = self.ft.storage_deposit(
            predecessor_account_id,
            &account_id,
            amount,
            args.registration_only,
        )?;
        self.save_ft_contract();
        self.io.return_output(&res.to_json_bytes());
        Ok(maybe_promise)
    }

    /// FT storage unregister.
    pub fn storage_unregister(
        &mut self,
        account_id: AccountId,
        force: Option<bool>,
    ) -> Result<Option<PromiseBatchAction>, errors::StorageFundingError> {
        self.assert_not_paused(PAUSE_FT, false)
            .map_err(|_| errors::StorageFundingError::Paused)?;

        let promise = match self.ft.internal_storage_unregister(account_id, force) {
            Ok((_, p)) => {
                self.io.return_output(b"true");
                Some(p)
            }
            Err(errors::StorageFundingError::NotRegistered) => {
                self.io.return_output(b"false");
                None
            }
            Err(other) => return Err(other),
        };
        Ok(promise)
    }

    /// FT storage withdraw.
    pub fn storage_withdraw(
        &mut self,
        account_id: &AccountId,
        args: &StorageWithdrawCallArgs,
    ) -> Result<(), errors::StorageFundingError> {
        let res = self.ft.storage_withdraw(account_id, args.amount)?;
        self.save_ft_contract();
        self.io.return_output(&res.to_json_bytes());
        Ok(())
    }

    /// Get the balance used by usage of the storage.
    pub fn storage_balance_of(&mut self, args: &StorageBalanceOfCallArgs) {
        self.io
            .return_output(&self.ft.storage_balance_of(&args.account_id).to_json_bytes());
    }

    /// `ft_on_transfer` callback function.
    pub fn ft_on_transfer(
        &mut self,
        args: &NEP141FtOnTransferArgs,
    ) -> Result<(), errors::FtTransferCallError> {
        sdk::log!("Call ft_on_transfer");
        // Parse message with specific rules
        let message_data = FtTransferMessageData::parse_on_transfer_message(&args.msg)
            .map_err(errors::FtTransferCallError::MessageParseFailed)?;
        let amount = Wei::new_u128(args.amount.as_u128());

        self.mint_eth_on_aurora(message_data.recipient, amount)?;
        self.save_ft_contract();
        self.io.return_output(b"\"0\"");
        Ok(())
    }

    /// Return account counter.
    /// It represents total unique accounts (all-time, including accounts which now have zero balance).
    pub fn get_accounts_counter(&mut self) {
        self.io
            .return_output(&self.ft.get_accounts_counter().to_le_bytes());
    }

    /// Return account id of the prover smart contract.
    pub const fn get_bridge_prover(&self) -> &AccountId {
        &self.contract.prover_account
    }

    /// Save eth-connector fungible token contract data in the storage.
    fn save_ft_contract(&mut self) {
        self.io.write_borsh(
            &construct_contract_key(EthConnectorStorageId::FungibleToken),
            &self.ft.data(),
        );
    }

    /// Generate key for used events from Proof.
    fn used_event_key(key: &str) -> Vec<u8> {
        [
            &construct_contract_key(EthConnectorStorageId::UsedEvent),
            key.as_bytes(),
        ]
        .concat()
    }

    /// Save already used event proof as hash key
    fn save_used_event(&mut self, key: &str) {
        self.io.write_borsh(&Self::used_event_key(key), &0u8);
    }

    /// Check if the event of the proof has already been used.
    fn is_used_event(&self, key: &str) -> bool {
        self.io.storage_has_key(&Self::used_event_key(key))
    }

    /// Check whether the provided proof has already been used.
    pub fn is_used_proof(&self, proof: &Proof) -> bool {
        self.is_used_event(&proof_key(proof))
    }

    /// Get Eth connector paused flags
    pub fn get_paused_flags(&self) -> PausedMask {
        self.get_paused()
    }

    /// Set Eth connector paused flags
    pub fn set_paused_flags(&mut self, args: &PauseEthConnectorCallArgs) {
        self.set_paused(args.paused_mask);
    }
}

impl<I: IO + Copy> AdminControlled for EthConnectorContract<I> {
    /// Get current admin paused status.
    fn get_paused(&self) -> PausedMask {
        self.paused_mask
    }

    /// Set admin paused status.
    fn set_paused(&mut self, paused_mask: PausedMask) {
        self.paused_mask = paused_mask;
        self.io.write_borsh(
            &construct_contract_key(EthConnectorStorageId::PausedMask),
            &self.paused_mask,
        );
    }
}
