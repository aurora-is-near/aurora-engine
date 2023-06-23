use crate::connector::ZERO_ATTACHED_BALANCE;
use crate::engine;
use crate::parameters::{NEP141FtOnTransferArgs, ResolveTransferCallArgs, StorageBalance};
use crate::prelude::account_id::AccountId;
use crate::prelude::Wei;
use crate::prelude::{
    sdk, storage, vec, Address, Balance, BorshDeserialize, BorshSerialize, NearGas, PromiseAction,
    PromiseBatchAction, PromiseCreateArgs, PromiseResult, PromiseWithCallbackArgs,
    StorageBalanceBounds, StorageUsage, String, ToString, Vec,
};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::borsh;
pub use aurora_engine_types::parameters::connector::FungibleTokenMetadata;
use aurora_engine_types::types::{NEP141Wei, Yocto, ZERO_NEP141_WEI, ZERO_YOCTO};

/// Gas for `resolve_transfer`: 5 `TGas`
const GAS_FOR_RESOLVE_TRANSFER: NearGas = NearGas::new(5_000_000_000_000);
/// Gas for `ft_on_transfer`
const GAS_FOR_FT_TRANSFER_CALL: NearGas = NearGas::new(35_000_000_000_000);

#[derive(Debug, Default, BorshDeserialize, BorshSerialize)]
pub struct FungibleToken {
    /// Total ETH supply on Near (nETH as NEP-141 token)
    pub total_eth_supply_on_near: NEP141Wei,

    /// Total ETH supply on Aurora (ETH in Aurora EVM)
    /// NOTE: For compatibility reasons, we do not use  `Wei` (32 bytes)
    /// buy `NEP141Wei` (16 bytes)
    pub total_eth_supply_on_aurora: NEP141Wei,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

impl FungibleToken {
    pub fn ops<I: IO>(self, io: I) -> FungibleTokenOps<I> {
        FungibleTokenOps {
            total_eth_supply_on_near: self.total_eth_supply_on_near,
            total_eth_supply_on_aurora: Wei::from(self.total_eth_supply_on_aurora),
            account_storage_usage: self.account_storage_usage,
            io,
        }
    }
}

pub struct FungibleTokenOps<I: IO> {
    /// Total ETH supply on Near (nETH as NEP-141 token)
    pub total_eth_supply_on_near: NEP141Wei,

    /// Total ETH supply on Aurora (ETH in Aurora EVM)
    pub total_eth_supply_on_aurora: Wei,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,

    io: I,
}

impl<I: IO + Copy> FungibleTokenOps<I> {
    pub fn new(io: I) -> Self {
        FungibleToken::default().ops(io)
    }

    pub fn data(&self) -> FungibleToken {
        FungibleToken {
            total_eth_supply_on_near: self.total_eth_supply_on_near,
            // TODO: both types should be same
            // ut must never panic
            total_eth_supply_on_aurora: NEP141Wei::new(
                self.total_eth_supply_on_aurora.try_into_u128().unwrap(),
            ),
            account_storage_usage: self.account_storage_usage,
        }
    }

    /// Balance of ETH (ETH on Aurora).
    pub fn internal_unwrap_balance_of_eth_on_aurora(&self, address: &Address) -> Wei {
        engine::get_balance(&self.io, address)
    }

    /// Internal `nETH` deposit (ETH on NEAR).
    pub fn internal_deposit_eth_to_near(
        &mut self,
        account_id: &AccountId,
        amount: NEP141Wei,
    ) -> Result<(), error::DepositError> {
        let balance = self
            .get_account_eth_balance(account_id)
            .unwrap_or(ZERO_NEP141_WEI);
        let new_balance = balance
            .checked_add(amount)
            .ok_or(error::DepositError::BalanceOverflow)?;
        self.accounts_insert(account_id, new_balance);
        self.total_eth_supply_on_near = self
            .total_eth_supply_on_near
            .checked_add(amount)
            .ok_or(error::DepositError::TotalSupplyOverflow)?;
        Ok(())
    }

    /// Internal `ETH` deposit (ETH on Aurora).
    pub fn internal_deposit_eth_to_aurora(
        &mut self,
        address: Address,
        amount: Wei,
    ) -> Result<(), error::DepositError> {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(&address);
        let new_balance = balance
            .checked_add(amount)
            .ok_or(error::DepositError::BalanceOverflow)?;
        engine::set_balance(&mut self.io, &address, &new_balance);
        self.total_eth_supply_on_aurora = self
            .total_eth_supply_on_aurora
            .checked_add(amount)
            .ok_or(error::DepositError::TotalSupplyOverflow)?;
        Ok(())
    }

    /// Withdraw `nETH` tokens (ETH on NEAR).
    pub fn internal_withdraw_eth_from_near(
        &mut self,
        account_id: &AccountId,
        amount: NEP141Wei,
    ) -> Result<(), error::WithdrawError> {
        let balance = self
            .get_account_eth_balance(account_id)
            .unwrap_or(ZERO_NEP141_WEI);
        let new_balance = balance
            .checked_sub(amount)
            .ok_or(error::WithdrawError::InsufficientFunds)?;
        self.accounts_insert(account_id, new_balance);
        self.total_eth_supply_on_near = self
            .total_eth_supply_on_near
            .checked_sub(amount)
            .ok_or(error::WithdrawError::TotalSupplyUnderflow)?;
        Ok(())
    }

    /// Withdraw `ETH` tokens (ETH on Aurora).
    pub fn internal_withdraw_eth_from_aurora(
        &mut self,
        amount: Wei,
    ) -> Result<(), error::WithdrawError> {
        self.total_eth_supply_on_aurora = self
            .total_eth_supply_on_aurora
            .checked_sub(amount)
            .ok_or(error::WithdrawError::TotalSupplyUnderflow)?;
        Ok(())
    }

    /// Transfer `nETH` tokens (ETH on NEAR).
    pub fn internal_transfer_eth_on_near(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: NEP141Wei,
        #[allow(unused_variables)] memo: &Option<String>,
    ) -> Result<(), error::TransferError> {
        if sender_id == receiver_id {
            return Err(error::TransferError::SelfTransfer);
        }
        if amount == ZERO_NEP141_WEI {
            return Err(error::TransferError::ZeroAmount);
        }

        // Check is account receiver_id exist
        if !self.accounts_contains_key(receiver_id) {
            // Register receiver_id account with 0 balance. We need it because
            // when we retire to get the balance of `receiver_id` it will fail
            // if it does not exist.
            self.internal_register_account(receiver_id);
        }
        self.internal_withdraw_eth_from_near(sender_id, amount)?;
        self.internal_deposit_eth_to_near(receiver_id, amount)?;
        sdk::log!("Transfer {} from {} to {}", amount, sender_id, receiver_id);
        #[cfg(feature = "log")]
        if let Some(memo) = memo {
            sdk::log!("Memo: {}", memo);
        }
        Ok(())
    }

    /// Register a new account with zero balance.
    pub fn internal_register_account(&mut self, account_id: &AccountId) {
        self.accounts_insert(account_id, ZERO_NEP141_WEI);
    }

    /// Return total `nETH` supply (ETH on NEAR).
    pub const fn ft_total_eth_supply_on_near(&self) -> NEP141Wei {
        self.total_eth_supply_on_near
    }

    /// Return total `ETH` supply (ETH on Aurora).
    pub const fn ft_total_eth_supply_on_aurora(&self) -> Wei {
        self.total_eth_supply_on_aurora
    }

    /// Return `nETH` balance of the account (ETH on NEAR).
    pub fn ft_balance_of(&self, account_id: &AccountId) -> NEP141Wei {
        self.get_account_eth_balance(account_id)
            .unwrap_or(ZERO_NEP141_WEI)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ft_transfer_call(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: NEP141Wei,
        memo: &Option<String>,
        msg: String,
        current_account_id: AccountId,
        prepaid_gas: NearGas,
    ) -> Result<PromiseWithCallbackArgs, error::TransferError> {
        // check balance to prevent setting an arbitrary value for `amount` for (receiver_id == receiver_id).
        let balance = self
            .get_account_eth_balance(&sender_id)
            .unwrap_or(ZERO_NEP141_WEI);
        if amount > balance {
            return Err(error::TransferError::InsufficientFunds);
        }
        // Special case for Aurora transfer itself - we shouldn't transfer
        if sender_id != receiver_id {
            self.internal_transfer_eth_on_near(&sender_id, &receiver_id, amount, memo)?;
        }
        let args = serde_json::to_vec(&NEP141FtOnTransferArgs {
            sender_id: sender_id.clone(),
            amount: Balance::new(amount.as_u128()),
            msg,
        })
        .unwrap();

        let data2 = ResolveTransferCallArgs {
            receiver_id: receiver_id.clone(),
            amount,
            sender_id,
        }
        .try_to_vec()
        .unwrap();
        // Initiating receiver's call and the callback
        let ft_on_transfer_call = PromiseCreateArgs {
            target_account_id: receiver_id,
            method: "ft_on_transfer".to_string(),
            args,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: prepaid_gas - GAS_FOR_FT_TRANSFER_CALL - GAS_FOR_RESOLVE_TRANSFER,
        };
        let ft_resolve_transfer_call = PromiseCreateArgs {
            target_account_id: current_account_id,
            method: "ft_resolve_transfer".to_string(),
            args: data2,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_RESOLVE_TRANSFER,
        };
        Ok(PromiseWithCallbackArgs {
            base: ft_on_transfer_call,
            callback: ft_resolve_transfer_call,
        })
    }

    pub fn internal_ft_resolve_transfer(
        &mut self,
        promise_result: PromiseResult,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: NEP141Wei,
    ) -> (NEP141Wei, NEP141Wei) {
        // Get the unused amount from the `ft_on_transfer` call result.
        let unused_amount = match promise_result {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(bytes) => {
                serde_json::from_slice(&bytes).map_or(amount, |unused_amount| {
                    if amount > unused_amount {
                        unused_amount
                    } else {
                        amount
                    }
                })
            }
            PromiseResult::Failed => amount,
        };

        if unused_amount > ZERO_NEP141_WEI {
            let receiver_balance = self
                .get_account_eth_balance(receiver_id)
                .unwrap_or_else(|| {
                    self.accounts_insert(receiver_id, ZERO_NEP141_WEI);
                    ZERO_NEP141_WEI
                });
            if receiver_balance > ZERO_NEP141_WEI {
                let refund_amount = if receiver_balance > unused_amount {
                    unused_amount
                } else {
                    receiver_balance
                };
                self.accounts_insert(receiver_id, receiver_balance - refund_amount);
                sdk::log!(
                    "Decrease receiver {} balance to: {}",
                    receiver_id,
                    receiver_balance - refund_amount
                );

                return if let Some(sender_balance) = self.get_account_eth_balance(sender_id) {
                    self.accounts_insert(sender_id, sender_balance + refund_amount);
                    sdk::log!(
                        "Refund amount {} from {} to {}",
                        refund_amount,
                        receiver_id,
                        sender_id
                    );
                    (amount - refund_amount, ZERO_NEP141_WEI)
                } else {
                    // Sender's account was deleted, so we need to burn tokens.
                    self.total_eth_supply_on_near -= refund_amount;
                    sdk::log!("The account of the sender was deleted");
                    (amount, refund_amount)
                };
            }
        }

        (amount, ZERO_NEP141_WEI)
    }

    pub fn ft_resolve_transfer(
        &mut self,
        promise_result: PromiseResult,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: NEP141Wei,
    ) -> NEP141Wei {
        self.internal_ft_resolve_transfer(promise_result, sender_id, receiver_id, amount)
            .0
    }

    pub fn internal_storage_unregister(
        &mut self,
        account_id: AccountId,
        force: Option<bool>,
    ) -> Result<(NEP141Wei, PromiseBatchAction), error::StorageFundingError> {
        let force = force.unwrap_or(false);
        if let Some(balance) = self.get_account_eth_balance(&account_id) {
            if balance == ZERO_NEP141_WEI || force {
                self.accounts_remove(&account_id);
                self.total_eth_supply_on_near -= balance;
                let storage_deposit = self.storage_balance_of(&account_id);
                let action = PromiseAction::Transfer {
                    // The `+ 1` is to cover the 1 yoctoNEAR necessary to call this function in the first place.
                    amount: storage_deposit.total + Yocto::new(1),
                };
                let promise = PromiseBatchAction {
                    target_account_id: account_id,
                    actions: vec![action],
                };
                Ok((balance, promise))
            } else {
                Err(error::StorageFundingError::UnRegisterPositiveBalance)
            }
        } else {
            sdk::log!("The account {} is not registered", account_id);
            Err(error::StorageFundingError::NotRegistered)
        }
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let required_storage_balance =
            Yocto::new(u128::from(self.account_storage_usage) * sdk::storage_byte_cost());
        StorageBalanceBounds {
            min: required_storage_balance,
            max: Some(required_storage_balance),
        }
    }

    pub fn internal_storage_balance_of(&self, account_id: &AccountId) -> Option<StorageBalance> {
        if self.accounts_contains_key(account_id) {
            Some(StorageBalance {
                total: self.storage_balance_bounds().min,
                available: ZERO_YOCTO,
            })
        } else {
            None
        }
    }

    pub fn storage_balance_of(&self, account_id: &AccountId) -> StorageBalance {
        self.internal_storage_balance_of(account_id)
            .unwrap_or_default()
    }

    // `registration_only` doesn't affect the implementation for vanilla fungible token.
    #[allow(unused_variables)]
    pub fn storage_deposit(
        &mut self,
        predecessor_account_id: AccountId,
        account_id: &AccountId,
        amount: Yocto,
        registration_only: Option<bool>,
    ) -> Result<(StorageBalance, Option<PromiseBatchAction>), error::StorageFundingError> {
        let promise = if self.accounts_contains_key(account_id) {
            sdk::log!("The account is already registered, refunding the deposit");
            amount
        } else {
            let min_balance = self.storage_balance_bounds().min;

            if amount < min_balance {
                return Err(error::StorageFundingError::InsufficientDeposit);
            }

            self.internal_register_account(account_id);
            amount - min_balance
        };
        let promise = if amount > ZERO_YOCTO {
            let action = PromiseAction::Transfer { amount };
            let promise = PromiseBatchAction {
                target_account_id: predecessor_account_id,
                actions: vec![action],
            };
            Some(promise)
        } else {
            None
        };
        let balance = self.internal_storage_balance_of(account_id).unwrap();

        Ok((balance, promise))
    }

    #[allow(clippy::option_if_let_else)]
    pub fn storage_withdraw(
        &mut self,
        account_id: &AccountId,
        amount: Option<Yocto>,
    ) -> Result<StorageBalance, error::StorageFundingError> {
        self.internal_storage_balance_of(account_id).map_or(
            Err(error::StorageFundingError::NotRegistered),
            |storage_balance| match amount {
                Some(amount) if amount > ZERO_YOCTO => {
                    // The available balance is always zero because `StorageBalanceBounds::max` is
                    // equal to `StorageBalanceBounds::min`. Therefore, it is impossible to withdraw
                    // a positive amount.
                    Err(error::StorageFundingError::NoAvailableBalance)
                }
                _ => Ok(storage_balance),
            },
        )
    }

    /// Set account's balance and increment the account counter if the account doesn't exist.
    pub fn accounts_insert(&mut self, account_id: &AccountId, amount: NEP141Wei) {
        if !self.accounts_contains_key(account_id) {
            self.increment_account_counter();
        }

        self.io
            .write_borsh(&Self::account_to_key(account_id), &amount);
    }

    /// Get total unique accounts number. It represents total unique accounts.
    pub fn get_accounts_counter(&self) -> u64 {
        self.io.read_u64(&Self::get_statistic_key()).unwrap_or(0)
    }

    /// Balance of `nETH` (ETH on NEAR).
    pub fn get_account_eth_balance(&self, account_id: &AccountId) -> Option<NEP141Wei> {
        self.io
            .read_storage(&Self::account_to_key(account_id))
            .and_then(|s| NEP141Wei::try_from_slice(&s.to_vec()).ok())
    }

    fn accounts_contains_key(&self, account_id: &AccountId) -> bool {
        self.io.storage_has_key(&Self::account_to_key(account_id))
    }

    fn accounts_remove(&mut self, account_id: &AccountId) {
        self.io.remove_storage(&Self::account_to_key(account_id));
    }

    /// Fungible token key for account id.
    fn account_to_key(account_id: &AccountId) -> Vec<u8> {
        let mut key = storage::bytes_to_key(
            storage::KeyPrefix::EthConnector,
            &[u8::from(storage::EthConnectorStorageId::FungibleToken)],
        );
        key.extend_from_slice(account_id.as_bytes());
        key
    }

    /// Key for storing contract statistics data.
    fn get_statistic_key() -> Vec<u8> {
        storage::bytes_to_key(
            storage::KeyPrefix::EthConnector,
            &[u8::from(
                crate::prelude::EthConnectorStorageId::StatisticsAuroraAccountsCounter,
            )],
        )
    }

    fn increment_account_counter(&mut self) {
        let key = Self::get_statistic_key();
        let accounts_counter = self
            .io
            .read_u64(&key)
            .unwrap_or(0)
            .checked_add(1)
            .expect(crate::errors::ERR_ACCOUNTS_COUNTER_OVERFLOW);
        self.io.write_storage(&key, &accounts_counter.to_le_bytes());
    }
}

pub mod error {
    use crate::errors;
    use crate::prelude::types::balance::error::BalanceOverflowError;

    const TOTAL_SUPPLY_OVERFLOW: &[u8; 25] = errors::ERR_TOTAL_SUPPLY_OVERFLOW;
    const BALANCE_OVERFLOW: &[u8; 20] = errors::ERR_BALANCE_OVERFLOW;
    const NOT_ENOUGH_BALANCE: &[u8; 22] = errors::ERR_NOT_ENOUGH_BALANCE;
    const TOTAL_SUPPLY_UNDERFLOW: &[u8; 26] = errors::ERR_TOTAL_SUPPLY_UNDERFLOW;
    const ZERO_AMOUNT: &[u8; 15] = errors::ERR_ZERO_AMOUNT;
    const SELF_TRANSFER: &[u8; 26] = errors::ERR_SENDER_EQUALS_RECEIVER;

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
    pub enum WithdrawError {
        TotalSupplyUnderflow,
        InsufficientFunds,
        BalanceOverflow(BalanceOverflowError),
    }

    impl AsRef<[u8]> for WithdrawError {
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

    impl From<WithdrawError> for TransferError {
        fn from(err: WithdrawError) -> Self {
            match err {
                WithdrawError::InsufficientFunds => Self::InsufficientFunds,
                WithdrawError::TotalSupplyUnderflow => Self::TotalSupplyUnderflow,
                WithdrawError::BalanceOverflow(_) => Self::BalanceOverflow,
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
