use crate::admin_controlled::AdminControlled;
use crate::parameters::{BalanceOfEthCallArgs, NEP141FtOnTransferArgs, SetContractDataCallArgs};
use crate::prelude::{address::error::AddressError, Wei};
use crate::prelude::{PromiseCreateArgs, U256};

use crate::deposit_event::FtTransferMessageData;
use crate::engine::Engine;
use crate::metadata::FungibleTokenMetadata;
use crate::prelude::{
    format, sdk, str, AccountId, Address, BorshDeserialize, BorshSerialize, EthConnectorStorageId,
    KeyPrefix, NearGas, ToString, Vec, Yocto,
};
use aurora_engine_sdk::env::{Env, DEFAULT_PREPAID_GAS};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::types::ZERO_WEI;
use error::DepositError;

pub const ERR_NOT_ENOUGH_BALANCE_FOR_FEE: &str = "ERR_NOT_ENOUGH_BALANCE_FOR_FEE";
/// Indicate zero attached balance for promise call
pub const ZERO_ATTACHED_BALANCE: Yocto = Yocto::new(0);
/// NEAR Gas for calling `fininsh_deposit` promise. Used in the `deposit` logic.
pub const GAS_FOR_FINISH_DEPOSIT: NearGas = NearGas::new(50_000_000_000_000);
pub const GAS_FOR_DEPOSIT: NearGas = NearGas::new(120_000_000_000_000);
pub const GAS_FOR_WITHDRAW: NearGas = NearGas::new(20_000_000_000_000);
pub const GAS_FOR_FT_TRANSFER: NearGas = NearGas::new(50_000_000_000_000);
pub const GAS_FOR_FT_TRANSFER_CALL: NearGas = NearGas::new(100_000_000_000_000);
pub const VIEW_CALL_GAS: NearGas = NearGas::new(15_000_000_000_000);
/// NEAR Gas for calling `verify_log_entry` promise. Used in the `deposit` logic.
// Note: Is 40Tgas always enough?

/// Eth-connector contract data. It's stored in the storage.
/// Contains:
/// * connector specific data
/// * Fungible token data
/// * paused_mask - admin control flow data
/// * io - I/O trait handler
pub struct EthConnectorContract<I: IO> {
    io: I,
}

/// Connector specific data. It always should contain `prover account` -
#[derive(BorshSerialize, BorshDeserialize)]
pub struct EthConnector {
    /// It used in the Deposit flow, to verify log entry form incoming proof.
    pub prover_account: AccountId,
    /// It is Eth address, used in the Deposit and Withdraw logic.
    pub eth_custodian_address: Address,
}

impl<I: IO + Copy> EthConnectorContract<I> {
    /// Init Eth-connector contract instance.
    /// Load contract data from storage and init I/O handler.
    /// Used as single point of contract access for various contract actions
    pub fn init_instance(io: I) -> Result<Self, error::StorageReadError> {
        Ok(Self { io })
    }

    /// Create contract data - init eth-connector contract specific data.
    /// Used only once for first time initialization.
    /// Initialized contract data stored in the storage.
    pub fn create_contract() -> Result<(), error::InitContractError> {
        // NOTE: do nothing
        Ok(())
    }

    /// Deposit all types of tokens
    pub fn deposit(&self, data: Vec<u8>) -> PromiseCreateArgs {
        sdk::log!("Call Deposit");
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "deposit".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_DEPOSIT,
        }
    }

    /// Withdraw nETH from NEAR accounts
    /// NOTE: it should be without any log data
    pub fn withdraw_eth_from_near(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "withdraw".to_string(),
            args: data,
            attached_balance: Yocto::new(1),
            attached_gas: GAS_FOR_WITHDRAW,
        }
    }

    /// Returns total ETH supply on NEAR (nETH as NEP-141 token)
    pub fn ft_total_eth_supply_on_near(&mut self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_total_supply".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: VIEW_CALL_GAS,
        }
    }

    /// Return balance of nETH (ETH on Near)
    pub fn ft_balance_of(&self, input: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_balance_of".to_string(),
            args: input,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: VIEW_CALL_GAS,
        }
    }

    /// Return balance of ETH (ETH in Aurora EVM)
    /// Return balance of ETH (ETH in Aurora EVM)
    pub fn ft_balance_of_eth_on_aurora(
        &mut self,
        args: BalanceOfEthCallArgs,
    ) -> Result<(), crate::prelude::types::balance::error::BalanceOverflowError> {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(&args.address);
        sdk::log!("Balance of ETH [{}]: {}", args.address.encode(), balance);
        self.io.return_output(format!("\"{}\"", balance).as_bytes());
        Ok(())
    }

    /// Balance of ETH (ETH on Aurora)
    pub fn internal_unwrap_balance_of_eth_on_aurora(&self, address: &Address) -> Wei {
        crate::engine::get_balance(&self.io, address)
    }

    /// ft_on_transfer callback function
    pub fn ft_on_transfer<'env, E: Env>(
        &mut self,
        engine: &Engine<'env, I, E>,
        args: &NEP141FtOnTransferArgs,
    ) -> Result<(), error::FtTransferCallError> {
        sdk::log!("Call ft_on_transfer");
        // Parse message with specific rules
        let message_data = FtTransferMessageData::parse_on_transfer_message(&args.msg)
            .map_err(error::FtTransferCallError::MessageParseFailed)?;

        // Special case when predecessor_account_id is current_account_id
        let wei_fee = Wei::from(message_data.fee);
        // Mint fee to relayer
        let relayer = engine.get_relayer(message_data.relayer.as_bytes());
        match (wei_fee, relayer) {
            (fee, Some(evm_relayer_address)) if fee > ZERO_WEI => {
                self.mint_eth_on_aurora(
                    message_data.recipient,
                    Wei::new(U256::from(args.amount.as_u128())) - fee,
                )?;
                self.mint_eth_on_aurora(evm_relayer_address, fee)?;
            }
            _ => self.mint_eth_on_aurora(
                message_data.recipient,
                Wei::new(U256::from(args.amount.as_u128())),
            )?,
        }
        //self.save_ft_contract();
        self.io.return_output("\"0\"".as_bytes());
        Ok(())
    }

    ///  Mint ETH tokens
    fn mint_eth_on_aurora(&mut self, owner_id: Address, amount: Wei) -> Result<(), DepositError> {
        sdk::log!("Mint {} ETH tokens for: {}", amount, owner_id.encode());
        self.internal_deposit_eth_to_aurora(owner_id, amount)
    }

    /// Internal ETH deposit to Aurora
    pub fn internal_deposit_eth_to_aurora(
        &mut self,
        address: Address,
        amount: Wei,
    ) -> Result<(), DepositError> {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(&address);
        let new_balance = balance
            .checked_add(amount)
            .ok_or(DepositError::BalanceOverflow)?;
        crate::engine::set_balance(&mut self.io, &address, &new_balance);
        Ok(())
    }

    /// Transfer between NEAR accounts
    pub fn ft_transfer(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_ft_transfer".to_string(),
            args: data,
            attached_balance: Yocto::new(1),
            attached_gas: GAS_FOR_FT_TRANSFER,
        }
    }

    /// FT transfer call from sender account (invoker account) to receiver
    /// We starting early checking for message data to avoid `ft_on_transfer` call panics
    /// But we don't check relayer exists. If relayer doesn't exist we simply not mint/burn the amount of the fee
    /// We allow empty messages for cases when `receiver_id =! current_account_id`
    pub fn ft_transfer_call(&mut self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_ft_transfer_call".to_string(),
            args: data,
            attached_balance: Yocto::new(1),
            attached_gas: GAS_FOR_FT_TRANSFER_CALL,
        }
    }

    /// FT storage deposit logic
    pub fn storage_deposit(&self, data: Vec<u8>, attached_deposit: u128) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_storage_deposit".to_string(),
            args: data,
            attached_balance: Yocto::new(attached_deposit),
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// FT storage unregister
    pub fn storage_unregister(&mut self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_storage_unregister".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// FT storage withdraw
    pub fn storage_withdraw(&mut self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "engine_storage_withdraw".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Get balance of storage
    pub fn storage_balance_of(&mut self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "storage_balance_of".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Get accounts counter for statistics.
    /// It represents total unique accounts (all-time, including accounts which now have zero balance).
    pub fn get_accounts_counter(&mut self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "get_accounts_counter".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH_DEPOSIT,
        }
    }

    pub fn get_bridge_prover(&self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "get_bridge_prover".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH_DEPOSIT,
        }
    }

    /// Checks whether the provided proof was already used
    pub fn is_used_proof(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "is_used_proof".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: VIEW_CALL_GAS,
        }
    }

    /// Get Eth connector paused flags
    pub fn get_paused_flags(&self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "get_paused_flags".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Return FT metadata
    pub fn get_metadata(&self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_metadata".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH_DEPOSIT,
        }
    }

    /// Disable flag for standalone-legacy-nep141
    pub fn disable_legacy_nep141(&mut self) {
        self.io.write_borsh(
            &construct_contract_key(&EthConnectorStorageId::DisableLegacyNEP141),
            &1u8,
        );
    }

    pub fn is_disabled_legacy_nep141(&self) -> bool {
        self.io.storage_has_key(&construct_contract_key(
            &EthConnectorStorageId::DisableLegacyNEP141,
        ))
    }
}

impl<I: IO + Copy> AdminControlled for EthConnectorContract<I> {
    fn get_eth_connector_contract_account(&self) -> AccountId {
        get_contract_data(&self.io, &EthConnectorStorageId::EthConnectorAccount).unwrap()
    }

    fn set_eth_connector_contract_account(&mut self, account: &AccountId) {
        self.io.write_borsh(
            &construct_contract_key(&EthConnectorStorageId::EthConnectorAccount),
            account,
        );
    }
}

fn construct_contract_key(suffix: &EthConnectorStorageId) -> Vec<u8> {
    crate::prelude::bytes_to_key(KeyPrefix::EthConnector, &[u8::from(*suffix)])
}

fn get_contract_data<T: BorshDeserialize, I: IO>(
    io: &I,
    suffix: &EthConnectorStorageId,
) -> Result<T, error::StorageReadError> {
    io.read_storage(&construct_contract_key(suffix))
        .ok_or(error::StorageReadError::KeyNotFound)
        .and_then(|x| {
            x.to_value()
                .map_err(|_| error::StorageReadError::BorshDeserialize)
        })
}

/// Sets the contract data and returns it back
pub fn set_contract_data<I: IO>(
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
        &construct_contract_key(&EthConnectorStorageId::Contract),
        &contract_data,
    );

    io.write_borsh(
        &construct_contract_key(&EthConnectorStorageId::FungibleTokenMetadata),
        &args.metadata,
    );

    Ok(contract_data)
}

/// Return metadata
pub fn get_metadata<I: IO>(io: &I) -> Option<FungibleTokenMetadata> {
    io.read_storage(&construct_contract_key(
        &EthConnectorStorageId::FungibleTokenMetadata,
    ))
    .and_then(|data| data.to_value().ok())
}

pub mod error {
    use crate::errors;
    use aurora_engine_types::types::address::error::AddressError;

    use crate::deposit_event::error::ParseOnTransferMessageError;
    use crate::prelude::types::balance::error::BalanceOverflowError;

    const TOTAL_SUPPLY_OVERFLOW: &[u8; 25] = errors::ERR_TOTAL_SUPPLY_OVERFLOW;
    const BALANCE_OVERFLOW: &[u8; 20] = errors::ERR_BALANCE_OVERFLOW;
    const NOT_ENOUGH_BALANCE: &[u8; 22] = errors::ERR_NOT_ENOUGH_BALANCE;
    const TOTAL_SUPPLY_UNDERFLOW: &[u8; 26] = errors::ERR_TOTAL_SUPPLY_UNDERFLOW;
    const ZERO_AMOUNT: &[u8; 15] = errors::ERR_ZERO_AMOUNT;
    const SELF_TRANSFER: &[u8; 26] = errors::ERR_SENDER_EQUALS_RECEIVER;

    const PROOF_EXIST: &[u8; 15] = errors::ERR_PROOF_EXIST;

    #[derive(Debug)]
    pub enum StorageReadError {
        KeyNotFound,
        BorshDeserialize,
    }

    impl AsRef<[u8]> for StorageReadError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::KeyNotFound => errors::ERR_CONNECTOR_STORAGE_KEY_NOT_FOUND,
                Self::BorshDeserialize => errors::ERR_FAILED_DESERIALIZE_CONNECTOR_DATA,
            }
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
    pub enum FinishDepositError {
        TransferCall(FtTransferCallError),
        ProofUsed,
    }

    impl From<ProofUsed> for FinishDepositError {
        fn from(_: ProofUsed) -> Self {
            Self::ProofUsed
        }
    }

    impl From<FtTransferCallError> for FinishDepositError {
        fn from(e: FtTransferCallError) -> Self {
            Self::TransferCall(e)
        }
    }

    impl From<DepositError> for FinishDepositError {
        fn from(e: DepositError) -> Self {
            Self::TransferCall(FtTransferCallError::Transfer(e.into()))
        }
    }

    impl AsRef<[u8]> for FinishDepositError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::ProofUsed => PROOF_EXIST,
                Self::TransferCall(e) => e.as_ref(),
            }
        }
    }

    #[derive(Debug)]
    pub enum WithdrawError {
        FT(WithdrawFtError),
        ParseArgs,
    }

    impl From<WithdrawFtError> for WithdrawError {
        fn from(e: WithdrawFtError) -> Self {
            Self::FT(e)
        }
    }

    impl AsRef<[u8]> for WithdrawError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::FT(e) => e.as_ref(),
                Self::ParseArgs => b"ERR_PARSE_ARGS",
            }
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
    pub enum FtTransferCallError {
        BalanceOverflow(BalanceOverflowError),
        MessageParseFailed(ParseOnTransferMessageError),
        InsufficientAmountForFee,
        Transfer(TransferError),
    }

    impl From<TransferError> for FtTransferCallError {
        fn from(e: TransferError) -> Self {
            Self::Transfer(e)
        }
    }

    impl From<DepositError> for FtTransferCallError {
        fn from(e: DepositError) -> Self {
            Self::Transfer(e.into())
        }
    }

    impl From<ParseOnTransferMessageError> for FtTransferCallError {
        fn from(e: ParseOnTransferMessageError) -> Self {
            Self::MessageParseFailed(e)
        }
    }

    impl AsRef<[u8]> for FtTransferCallError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::MessageParseFailed(e) => e.as_ref(),
                Self::InsufficientAmountForFee => super::ERR_NOT_ENOUGH_BALANCE_FOR_FEE.as_bytes(),
                Self::Transfer(e) => e.as_ref(),
                Self::BalanceOverflow(e) => e.as_ref(),
            }
        }
    }

    #[derive(Debug)]
    pub enum InitContractError {
        AlreadyInitialized,
        InvalidCustodianAddress(AddressError),
    }

    impl AsRef<[u8]> for InitContractError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::AlreadyInitialized => errors::ERR_CONTRACT_INITIALIZED,
                Self::InvalidCustodianAddress(e) => e.as_ref(),
            }
        }
    }

    pub struct ProofUsed;

    impl AsRef<[u8]> for ProofUsed {
        fn as_ref(&self) -> &[u8] {
            PROOF_EXIST
        }
    }

    #[derive(Debug)]
    pub enum DepositError {
        TotalSupplyOverflow,
        BalanceOverflow,
    }

    impl AsRef<[u8]> for DepositError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::TotalSupplyOverflow => TOTAL_SUPPLY_OVERFLOW,
                Self::BalanceOverflow => BALANCE_OVERFLOW,
            }
        }
    }

    #[derive(Debug)]
    pub enum WithdrawFtError {
        TotalSupplyUnderflow,
        InsufficientFunds,
        BalanceOverflow(BalanceOverflowError),
    }

    impl AsRef<[u8]> for WithdrawFtError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::TotalSupplyUnderflow => TOTAL_SUPPLY_UNDERFLOW,
                Self::InsufficientFunds => NOT_ENOUGH_BALANCE,
                Self::BalanceOverflow(e) => e.as_ref(),
            }
        }
    }

    #[derive(Debug)]
    pub enum TransferError {
        TotalSupplyUnderflow,
        TotalSupplyOverflow,
        InsufficientFunds,
        BalanceOverflow,
        ZeroAmount,
        SelfTransfer,
    }

    impl AsRef<[u8]> for TransferError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::TotalSupplyUnderflow => TOTAL_SUPPLY_UNDERFLOW,
                Self::TotalSupplyOverflow => TOTAL_SUPPLY_OVERFLOW,
                Self::InsufficientFunds => NOT_ENOUGH_BALANCE,
                Self::BalanceOverflow => BALANCE_OVERFLOW,
                Self::ZeroAmount => ZERO_AMOUNT,
                Self::SelfTransfer => SELF_TRANSFER,
            }
        }
    }

    impl From<WithdrawFtError> for TransferError {
        fn from(err: WithdrawFtError) -> Self {
            match err {
                WithdrawFtError::InsufficientFunds => Self::InsufficientFunds,
                WithdrawFtError::TotalSupplyUnderflow => Self::TotalSupplyUnderflow,
                WithdrawFtError::BalanceOverflow(_) => Self::BalanceOverflow,
            }
        }
    }

    impl From<DepositError> for TransferError {
        fn from(err: DepositError) -> Self {
            match err {
                DepositError::BalanceOverflow => Self::BalanceOverflow,
                DepositError::TotalSupplyOverflow => Self::TotalSupplyOverflow,
            }
        }
    }

    #[derive(Debug)]
    pub enum StorageFundingError {
        NotRegistered,
        NoAvailableBalance,
        InsufficientDeposit,
        UnRegisterPositiveBalance,
    }

    impl AsRef<[u8]> for StorageFundingError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::NotRegistered => errors::ERR_ACCOUNT_NOT_REGISTERED,
                Self::NoAvailableBalance => errors::ERR_NO_AVAILABLE_BALANCE,
                Self::InsufficientDeposit => errors::ERR_ATTACHED_DEPOSIT_NOT_ENOUGH,
                Self::UnRegisterPositiveBalance => {
                    errors::ERR_FAILED_UNREGISTER_ACCOUNT_POSITIVE_BALANCE
                }
            }
        }
    }
}
