use crate::admin_controlled::{AdminControlled, PausedMask};
use crate::deposit_event::FtTransferMessageData;
use crate::engine::Engine;
use crate::fungible_token::{self, FungibleToken, FungibleTokenMetadata, FungibleTokenOps};
use crate::parameters::{
    InitCallArgs, NEP141FtOnTransferArgs, PauseEthConnectorCallArgs, SetContractDataCallArgs,
    StorageBalanceOfCallArgs, StorageDepositCallArgs, StorageWithdrawCallArgs, WithdrawResult,
};
use crate::prelude::{
    address::error::AddressError, NEP141Wei, Wei, U256, ZERO_NEP141_WEI, ZERO_WEI,
};
use crate::prelude::{
    sdk, str, AccountId, Address, BorshDeserialize, BorshSerialize, EthConnectorStorageId,
    KeyPrefix, NearGas, ToString, Vec, WithdrawCallArgs, Yocto, ERR_FAILED_PARSE,
};
use crate::prelude::{PromiseBatchAction, PromiseCreateArgs};
use aurora_engine_sdk::env::{Env, DEFAULT_PREPAID_GAS};
use aurora_engine_sdk::io::{StorageIntermediate, IO};

pub const ERR_NOT_ENOUGH_BALANCE_FOR_FEE: &str = "ERR_NOT_ENOUGH_BALANCE_FOR_FEE";
/// Indicate zero attached balance for promise call
pub const ZERO_ATTACHED_BALANCE: Yocto = Yocto::new(0);
/// NEAR Gas for calling `fininsh_deposit` promise. Used in the `deposit` logic.
pub const GAS_FOR_FINISH_DEPOSIT: NearGas = NearGas::new(50_000_000_000_000);
/// NEAR Gas for calling `verify_log_entry` promise. Used in the `deposit` logic.
// Note: Is 40Tgas always enough?
const GAS_FOR_VERIFY_LOG_ENTRY: NearGas = NearGas::new(40_000_000_000_000);

/// Admin control flow flag indicates that all control flow unpause (unblocked).
pub const UNPAUSE_ALL: PausedMask = 0;
/// Admin control flow flag indicates that the deposit is paused.
pub const PAUSE_DEPOSIT: PausedMask = 1 << 0;
/// Admin control flow flag indicates that withdrawal is paused.
pub const PAUSE_WITHDRAW: PausedMask = 1 << 1;

/// Eth-connector contract data. It's stored in the storage.
/// Contains:
/// * connector specific data
/// * Fungible token data
/// * paused_mask - admin control flow data
/// * io - I/O trait handler
pub struct EthConnectorContract<I: IO> {
    contract: EthConnector,
    ft: FungibleTokenOps<I>,
    paused_mask: PausedMask,
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
        Ok(Self {
            contract: get_contract_data(&io, &EthConnectorStorageId::Contract)?,
            ft: get_contract_data::<FungibleToken, I>(&io, &EthConnectorStorageId::FungibleToken)?
                .ops(io),
            paused_mask: get_contract_data(&io, &EthConnectorStorageId::PausedMask)?,
            io,
        })
    }

    /// Create contract data - init eth-connector contract specific data.
    /// Used only once for first time initialization.
    /// Initialized contract data stored in the storage.
    pub fn create_contract(
        mut io: I,
        owner_id: AccountId,
        args: InitCallArgs,
    ) -> Result<(), error::InitContractError> {
        // Check is it already initialized
        let contract_key_exists =
            io.storage_has_key(&construct_contract_key(&EthConnectorStorageId::Contract));
        if contract_key_exists {
            return Err(error::InitContractError::AlreadyInitialized);
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
        .map_err(error::InitContractError::InvalidCustodianAddress)?;

        let mut ft = FungibleTokenOps::new(io);
        // Register FT account for current contract
        ft.internal_register_account(&owner_id);

        let paused_mask = UNPAUSE_ALL;
        io.write_borsh(
            &construct_contract_key(&EthConnectorStorageId::PausedMask),
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

    /// Deposit all types of tokens
    pub fn deposit(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "deposit".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH_DEPOSIT,
        }
    }

    /// Internal ETH withdraw ETH logic
    pub(crate) fn internal_remove_eth(
        &mut self,
        amount: Wei,
    ) -> Result<(), fungible_token::error::WithdrawError> {
        self.burn_eth_on_aurora(amount)?;
        self.save_ft_contract();
        Ok(())
    }

    ///  Mint nETH tokens
    fn mint_eth_on_near(
        &mut self,
        owner_id: AccountId,
        amount: NEP141Wei,
    ) -> Result<(), fungible_token::error::DepositError> {
        sdk::log!(&format!("Mint {} nETH tokens for: {}", amount, owner_id));

        if self.ft.get_account_eth_balance(&owner_id).is_none() {
            self.ft.accounts_insert(&owner_id, ZERO_NEP141_WEI);
        }
        self.ft.internal_deposit_eth_to_near(&owner_id, amount)
    }

    ///  Mint ETH tokens
    fn mint_eth_on_aurora(
        &mut self,
        owner_id: Address,
        amount: Wei,
    ) -> Result<(), fungible_token::error::DepositError> {
        sdk::log!(&format!(
            "Mint {} ETH tokens for: {}",
            amount,
            owner_id.encode()
        ));
        self.ft.internal_deposit_eth_to_aurora(owner_id, amount)
    }

    /// Burn ETH tokens
    fn burn_eth_on_aurora(
        &mut self,
        amount: Wei,
    ) -> Result<(), fungible_token::error::WithdrawError> {
        self.ft.internal_withdraw_eth_from_aurora(amount)
    }

    /// Withdraw nETH from NEAR accounts
    /// NOTE: it should be without any log data
    pub fn withdraw_eth_from_near(
        &mut self,
        current_account_id: &AccountId,
        predecessor_account_id: &AccountId,
        args: WithdrawCallArgs,
    ) -> Result<WithdrawResult, error::WithdrawError> {
        // Check is current account id is owner
        let is_owner = current_account_id == predecessor_account_id;
        // Check is current flow paused. If it's owner just skip asserrion.
        self.assert_not_paused(PAUSE_WITHDRAW, is_owner)
            .map_err(|_| error::WithdrawError::Paused)?;

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

    /// Returns total ETH supply on NEAR (nETH as NEP-141 token)
    pub fn ft_total_eth_supply_on_near(&mut self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_total_eth_supply_on_near".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Returns total ETH supply on Aurora (ETH in Aurora EVM)
    pub fn ft_total_eth_supply_on_aurora(&mut self) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_total_eth_supply_on_aurora".to_string(),
            args: Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Return balance of nETH (ETH on Near)
    pub fn ft_balance_of(&self, input: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_balance_of".to_string(),
            args: input,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Return balance of ETH (ETH in Aurora EVM)
    pub fn ft_balance_of_eth_on_aurora(&mut self, input: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_balance_of_eth_on_aurora".to_string(),
            args: input,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        }
    }

    /// Transfer between NEAR accounts
    pub fn ft_transfer(
        &mut self,
        data: Vec<u8>,
    ) -> Result<PromiseCreateArgs, fungible_token::error::TransferError> {
        Ok(PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_transfer".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        })
    }

    /// FT transfer call from sender account (invoker account) to receiver
    /// We starting early checking for message data to avoid `ft_on_transfer` call panics
    /// But we don't check relayer exists. If relayer doesn't exist we simply not mint/burn the amount of the fee
    /// We allow empty messages for cases when `receiver_id =! current_account_id`
    pub fn ft_transfer_call(
        &mut self,
        data: Vec<u8>,
    ) -> Result<PromiseCreateArgs, error::FtTransferCallError> {
        Ok(PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "ft_transfer_call".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: DEFAULT_PREPAID_GAS,
        })
    }

    /// FT storage deposit logic
    pub fn storage_deposit(
        &mut self,
        predecessor_account_id: AccountId,
        amount: Yocto,
        args: StorageDepositCallArgs,
    ) -> Result<Option<PromiseBatchAction>, fungible_token::error::StorageFundingError> {
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

    /// FT storage unregister
    pub fn storage_unregister(
        &mut self,
        account_id: AccountId,
        force: Option<bool>,
    ) -> Result<Option<PromiseBatchAction>, fungible_token::error::StorageFundingError> {
        let promise = match self.ft.internal_storage_unregister(account_id, force) {
            Ok((_, p)) => {
                self.io.return_output(b"true");
                Some(p)
            }
            Err(fungible_token::error::StorageFundingError::NotRegistered) => {
                self.io.return_output(b"false");
                None
            }
            Err(other) => return Err(other),
        };
        Ok(promise)
    }

    /// FT storage withdraw
    pub fn storage_withdraw(
        &mut self,
        account_id: &AccountId,
        args: StorageWithdrawCallArgs,
    ) -> Result<(), fungible_token::error::StorageFundingError> {
        let res = self.ft.storage_withdraw(account_id, args.amount)?;
        self.save_ft_contract();
        self.io.return_output(&res.to_json_bytes());
        Ok(())
    }

    /// Get balance of storage
    pub fn storage_balance_of(&mut self, args: StorageBalanceOfCallArgs) {
        self.io
            .return_output(&self.ft.storage_balance_of(&args.account_id).to_json_bytes());
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
        self.save_ft_contract();
        self.io.return_output("\"0\"".as_bytes());
        Ok(())
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

    pub fn get_bridge_prover(&self) -> &AccountId {
        &self.contract.prover_account
    }

    /// Save eth-connector fungible token contract data
    fn save_ft_contract(&mut self) {
        self.io.write_borsh(
            &construct_contract_key(&EthConnectorStorageId::FungibleToken),
            &self.ft.data(),
        );
    }

    /// Generate key for used events from Proof
    fn used_event_key(&self, key: &str) -> Vec<u8> {
        let mut v = construct_contract_key(&EthConnectorStorageId::UsedEvent).to_vec();
        v.extend_from_slice(key.as_bytes());
        v
    }

    /// Save already used event proof as hash key
    fn save_used_event(&mut self, key: &str) {
        self.io.write_borsh(&self.used_event_key(key), &0u8);
    }

    /// Checks whether the provided proof was already used
    pub fn is_used_proof(&self, data: Vec<u8>) -> PromiseCreateArgs {
        PromiseCreateArgs {
            target_account_id: self.get_eth_connector_contract_account(),
            method: "is_used_proof".to_string(),
            args: data,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH_DEPOSIT,
        }
    }

    /// Get Eth connector paused flags
    pub fn get_paused_flags(&self) -> PausedMask {
        self.get_paused()
    }

    /// Set Eth connector paused flags
    pub fn set_paused_flags(&mut self, args: PauseEthConnectorCallArgs) {
        self.set_paused(args.paused_mask);
    }
}

impl<I: IO + Copy> AdminControlled for EthConnectorContract<I> {
    /// Get current admin paused status
    fn get_paused(&self) -> PausedMask {
        self.paused_mask
    }

    /// Set admin paused status
    fn set_paused(&mut self, paused_mask: PausedMask) {
        self.paused_mask = paused_mask;
        self.io.write_borsh(
            &construct_contract_key(&EthConnectorStorageId::PausedMask),
            &self.paused_mask,
        );
    }

    fn get_eth_connector_contract_account(&self) -> AccountId {
        get_contract_data(&self.io, &EthConnectorStorageId::EthConnectorAccount).unwrap()
    }

    fn set_eth_connector_contract_account(&mut self, account: AccountId) {
        self.io.write_borsh(
            &construct_contract_key(&EthConnectorStorageId::EthConnectorAccount),
            &account,
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

/// Return metdata
pub fn get_metadata<I: IO>(io: &I) -> Option<FungibleTokenMetadata> {
    io.read_storage(&construct_contract_key(
        &EthConnectorStorageId::FungibleTokenMetadata,
    ))
    .and_then(|data| data.to_value().ok())
}

pub mod error {
    use crate::errors;
    use aurora_engine_types::types::address::error::AddressError;
    use aurora_engine_types::types::balance::error::BalanceOverflowError;

    use crate::deposit_event::error::ParseOnTransferMessageError;
    use crate::{deposit_event, fungible_token};

    const PROOF_EXIST: &[u8; 15] = errors::ERR_PROOF_EXIST;

    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
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
    pub enum DepositError {
        Paused,
        ProofParseFailed,
        EventParseFailed(deposit_event::error::ParseError),
        CustodianAddressMismatch,
        InsufficientAmountForFee,
        InvalidAddress(AddressError),
    }

    impl AsRef<[u8]> for DepositError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::Paused => crate::admin_controlled::ERR_PAUSED.as_bytes(),
                Self::ProofParseFailed => super::ERR_FAILED_PARSE.as_bytes(),
                Self::EventParseFailed(e) => e.as_ref(),
                Self::CustodianAddressMismatch => errors::ERR_WRONG_EVENT_ADDRESS,
                Self::InsufficientAmountForFee => super::ERR_NOT_ENOUGH_BALANCE_FOR_FEE.as_bytes(),
                Self::InvalidAddress(e) => e.as_ref(),
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

    impl From<fungible_token::error::DepositError> for FinishDepositError {
        fn from(e: fungible_token::error::DepositError) -> Self {
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
        Paused,
        FT(fungible_token::error::WithdrawError),
    }

    impl From<fungible_token::error::WithdrawError> for WithdrawError {
        fn from(e: fungible_token::error::WithdrawError) -> Self {
            Self::FT(e)
        }
    }

    impl AsRef<[u8]> for WithdrawError {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::Paused => crate::admin_controlled::ERR_PAUSED.as_bytes(),
                Self::FT(e) => e.as_ref(),
            }
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
    pub enum FtTransferCallError {
        BalanceOverflow(BalanceOverflowError),
        MessageParseFailed(ParseOnTransferMessageError),
        InsufficientAmountForFee,
        Transfer(fungible_token::error::TransferError),
    }

    impl From<fungible_token::error::TransferError> for FtTransferCallError {
        fn from(e: fungible_token::error::TransferError) -> Self {
            Self::Transfer(e)
        }
    }

    impl From<fungible_token::error::DepositError> for FtTransferCallError {
        fn from(e: fungible_token::error::DepositError) -> Self {
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
}
