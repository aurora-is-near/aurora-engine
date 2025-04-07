use crate::contract_methods::connector::deposit_event::FtTransferMessageData;
use crate::contract_methods::connector::{construct_contract_key, errors, ZERO_ATTACHED_BALANCE};
use crate::contract_methods::{
    predecessor_address, require_owner_only, require_running, ContractError,
};
use crate::engine::Engine;
use crate::hashchain::with_hashchain;
use crate::parameters::{BalanceOfEthCallArgs, NEP141FtOnTransferArgs};
use crate::prelude::PromiseCreateArgs;
use crate::prelude::Wei;
use crate::prelude::{
    sdk, AccountId, Address, EthConnectorStorageId, NearGas, ToString, Vec, Yocto,
};
use crate::state;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::borsh;
use aurora_engine_types::parameters::connector::{
    EngineWithdrawCallArgs, InitCallArgs, SetEthConnectorContractAccountArgs,
    StorageDepositCallArgs, StorageUnregisterCallArgs, StorageWithdrawCallArgs, TransferCallArgs,
    TransferCallCallArgs, WithdrawSerializeType,
};
use aurora_engine_types::parameters::engine::errors::ParseArgsError;
use aurora_engine_types::parameters::engine::SubmitResult;
use aurora_engine_types::parameters::{PromiseWithCallbackArgs, WithdrawCallArgs};
use function_name::named;

/// NEAR Gas needed to create promise.
const GAS_FOR_PROMISE_CREATION: NearGas = NearGas::new(2_000_000_000_000);
const ONE_YOCTO: Yocto = Yocto::new(1);

pub fn withdraw<I: IO + Copy + PromiseHandler + Env, E>(
    mut io: I,
    _: E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    io.assert_one_yocto()?;
    let args: WithdrawCallArgs = io.read_input_borsh()?;
    let input = borsh::to_vec(&EngineWithdrawCallArgs {
        sender_id: io.predecessor_account_id(),
        recipient_address: args.recipient_address,
        amount: args.amount,
    })
    .unwrap();

    let promise_args = EthConnectorContract::init_with_env(io)?.withdraw_eth_from_near(input);
    let promise_id = unsafe { io.promise_create_call(&promise_args) };
    io.promise_return(promise_id);

    Ok(())
}

pub fn ft_total_eth_supply_on_near<I: IO + Copy + PromiseHandler + Env>(
    mut io: I,
) -> Result<(), ContractError> {
    let promise_args = EthConnectorContract::init_with_env(io)?.ft_total_eth_supply_on_near();
    let promise_id = unsafe { io.promise_create_call(&promise_args) };
    io.promise_return(promise_id);

    Ok(())
}

pub fn ft_balance_of<I: IO + Copy + PromiseHandler + Env>(mut io: I) -> Result<(), ContractError> {
    let input = io.read_input().to_vec();
    let promise_args = EthConnectorContract::init_with_env(io)?.ft_balance_of(input);
    let promise_id = unsafe { io.promise_create_call(&promise_args) };
    io.promise_return(promise_id);
    Ok(())
}

pub fn ft_transfer<I: IO + Env + Copy + PromiseHandler, E: Env>(
    mut io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;
    let input = read_json_args(&io).and_then(|args: TransferCallArgs| {
        serde_json::to_vec(&(
            env.predecessor_account_id(),
            args.receiver_id,
            args.amount,
            args.memo,
        ))
        .map_err(Into::<ParseArgsError>::into)
    })?;

    let promise_arg = EthConnectorContract::init_with_env(io)?.ft_transfer(input);
    let promise_id = unsafe { io.promise_create_call(&promise_arg) };
    io.promise_return(promise_id);

    Ok(())
}

pub fn ft_transfer_call<I: IO + Env + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    require_running(&state::get_state(&io)?)?;
    // Check is payable
    env.assert_one_yocto()?;
    let input = read_json_args(&io).and_then(|args: TransferCallCallArgs| {
        serde_json::to_vec(&(
            env.predecessor_account_id(),
            args.receiver_id,
            args.amount,
            args.memo,
            args.msg,
        ))
        .map_err(Into::<ParseArgsError>::into)
    })?;

    let promise_args = EthConnectorContract::init_with_env(io)?.ft_transfer_call(input);
    let promise_id = unsafe { handler.promise_create_call(&promise_args) };
    handler.promise_return(promise_id);

    Ok(None)
}

pub fn ft_on_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<SubmitResult>, ContractError> {
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let mut engine: Engine<_, _> = Engine::new(
        predecessor_address(&predecessor_account_id),
        current_account_id.clone(),
        io,
        env,
    )?;

    let args: NEP141FtOnTransferArgs = read_json_args(&io).map_err(Into::<ParseArgsError>::into)?;
    let mut eth_connector = EthConnectorContract::init(io)?;

    let output = if predecessor_account_id == eth_connector.get_eth_connector_contract_account() {
        eth_connector.ft_on_transfer(&args)?;
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
}

pub fn storage_deposit<I: IO + Env + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    let input = read_json_args(&io).and_then(|args: StorageDepositCallArgs| {
        serde_json::to_vec(&(
            env.predecessor_account_id(),
            args.account_id,
            args.registration_only,
        ))
        .map_err(Into::<ParseArgsError>::into)
    })?;

    let promise_args =
        EthConnectorContract::init_with_env(io)?.storage_deposit(input, env.attached_deposit());
    let promise_id = unsafe { handler.promise_create_call(&promise_args) };
    handler.promise_return(promise_id);

    Ok(())
}

pub fn storage_unregister<I: IO + Env + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;

    let input = read_json_args(&io).and_then(|args: StorageUnregisterCallArgs| {
        serde_json::to_vec(&(env.predecessor_account_id(), args.force))
            .map_err(Into::<ParseArgsError>::into)
    })?;

    let promise_args = EthConnectorContract::init_with_env(io)?.storage_unregister(input);
    let promise_id = unsafe { handler.promise_create_call(&promise_args) };
    handler.promise_return(promise_id);

    Ok(())
}

pub fn storage_withdraw<I: IO + Copy + PromiseHandler + Env, E: Env>(
    mut io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;

    let input = read_json_args(&io).and_then(|args: StorageWithdrawCallArgs| {
        serde_json::to_vec(&(env.predecessor_account_id(), args.amount))
            .map_err(Into::<ParseArgsError>::into)
    })?;

    let promise_args = EthConnectorContract::init_with_env(io)?.storage_withdraw(input);
    let promise_id = unsafe { io.promise_create_call(&promise_args) };

    io.promise_return(promise_id);

    Ok(())
}

pub fn storage_balance_of<I: IO + Copy + PromiseHandler + Env>(
    mut io: I,
) -> Result<(), ContractError> {
    let input = io.read_input().to_vec();
    let promise_args = EthConnectorContract::init_with_env(io)?.storage_balance_of(input);
    let promise_id = unsafe { io.promise_create_call(&promise_args) };
    io.promise_return(promise_id);

    Ok(())
}

#[named]
pub fn set_eth_connector_account_id<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let is_private = env.assert_private_call();

        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }

        let args: SetEthConnectorContractAccountArgs = io.read_input_borsh()?;
        let mut connector = EthConnectorContract::init(io)?;

        connector.set_eth_connector_contract_account(&args.account);
        connector.set_withdraw_serialize_type(&args.withdraw_serialize_type);

        Ok(())
    })
}

pub fn get_eth_connector_account_id<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let account = EthConnectorContract::init(io)?.get_eth_connector_contract_account();
    let data = borsh::to_vec(&account).unwrap();
    io.return_output(&data);

    Ok(())
}

pub fn ft_metadata<I: IO + Copy + PromiseHandler + Env>(mut io: I) -> Result<(), ContractError> {
    let promise_args = EthConnectorContract::init_with_env(io)?.get_metadata();
    let promise_id = unsafe { io.promise_create_call(&promise_args) };
    io.promise_return(promise_id);

    Ok(())
}

/// Eth-connector contract. It's stored in the storage.
/// Contains:
/// * connector specific data
/// * Fungible token data
/// * `paused_mask` - admin control flow data
/// * io - I/O trait handler
pub struct EthConnectorContract<I: IO> {
    io: I,
}

impl<I: IO + Copy> EthConnectorContract<I> {
    pub const fn init(io: I) -> Result<Self, errors::StorageReadError> {
        Ok(Self { io })
    }

    /// Create contract data - init eth-connector contract specific data.
    /// Used only once for first time initialization.
    /// Initialized contract data stored in the storage.
    #[allow(clippy::missing_const_for_fn, clippy::needless_pass_by_value)]
    pub fn create_contract(
        _: I,
        _: &AccountId,
        _args: InitCallArgs,
    ) -> Result<(), errors::InitContractError> {
        // NOTE: do nothing
        Ok(())
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
        self.io.return_output(b"\"0\"");

        Ok(())
    }

    ///  Mint ETH tokens
    fn mint_eth_on_aurora(
        &mut self,
        owner_id: Address,
        amount: Wei,
    ) -> Result<(), errors::DepositError> {
        sdk::log!("Mint {} ETH tokens for: {}", amount, owner_id.encode());
        self.internal_deposit_eth_to_aurora(owner_id, amount)
    }

    /// Internal ETH deposit to Aurora
    pub fn internal_deposit_eth_to_aurora(
        &mut self,
        address: Address,
        amount: Wei,
    ) -> Result<(), errors::DepositError> {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(&address);
        let new_balance = balance
            .checked_add(amount)
            .ok_or(errors::DepositError::BalanceOverflow)?;
        crate::engine::set_balance(&mut self.io, &address, &new_balance);
        Ok(())
    }

    /// Return `ETH` balance (ETH on Aurora).
    pub fn ft_balance_of_eth_on_aurora(
        &mut self,
        args: &BalanceOfEthCallArgs,
    ) -> Result<(), ParseArgsError> {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(&args.address);
        sdk::log!("Balance of ETH [{}]: {}", args.address.encode(), balance);
        self.io.return_output(&serde_json::to_vec(&balance)?);
        Ok(())
    }

    /// Balance of ETH (ETH on Aurora)
    pub fn internal_unwrap_balance_of_eth_on_aurora(&self, address: &Address) -> Wei {
        crate::engine::get_balance(&self.io, address)
    }
}

impl<I: IO + Env + Copy> EthConnectorContract<I> {
    /// Init Eth-connector contract instance.
    /// Load contract data from storage and init I/O handler.
    /// Used as single point of contract access for various contract actions
    pub const fn init_with_env(io: I) -> Result<Self, errors::StorageReadError> {
        Ok(Self { io })
    }

    /// Withdraw `nETH` from NEAR accounts
    /// NOTE: it should be without any log data
    pub fn withdraw_eth_from_near(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_withdraw".to_string(),
            args: data,
            attached_balance: ONE_YOCTO,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// Returns total ETH supply on NEAR (`nETH` as NEP-141 token)
    pub fn ft_total_eth_supply_on_near(&mut self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_total_supply".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// Return `nETH` balance (ETH on NEAR).
    pub fn ft_balance_of(&self, input: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_balance_of".to_string(),
            args: input,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// Transfer between NEAR accounts
    pub fn ft_transfer(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_ft_transfer".to_string(),
            args: data,
            attached_balance: ONE_YOCTO,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// FT transfer call from sender account (invoker account) to receiver
    /// We start early checking for message data to avoid `ft_on_transfer` call panics
    /// But we don't check relayer exists. If relayer doesn't exist we simply not mint/burn the amount of the fee
    /// We allow empty messages for cases when `receiver_id =! current_account_id`
    pub fn ft_transfer_call(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_ft_transfer_call".to_string(),
            args: data,
            attached_balance: ONE_YOCTO,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// FT storage deposit logic
    pub fn storage_deposit(&self, data: Vec<u8>, attached_deposit: u128) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_storage_deposit".to_string(),
            args: data,
            attached_balance: Yocto::new(attached_deposit),
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// FT storage unregister
    pub fn storage_unregister(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_storage_unregister".to_string(),
            args: data,
            attached_balance: ONE_YOCTO,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// FT storage withdraw
    pub fn storage_withdraw(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_storage_withdraw".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// Get balance of storage
    pub fn storage_balance_of(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "storage_balance_of".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// Get Eth connector paused flags
    pub fn get_paused_flags(&self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "get_paused_flags".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    /// Return FT metadata
    pub fn get_metadata(&self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_metadata".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: self.calculate_attached_gas(),
        }
    }

    fn calculate_attached_gas(&self) -> NearGas {
        self.io.prepaid_gas() - self.io.used_gas() - GAS_FOR_PROMISE_CREATION
    }
}

pub trait AdminControlled {
    fn get_eth_connector_contract_account(&self) -> AccountId;
    fn set_eth_connector_contract_account(&mut self, account: &AccountId);
    fn get_withdraw_serialize_type(&self) -> WithdrawSerializeType;
    fn set_withdraw_serialize_type(&mut self, serialize_type: &WithdrawSerializeType);
}

impl<I: IO + Copy> AdminControlled for EthConnectorContract<I> {
    fn get_eth_connector_contract_account(&self) -> AccountId {
        super::get_contract_data(&self.io, EthConnectorStorageId::EthConnectorAccount)
            .expect("ERROR GETTING ETH CONNECTOR ACCOUNT ID")
    }

    fn set_eth_connector_contract_account(&mut self, account: &AccountId) {
        self.io.write_borsh(
            &construct_contract_key(EthConnectorStorageId::EthConnectorAccount),
            account,
        );
    }

    fn get_withdraw_serialize_type(&self) -> WithdrawSerializeType {
        super::get_contract_data(&self.io, EthConnectorStorageId::WithdrawSerializationType)
            .expect("ERROR GETTING WITHDRAW SERIALIZE TYPE")
    }

    fn set_withdraw_serialize_type(&mut self, serialize_type: &WithdrawSerializeType) {
        self.io.write_borsh(
            &construct_contract_key(EthConnectorStorageId::WithdrawSerializationType),
            serialize_type,
        );
    }
}

fn read_json_args<I: IO, T>(io: &I) -> Result<T, ParseArgsError>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = io.read_input().to_vec();
    aurora_engine_types::parameters::engine::parse_json_args(&bytes)
}
