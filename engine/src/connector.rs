use crate::admin_controlled::{AdminControlled, PausedMask};
use crate::deposit_event::{DepositedEvent, FtTransferMessageData, TokenMessageData};
use crate::engine::Engine;
use crate::fungible_token::{self, FungibleToken, FungibleTokenMetadata, FungibleTokenOps};
use crate::parameters::{
    BalanceOfCallArgs, BalanceOfEthCallArgs, FinishDepositCallArgs, InitCallArgs,
    NEP141FtOnTransferArgs, PauseEthConnectorCallArgs, ResolveTransferCallArgs,
    SetContractDataCallArgs, StorageBalanceOfCallArgs, StorageDepositCallArgs,
    StorageWithdrawCallArgs, TransferCallArgs, TransferCallCallArgs, WithdrawResult,
};
use crate::prelude::{
    address::error::AddressError, NEP141Wei, Wei, U256, ZERO_NEP141_WEI, ZERO_WEI,
};
use crate::prelude::{
    format, sdk, str, AccountId, Address, BorshDeserialize, BorshSerialize, EthConnectorStorageId,
    KeyPrefix, NearGas, PromiseResult, ToString, Vec, WithdrawCallArgs, Yocto, ERR_FAILED_PARSE,
};
use crate::prelude::{PromiseBatchAction, PromiseCreateArgs, PromiseWithCallbackArgs};
use crate::proof::Proof;
use aurora_engine_sdk::env::Env;
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
    pub fn deposit(
        &self,
        raw_proof: Vec<u8>,
        current_account_id: AccountId,
        predecessor_account_id: AccountId,
    ) -> Result<PromiseWithCallbackArgs, error::DepositError> {
        // Check is current account owner
        let is_owner = current_account_id == predecessor_account_id;
        // Check is current flow paused. If it's owner account just skip it.
        self.assert_not_paused(PAUSE_DEPOSIT, is_owner)
            .map_err(|_| error::DepositError::Paused)?;

        sdk::log!("[Deposit tokens]");

        // Get incoming deposit arguments
        let proof: Proof =
            Proof::try_from_slice(&raw_proof).map_err(|_| error::DepositError::ProofParseFailed)?;
        // Fetch event data from Proof
        let event = DepositedEvent::from_log_entry_data(&proof.log_entry_data)
            .map_err(error::DepositError::EventParseFailed)?;

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
            return Err(error::DepositError::CustodianAddressMismatch);
        }

        if NEP141Wei::new(event.fee.as_u128()) >= event.amount {
            return Err(error::DepositError::InsufficientAmountForFee);
        }

        // Verify proof data with cross-contract call to prover account
        sdk::log!(
            "Deposit verify_log_entry for prover: {}",
            self.contract.prover_account,
        );

        // Do not skip bridge call. This is only used for development and diagnostics.
        let skip_bridge_call = false.try_to_vec().unwrap();
        let mut proof_to_verify = raw_proof;
        proof_to_verify.extend(skip_bridge_call);

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
            TokenMessageData::Near(account_id) => FinishDepositCallArgs {
                new_owner_id: account_id,
                amount: event.amount,
                proof_key: proof.key(),
                relayer_id: predecessor_account_id,
                fee: event.fee,
                msg: None,
            }
            .try_to_vec()
            .unwrap(),
            // Deposit to Eth accounts
            // fee is being minted in the `ft_on_transfer` callback method
            TokenMessageData::Eth {
                receiver_id,
                message,
            } => {
                // Transfer to self and then transfer ETH in `ft_on_transfer`
                // address - is NEAR account
                let transfer_data = TransferCallCallArgs {
                    receiver_id,
                    amount: event.amount,
                    memo: None,
                    msg: message.encode(),
                }
                .try_to_vec()
                .unwrap();

                // Send to self - current account id
                FinishDepositCallArgs {
                    new_owner_id: current_account_id.clone(),
                    amount: event.amount,
                    proof_key: proof.key(),
                    relayer_id: predecessor_account_id,
                    fee: event.fee,
                    msg: Some(transfer_data),
                }
                .try_to_vec()
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

    /// Finish deposit (private method)
    /// NOTE: we should `record_proof` only after `mint` operation. The reason
    /// is that in this case we only calculate the amount to be credited but
    /// do not save it, however, if an error occurs during the calculation,
    /// this will happen before `record_proof`. After that contract will save.
    pub fn finish_deposit(
        &mut self,
        predecessor_account_id: AccountId,
        current_account_id: AccountId,
        data: FinishDepositCallArgs,
        prepaid_gas: NearGas,
    ) -> Result<Option<PromiseWithCallbackArgs>, error::FinishDepositError> {
        sdk::log!("Finish deposit with the amount: {}", data.amount);

        // Mint tokens to recipient minus fee
        if let Some(msg) = data.msg {
            // Mint - calculate new balances
            self.mint_eth_on_near(data.new_owner_id, data.amount)?;
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
                data.new_owner_id.clone(),
                data.amount - NEP141Wei::new(data.fee.as_u128()),
            )?;
            self.mint_eth_on_near(data.relayer_id, NEP141Wei::new(data.fee.as_u128()))?;
            // Store proof only after `mint` calculations
            self.record_proof(&data.proof_key)?;
            // Save new contract data
            self.save_ft_contract();
            Ok(None)
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

    /// Record used proof as hash key
    fn record_proof(&mut self, key: &str) -> Result<(), error::ProofUsed> {
        sdk::log!("Record proof: {}", key);

        if self.is_used_event(key) {
            return Err(error::ProofUsed);
        }

        self.save_used_event(key);
        Ok(())
    }

    ///  Mint nETH tokens
    fn mint_eth_on_near(
        &mut self,
        owner_id: AccountId,
        amount: NEP141Wei,
    ) -> Result<(), fungible_token::error::DepositError> {
        sdk::log!("Mint {} nETH tokens for: {}", amount, owner_id);

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
        sdk::log!("Mint {} ETH tokens for: {}", amount, owner_id.encode());
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
    pub fn ft_total_eth_supply_on_near(&mut self) {
        let total_supply = self.ft.ft_total_eth_supply_on_near();
        sdk::log!("Total ETH supply on NEAR: {}", total_supply);
        self.io
            .return_output(format!("\"{}\"", total_supply).as_bytes());
    }

    /// Returns total ETH supply on Aurora (ETH in Aurora EVM)
    pub fn ft_total_eth_supply_on_aurora(&mut self) {
        let total_supply = self.ft.ft_total_eth_supply_on_aurora();
        sdk::log!("Total ETH supply on Aurora: {}", total_supply);
        self.io
            .return_output(format!("\"{}\"", total_supply).as_bytes());
    }

    /// Return balance of nETH (ETH on Near)
    pub fn ft_balance_of(&mut self, args: BalanceOfCallArgs) {
        let balance = self.ft.ft_balance_of(&args.account_id);
        sdk::log!("Balance of nETH [{}]: {}", args.account_id, balance);

        self.io.return_output(format!("\"{}\"", balance).as_bytes());
    }

    /// Return balance of ETH (ETH in Aurora EVM)
    pub fn ft_balance_of_eth_on_aurora(
        &mut self,
        args: BalanceOfEthCallArgs,
    ) -> Result<(), crate::prelude::types::balance::error::BalanceOverflowError> {
        let balance = self
            .ft
            .internal_unwrap_balance_of_eth_on_aurora(&args.address);
        sdk::log!("Balance of ETH [{}]: {}", args.address.encode(), balance);
        self.io.return_output(format!("\"{}\"", balance).as_bytes());
        Ok(())
    }

    /// Transfer between NEAR accounts
    pub fn ft_transfer(
        &mut self,
        predecessor_account_id: &AccountId,
        args: TransferCallArgs,
    ) -> Result<(), fungible_token::error::TransferError> {
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
        args: ResolveTransferCallArgs,
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
        self.io.return_output(format!("\"{}\"", amount).as_bytes());
    }

    /// FT transfer call from sender account (invoker account) to receiver
    /// We starting early checking for message data to avoid `ft_on_transfer` call panics
    /// But we don't check relayer exists. If relayer doesn't exist we simply not mint/burn the amount of the fee
    /// We allow empty messages for cases when `receiver_id =! current_account_id`
    pub fn ft_transfer_call(
        &mut self,
        predecessor_account_id: AccountId,
        current_account_id: AccountId,
        args: TransferCallCallArgs,
        prepaid_gas: NearGas,
    ) -> Result<PromiseWithCallbackArgs, error::FtTransferCallError> {
        sdk::log!(
            "Transfer call to {} amount {}",
            args.receiver_id,
            args.amount,
        );

        // Verify message data before `ft_on_transfer` call to avoid verification panics
        // It's allowed empty message if `receiver_id =! current_account_id`
        if args.receiver_id == current_account_id {
            let message_data = FtTransferMessageData::parse_on_transfer_message(&args.msg)
                .map_err(error::FtTransferCallError::MessageParseFailed)?;
            // Check is transfer amount > fee
            if message_data.fee.as_u128() >= args.amount.as_u128() {
                return Err(error::FtTransferCallError::InsufficientAmountForFee);
            }

            // Additional check overflow before process `ft_on_transfer`
            // But don't check overflow for relayer
            // Note: It can't overflow because the total supply doesn't change during transfer.
            let amount_for_check = self
                .ft
                .internal_unwrap_balance_of_eth_on_aurora(&message_data.recipient);
            if amount_for_check
                .checked_add(Wei::from(args.amount))
                .is_none()
            {
                return Err(error::FtTransferCallError::Transfer(
                    fungible_token::error::TransferError::BalanceOverflow,
                ));
            }
            if self
                .ft
                .total_eth_supply_on_aurora
                .checked_add(Wei::from(args.amount))
                .is_none()
            {
                return Err(error::FtTransferCallError::Transfer(
                    fungible_token::error::TransferError::TotalSupplyOverflow,
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
    pub fn get_accounts_counter(&mut self) {
        self.io
            .return_output(&self.ft.get_accounts_counter().to_le_bytes());
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

    /// Check is event of proof already used
    fn is_used_event(&self, key: &str) -> bool {
        self.io.storage_has_key(&self.used_event_key(key))
    }

    /// Checks whether the provided proof was already used
    pub fn is_used_proof(&self, proof: Proof) -> bool {
        self.is_used_event(&proof.key())
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
