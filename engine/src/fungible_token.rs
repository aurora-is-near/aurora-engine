use crate::connector::ZERO_ATTACHED_BALANCE;
use crate::engine;
use crate::json::{parse_json, JsonValue};
use crate::parameters::{NEP141FtOnTransferArgs, ResolveTransferCallArgs, StorageBalance};
use crate::prelude::account_id::AccountId;
use crate::prelude::{
    sdk, storage, vec, Address, BTreeMap, Balance, BorshDeserialize, BorshSerialize, EthAddress,
    NearGas, PromiseAction, PromiseBatchAction, PromiseCreateArgs, PromiseResult,
    PromiseWithCallbackArgs, StorageBalanceBounds, StorageUsage, String, ToString, TryInto, Vec,
    Wei, U256,
};
use aurora_engine_sdk::io::{StorageIntermediate, IO};

/// Gas for `resolve_transfer`: 5 TGas
const GAS_FOR_RESOLVE_TRANSFER: NearGas = NearGas::new(5_000_000_000_000);
/// Gas for `ft_on_transfer`
const GAS_FOR_FT_TRANSFER_CALL: NearGas = NearGas::new(25_000_000_000_000);

#[derive(Debug, Default, BorshDeserialize, BorshSerialize)]
pub struct FungibleToken {
    /// Total ETH supply on Near (nETH as NEP-141 token)
    pub total_eth_supply_on_near: Balance,

    /// Total ETH supply on Aurora (ETH in Aurora EVM)
    pub total_eth_supply_on_aurora: Balance,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

impl FungibleToken {
    pub fn ops<I: IO>(self, io: I) -> FungibleTokenOps<I> {
        FungibleTokenOps {
            total_eth_supply_on_near: self.total_eth_supply_on_near,
            total_eth_supply_on_aurora: self.total_eth_supply_on_aurora,
            account_storage_usage: self.account_storage_usage,
            io,
        }
    }
}

pub struct FungibleTokenOps<I: IO> {
    /// Total ETH supply on Near (nETH as NEP-141 token)
    pub total_eth_supply_on_near: Balance,

    /// Total ETH supply on Aurora (ETH in Aurora EVM)
    pub total_eth_supply_on_aurora: Balance,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,

    io: I,
}

/// Fungible token Reference hash type.
/// Used for FungibleTokenMetadata
#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct FungibleReferenceHash([u8; 32]);

impl FungibleReferenceHash {
    /// Encode to base64-encoded string
    pub fn encode(&self) -> String {
        base64::encode(self)
    }
}

impl AsRef<[u8]> for FungibleReferenceHash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct FungibleTokenMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<FungibleReferenceHash>,
    pub decimals: u8,
}

impl Default for FungibleTokenMetadata {
    fn default() -> Self {
        Self {
            spec: "ft-1.0.0".to_string(),
            name: "Ether".to_string(),
            symbol: "ETH".to_string(),
            icon: Some("data:image/png;base64,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".to_string()),
            reference: None,
            reference_hash: None,
            decimals: 18,
        }
    }
}

impl From<FungibleTokenMetadata> for JsonValue {
    fn from(metadata: FungibleTokenMetadata) -> Self {
        let mut kvs = BTreeMap::new();
        kvs.insert("spec".to_string(), JsonValue::String(metadata.spec));
        kvs.insert("name".to_string(), JsonValue::String(metadata.name));
        kvs.insert("symbol".to_string(), JsonValue::String(metadata.symbol));
        kvs.insert(
            "icon".to_string(),
            metadata
                .icon
                .map(JsonValue::String)
                .unwrap_or(JsonValue::Null),
        );
        kvs.insert(
            "reference".to_string(),
            metadata
                .reference
                .map(JsonValue::String)
                .unwrap_or(JsonValue::Null),
        );
        kvs.insert(
            "reference_hash".to_string(),
            metadata
                .reference_hash
                .map(|hash| JsonValue::String(hash.encode()))
                .unwrap_or(JsonValue::Null),
        );
        kvs.insert(
            "decimals".to_string(),
            JsonValue::U64(metadata.decimals as u64),
        );

        JsonValue::Object(kvs)
    }
}

impl<I: IO + Copy> FungibleTokenOps<I> {
    pub fn new(io: I) -> Self {
        FungibleToken::default().ops(io)
    }

    pub fn data(&self) -> FungibleToken {
        FungibleToken {
            total_eth_supply_on_near: self.total_eth_supply_on_near,
            total_eth_supply_on_aurora: self.total_eth_supply_on_aurora,
            account_storage_usage: self.account_storage_usage,
        }
    }

    /// Balance of ETH (ETH on Aurora)
    pub fn internal_unwrap_balance_of_eth_on_aurora(
        &self,
        address: EthAddress,
    ) -> Result<Balance, crate::prelude::types::error::BalanceOverflowError> {
        engine::get_balance(&self.io, &Address(address)).try_into_u128()
    }

    /// Internal ETH deposit to NEAR - nETH (NEP-141)
    pub fn internal_deposit_eth_to_near(
        &mut self,
        account_id: &AccountId,
        amount: Balance,
    ) -> Result<(), error::DepositError> {
        let balance = self.get_account_eth_balance(account_id).unwrap_or(0);
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

    /// Internal ETH deposit to Aurora
    pub fn internal_deposit_eth_to_aurora(
        &mut self,
        address: EthAddress,
        amount: Balance,
    ) -> Result<(), error::DepositError> {
        let balance = self
            .internal_unwrap_balance_of_eth_on_aurora(address)
            .map_err(|_| error::DepositError::BalanceOverflow)?;
        let new_balance = balance
            .checked_add(amount)
            .ok_or(error::DepositError::BalanceOverflow)?;
        engine::set_balance(
            &mut self.io,
            &Address(address),
            &Wei::new(U256::from(new_balance)),
        );
        self.total_eth_supply_on_aurora = self
            .total_eth_supply_on_aurora
            .checked_add(amount)
            .ok_or(error::DepositError::TotalSupplyOverflow)?;
        Ok(())
    }

    /// Withdraw NEAR tokens
    pub fn internal_withdraw_eth_from_near(
        &mut self,
        account_id: &AccountId,
        amount: Balance,
    ) -> Result<(), error::WithdrawError> {
        let balance = self.get_account_eth_balance(account_id).unwrap_or(0);
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

    /// Withdraw ETH tokens
    pub fn internal_withdraw_eth_from_aurora(
        &mut self,
        address: EthAddress,
        amount: Balance,
    ) -> Result<(), error::WithdrawError> {
        let balance = self
            .internal_unwrap_balance_of_eth_on_aurora(address)
            .map_err(error::WithdrawError::BalanceOverflow)?;
        let new_balance = balance
            .checked_sub(amount)
            .ok_or(error::WithdrawError::InsufficientFunds)?;
        engine::set_balance(
            &mut self.io,
            &Address(address),
            &Wei::new(U256::from(new_balance)),
        );
        self.total_eth_supply_on_aurora = self
            .total_eth_supply_on_aurora
            .checked_sub(amount)
            .ok_or(error::WithdrawError::TotalSupplyUnderflow)?;
        Ok(())
    }

    /// Transfer NEAR tokens
    pub fn internal_transfer_eth_on_near(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: Balance,
        #[allow(unused_variables)] memo: &Option<String>,
    ) -> Result<(), error::TransferError> {
        if sender_id == receiver_id {
            return Err(error::TransferError::SelfTransfer);
        }
        if amount == 0 {
            return Err(error::TransferError::ZeroAmount);
        }

        // Check is account receiver_id exist
        if !self.accounts_contains_key(receiver_id) {
            // Register receiver_id account with 0 balance. We need it because
            // when we retire to get the balance of `receiver_id` it will fail
            // if it does not exist.
            self.internal_register_account(receiver_id)
        }
        self.internal_withdraw_eth_from_near(sender_id, amount)?;
        self.internal_deposit_eth_to_near(receiver_id, amount)?;
        sdk::log!(&crate::prelude::format!(
            "Transfer {} from {} to {}",
            amount,
            sender_id,
            receiver_id
        ));
        #[cfg(feature = "log")]
        if let Some(memo) = memo {
            sdk::log!(&crate::prelude::format!("Memo: {}", memo));
        }
        Ok(())
    }

    pub fn internal_register_account(&mut self, account_id: &AccountId) {
        self.accounts_insert(account_id, 0)
    }

    pub fn ft_total_eth_supply_on_near(&self) -> Balance {
        self.total_eth_supply_on_near
    }

    pub fn ft_total_eth_supply_on_aurora(&self) -> Balance {
        self.total_eth_supply_on_aurora
    }

    pub fn ft_balance_of(&self, account_id: &AccountId) -> Balance {
        self.get_account_eth_balance(account_id).unwrap_or(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ft_transfer_call(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: Balance,
        memo: &Option<String>,
        msg: String,
        current_account_id: AccountId,
        prepaid_gas: NearGas,
    ) -> Result<PromiseWithCallbackArgs, error::TransferError> {
        // Special case for Aurora transfer itself - we shouldn't transfer
        if sender_id != receiver_id {
            self.internal_transfer_eth_on_near(&sender_id, &receiver_id, amount, memo)?;
        }
        let data1: String = NEP141FtOnTransferArgs {
            amount,
            msg,
            sender_id: sender_id.clone(),
        }
        .try_into()
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
            args: data1.into_bytes(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: (prepaid_gas - GAS_FOR_FT_TRANSFER_CALL - GAS_FOR_RESOLVE_TRANSFER)
                .into_u64(),
        };
        let ft_resolve_transfer_call = PromiseCreateArgs {
            target_account_id: current_account_id,
            method: "ft_resolve_transfer".to_string(),
            args: data2,
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_RESOLVE_TRANSFER.into_u64(),
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
        amount: Balance,
    ) -> (Balance, Balance) {
        // Get the unused amount from the `ft_on_transfer` call result.
        let unused_amount = match promise_result {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Some(unused_amount) =
                    parse_json(value.as_slice()).and_then(|x| (&x).try_into().ok())
                {
                    if amount > unused_amount {
                        unused_amount
                    } else {
                        amount
                    }
                } else {
                    amount
                }
            }
            PromiseResult::Failed => amount,
        };

        if unused_amount > 0 {
            let receiver_balance = self
                .get_account_eth_balance(receiver_id)
                .unwrap_or_else(|| {
                    self.accounts_insert(receiver_id, 0);
                    0
                });
            if receiver_balance > 0 {
                let refund_amount = if receiver_balance > unused_amount {
                    unused_amount
                } else {
                    receiver_balance
                };
                self.accounts_insert(receiver_id, receiver_balance - refund_amount);
                sdk::log!(&crate::prelude::format!(
                    "Decrease receiver {} balance to: {}",
                    receiver_id,
                    receiver_balance - refund_amount
                ));

                return if let Some(sender_balance) = self.get_account_eth_balance(sender_id) {
                    self.accounts_insert(sender_id, sender_balance + refund_amount);
                    sdk::log!(&crate::prelude::format!(
                        "Refund amount {} from {} to {}",
                        refund_amount,
                        receiver_id,
                        sender_id
                    ));
                    (amount - refund_amount, 0)
                } else {
                    // Sender's account was deleted, so we need to burn tokens.
                    self.total_eth_supply_on_near -= refund_amount;
                    sdk::log!("The account of the sender was deleted");
                    (amount, refund_amount)
                };
            }
        }
        (amount, 0)
    }

    pub fn ft_resolve_transfer(
        &mut self,
        promise_result: PromiseResult,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        amount: Balance,
    ) -> Balance {
        self.internal_ft_resolve_transfer(promise_result, sender_id, receiver_id, amount)
            .0
    }

    pub fn internal_storage_unregister(
        &mut self,
        account_id: AccountId,
        force: Option<bool>,
    ) -> Result<(Balance, PromiseBatchAction), error::StorageFundingError> {
        let force = force.unwrap_or(false);
        if let Some(balance) = self.get_account_eth_balance(&account_id) {
            if balance == 0 || force {
                self.accounts_remove(&account_id);
                self.total_eth_supply_on_near -= balance;
                let storage_deposit = self.storage_balance_of(&account_id);
                let action = PromiseAction::Transfer {
                    // The `+ 1` is to cover the 1 yoctoNEAR necessary to call this function in the first place.
                    amount: storage_deposit.total + 1,
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
            sdk::log!(&crate::prelude::format!(
                "The account {} is not registered",
                account_id
            ));
            Err(error::StorageFundingError::NotRegistered)
        }
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let required_storage_balance =
            Balance::from(self.account_storage_usage) * sdk::storage_byte_cost();
        StorageBalanceBounds {
            min: required_storage_balance,
            max: Some(required_storage_balance),
        }
    }

    pub fn internal_storage_balance_of(&self, account_id: &AccountId) -> Option<StorageBalance> {
        if self.accounts_contains_key(account_id) {
            Some(StorageBalance {
                total: self.storage_balance_bounds().min,
                available: 0,
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
        amount: Balance,
        registration_only: Option<bool>,
    ) -> Result<(StorageBalance, Option<PromiseBatchAction>), error::StorageFundingError> {
        let promise = if self.accounts_contains_key(account_id) {
            sdk::log!("The account is already registered, refunding the deposit");
            if amount > 0 {
                let action = PromiseAction::Transfer { amount };
                let promise = PromiseBatchAction {
                    target_account_id: predecessor_account_id,
                    actions: vec![action],
                };
                Some(promise)
            } else {
                None
            }
        } else {
            let min_balance = self.storage_balance_bounds().min;
            if amount < min_balance {
                return Err(error::StorageFundingError::InsufficientDeposit);
            }

            self.internal_register_account(account_id);
            let refund = amount - min_balance;
            if refund > 0 {
                let action = PromiseAction::Transfer { amount: refund };
                let promise = PromiseBatchAction {
                    target_account_id: predecessor_account_id,
                    actions: vec![action],
                };
                Some(promise)
            } else {
                None
            }
        };
        let balance = self.internal_storage_balance_of(account_id).unwrap();
        Ok((balance, promise))
    }

    pub fn storage_withdraw(
        &mut self,
        account_id: &AccountId,
        amount: Option<u128>,
    ) -> Result<StorageBalance, error::StorageFundingError> {
        if let Some(storage_balance) = self.internal_storage_balance_of(account_id) {
            match amount {
                Some(amount) if amount > 0 => {
                    // The available balance is always zero because `StorageBalanceBounds::max` is
                    // equal to `StorageBalanceBounds::min`. Therefore it is impossible to withdraw
                    // a positive amount.
                    Err(error::StorageFundingError::NoAvailableBalance)
                }
                _ => Ok(storage_balance),
            }
        } else {
            Err(error::StorageFundingError::NotRegistered)
        }
    }

    /// Insert account.
    /// Calculate total unique accounts
    pub fn accounts_insert(&mut self, account_id: &AccountId, amount: Balance) {
        if !self.accounts_contains_key(account_id) {
            let key = Self::get_statistic_key();
            let accounts_counter = self
                .io
                .read_u64(&key)
                .unwrap_or(0)
                .checked_add(1)
                .expect("ERR_ACCOUNTS_COUNTER_OVERFLOW");
            self.io.write_storage(&key, &accounts_counter.to_le_bytes());
        }
        self.io
            .write_borsh(&Self::account_to_key(account_id), &amount);
    }

    /// Get accounts counter for statistics
    /// It represents total unique accounts.
    pub fn get_accounts_counter(&self) -> u64 {
        self.io.read_u64(&Self::get_statistic_key()).unwrap_or(0)
    }

    fn accounts_contains_key(&self, account_id: &AccountId) -> bool {
        self.io.storage_has_key(&Self::account_to_key(account_id))
    }

    fn accounts_remove(&mut self, account_id: &AccountId) {
        self.io.remove_storage(&Self::account_to_key(account_id));
    }

    /// Balance of nETH (ETH on NEAR token)
    pub fn get_account_eth_balance(&self, account_id: &AccountId) -> Option<Balance> {
        self.io
            .read_storage(&Self::account_to_key(account_id))
            .and_then(|s| Balance::try_from_slice(&s.to_vec()).ok())
    }

    /// Fungible token key
    fn account_to_key(account_id: &AccountId) -> Vec<u8> {
        let mut key = storage::bytes_to_key(
            storage::KeyPrefix::EthConnector,
            &[storage::EthConnectorStorageId::FungibleToken as u8],
        );
        key.extend_from_slice(account_id.as_bytes());
        key
    }

    /// Key for store contract statistics data
    fn get_statistic_key() -> Vec<u8> {
        storage::bytes_to_key(
            crate::prelude::storage::KeyPrefix::EthConnector,
            &[crate::prelude::EthConnectorStorageId::StatisticsAuroraAccountsCounter as u8],
        )
    }
}

pub mod error {
    use crate::prelude::types::error::BalanceOverflowError;

    const TOTAL_SUPPLY_OVERFLOW: &[u8; 25] = b"ERR_TOTAL_SUPPLY_OVERFLOW";
    const BALANCE_OVERFLOW: &[u8; 20] = b"ERR_BALANCE_OVERFLOW";
    const NOT_ENOUGH_BALANCE: &[u8; 22] = b"ERR_NOT_ENOUGH_BALANCE";
    const TOTAL_SUPPLY_UNDERFLOW: &[u8; 26] = b"ERR_TOTAL_SUPPLY_UNDERFLOW";
    const ZERO_AMOUNT: &[u8; 15] = b"ERR_ZERO_AMOUNT";
    const SELF_TRANSFER: &[u8; 26] = b"ERR_SENDER_EQUALS_RECEIVER";

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
                Self::NotRegistered => b"ERR_ACCOUNT_NOT_REGISTERED",
                Self::NoAvailableBalance => b"ERR_NO_AVAILABLE_BALANCE",
                Self::InsufficientDeposit => b"ERR_ATTACHED_DEPOSIT_NOT_ENOUGH",
                Self::UnRegisterPositiveBalance => {
                    b"ERR_FAILED_UNREGISTER_ACCOUNT_POSITIVE_BALANCE"
                }
            }
        }
    }
}
