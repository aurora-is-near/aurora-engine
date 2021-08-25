use crate::json::JsonValue;
use crate::types::*;
use borsh::{BorshDeserialize, BorshSerialize};
#[cfg(feature = "log")]
use prelude::format;
use prelude::BTreeMap;
use {
    crate::connector,
    crate::engine,
    crate::json::parse_json,
    crate::parameters::*,
    crate::sdk,
    crate::storage,
    prelude::{self, Ordering, String, ToString, TryInto, Vec, U256},
};

const GAS_FOR_RESOLVE_TRANSFER: Gas = 5_000_000_000_000;
const GAS_FOR_FT_ON_TRANSFER: Gas = 10_000_000_000_000;

#[derive(Debug, Default, BorshDeserialize, BorshSerialize)]
pub struct FungibleToken {
    /// Total ETH supply on Near (nETH as NEP-141 token)
    pub total_eth_supply_on_near: Balance,

    /// Total ETH supply on Aurora (ETH in Aurora EVM)
    pub total_eth_supply_on_aurora: Balance,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct FungibleTokenMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<[u8; 32]>,
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
                .map(|hash| JsonValue::String(base64::encode(hash)))
                .unwrap_or(JsonValue::Null),
        );
        kvs.insert(
            "decimals".to_string(),
            JsonValue::U64(metadata.decimals as u64),
        );

        JsonValue::Object(kvs)
    }
}

impl FungibleToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Balance of nETH (ETH on NEAR token)
    pub fn internal_unwrap_balance_of_eth_on_near(&self, account_id: &str) -> Balance {
        match self.accounts_get(account_id) {
            Some(balance) => u128::try_from_slice(&balance[..]).unwrap(),
            None => sdk::panic_utf8(b"ERR_ACCOUNT_NOT_EXIST"),
        }
    }

    /// Balance of ETH (ETH on Aurora)
    pub fn internal_unwrap_balance_of_eth_on_aurora(&self, address: EthAddress) -> Balance {
        engine::Engine::get_balance(&prelude::Address(address))
            .raw()
            .as_u128()
    }

    /// Internal ETH deposit to NEAR - nETH (NEP-141)
    pub fn internal_deposit_eth_to_near(&mut self, account_id: &str, amount: Balance) {
        let balance = self.internal_unwrap_balance_of_eth_on_near(account_id);
        if let Some(new_balance) = balance.checked_add(amount) {
            self.accounts_insert(account_id, new_balance);
            self.total_eth_supply_on_near = self
                .total_eth_supply_on_near
                .checked_add(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_BALANCE_OVERFLOW");
        }
    }

    /// Internal ETH deposit to Aurora
    pub fn internal_deposit_eth_to_aurora(&mut self, address: EthAddress, amount: Balance) {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(address);
        if let Some(new_balance) = balance.checked_add(amount) {
            engine::Engine::set_balance(
                &prelude::Address(address),
                &Wei::new(U256::from(new_balance)),
            );
            self.total_eth_supply_on_aurora = self
                .total_eth_supply_on_aurora
                .checked_add(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_BALANCE_OVERFLOW");
        }
    }

    /// Needed by engine to update balances after a transaction (see ApplyBackend for Engine)
    pub(crate) fn internal_set_eth_balance(&mut self, address: EthAddress, new_balance: Balance) {
        let current_balance = self.internal_unwrap_balance_of_eth_on_aurora(address);
        match current_balance.cmp(&new_balance) {
            Ordering::Less => {
                // current_balance is smaller, so we need to deposit
                let diff = new_balance - current_balance;
                self.internal_deposit_eth_to_aurora(address, diff);
            }
            Ordering::Greater => {
                // current_balance is larger, so we need to withdraw
                let diff = current_balance - new_balance;
                self.internal_withdraw_eth_from_aurora(address, diff);
            }
            // if the balances are equal then we do not need to do anything
            Ordering::Equal => (),
        }
    }

    /// Withdraw NEAR tokens
    pub fn internal_withdraw_eth_from_near(&mut self, account_id: &str, amount: Balance) {
        let balance = self.internal_unwrap_balance_of_eth_on_near(account_id);
        if let Some(new_balance) = balance.checked_sub(amount) {
            self.accounts_insert(account_id, new_balance);
            self.total_eth_supply_on_near = self
                .total_eth_supply_on_near
                .checked_sub(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_NOT_ENOUGH_BALANCE");
        }
    }

    /// Withdraw ETH tokens
    pub fn internal_withdraw_eth_from_aurora(&mut self, address: EthAddress, amount: Balance) {
        let balance = self.internal_unwrap_balance_of_eth_on_aurora(address);
        if let Some(new_balance) = balance.checked_sub(amount) {
            engine::Engine::set_balance(
                &prelude::Address(address),
                &Wei::new(U256::from(new_balance)),
            );
            self.total_eth_supply_on_aurora = self
                .total_eth_supply_on_aurora
                .checked_sub(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_NOT_ENOUGH_BALANCE");
        }
    }

    /// Transfer NEAR tokens
    pub fn internal_transfer_eth_on_near(
        &mut self,
        sender_id: &str,
        receiver_id: &str,
        amount: Balance,
        #[allow(unused_variables)] memo: &Option<String>,
    ) {
        assert_ne!(
            sender_id, receiver_id,
            "Sender and receiver should be different"
        );
        assert!(amount > 0, "The amount should be a positive number");
        if !self.accounts_contains_key(receiver_id) {
            // TODO: how does this interact with the storage deposit concept?
            self.internal_register_account(receiver_id)
        }
        self.internal_withdraw_eth_from_near(sender_id, amount);
        self.internal_deposit_eth_to_near(receiver_id, amount);
        crate::log!(&format!(
            "Transfer {} from {} to {}",
            amount, sender_id, receiver_id
        ));
        #[cfg(feature = "log")]
        if let Some(memo) = memo {
            sdk::log(&format!("Memo: {}", memo));
        }
    }

    pub fn internal_register_account(&mut self, account_id: &str) {
        self.accounts_insert(account_id, 0)
    }

    pub fn ft_transfer(&mut self, receiver_id: &str, amount: Balance, memo: &Option<String>) {
        sdk::assert_one_yocto();
        let predecessor_account_id = sdk::predecessor_account_id();
        let sender_id = str_from_slice(&predecessor_account_id);
        self.internal_transfer_eth_on_near(sender_id, receiver_id, amount, memo);
    }

    pub fn ft_total_eth_supply_on_near(&self) -> u128 {
        self.total_eth_supply_on_near
    }

    pub fn ft_total_eth_supply_on_aurora(&self) -> u128 {
        self.total_eth_supply_on_aurora
    }

    pub fn ft_balance_of(&self, account_id: &str) -> u128 {
        if let Some(data) = self.accounts_get(account_id) {
            u128::try_from_slice(&data[..]).unwrap()
        } else {
            0
        }
    }

    pub fn ft_transfer_call(
        &mut self,
        receiver_id: &str,
        amount: Balance,
        memo: &Option<String>,
        msg: String,
    ) {
        let predecessor_account_id = sdk::predecessor_account_id();
        let sender_id = str_from_slice(&predecessor_account_id);
        // Special case for Aurora transfer itself - we shouldn't transfer
        if sender_id != receiver_id {
            self.internal_transfer_eth_on_near(sender_id, receiver_id, amount, memo);
        }
        let data1: String = NEP141FtOnTransferArgs {
            amount,
            msg,
            sender_id: receiver_id.to_string(),
        }
        .try_into()
        .unwrap();

        let account_id = String::from_utf8(sdk::current_account_id()).unwrap();
        let data2 = FtResolveTransfer {
            receiver_id: receiver_id.to_string(),
            amount,
            current_account_id: account_id,
        }
        .try_to_vec()
        .unwrap();
        // Initiating receiver's call and the callback
        let promise0 = sdk::promise_create(
            receiver_id.as_bytes(),
            b"ft_on_transfer",
            data1.as_bytes(),
            connector::NO_DEPOSIT,
            GAS_FOR_FT_ON_TRANSFER,
        );
        let promise1 = sdk::promise_then(
            promise0,
            &sdk::current_account_id(),
            b"ft_resolve_transfer",
            &data2[..],
            connector::NO_DEPOSIT,
            GAS_FOR_RESOLVE_TRANSFER,
        );
        sdk::promise_return(promise1);
    }

    pub fn internal_ft_resolve_transfer(
        &mut self,
        sender_id: &str,
        receiver_id: &str,
        amount: Balance,
    ) -> (u128, u128) {
        // Get the unused amount from the `ft_on_transfer` call result.
        let unused_amount = match sdk::promise_result(0) {
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
            let receiver_balance = if let Some(receiver_balance) = self.accounts_get(receiver_id) {
                u128::try_from_slice(&receiver_balance[..]).unwrap()
            } else {
                self.accounts_insert(receiver_id, 0);
                0
            };
            if receiver_balance > 0 {
                let refund_amount = if receiver_balance > unused_amount {
                    unused_amount
                } else {
                    receiver_balance
                };
                self.accounts_insert(receiver_id, receiver_balance - refund_amount);
                crate::log!(&format!(
                    "Decrease receiver {} balance to: {}",
                    receiver_id,
                    receiver_balance - refund_amount
                ));

                return if let Some(sender_balance) = self.accounts_get(sender_id) {
                    let sender_balance = u128::try_from_slice(&sender_balance[..]).unwrap();
                    self.accounts_insert(sender_id, sender_balance + refund_amount);
                    crate::log!(&format!(
                        "Refund amount {} from {} to {}",
                        refund_amount, receiver_id, sender_id
                    ));
                    (amount - refund_amount, 0)
                } else {
                    // Sender's account was deleted, so we need to burn tokens.
                    self.total_eth_supply_on_near -= refund_amount;
                    crate::log!("The account of the sender was deleted");
                    (amount, refund_amount)
                };
            }
        }
        (amount, 0)
    }

    pub fn ft_resolve_transfer(
        &mut self,
        sender_id: &str,
        receiver_id: &str,
        amount: u128,
    ) -> u128 {
        self.internal_ft_resolve_transfer(sender_id, receiver_id, amount)
            .0
    }

    pub fn internal_storage_unregister(
        &mut self,
        force: Option<bool>,
    ) -> Option<(AccountId, Balance)> {
        sdk::assert_one_yocto();
        let account_id_key = sdk::predecessor_account_id();
        let account_id = str_from_slice(&account_id_key);
        let force = force.unwrap_or(false);
        if let Some(balance) = self.accounts_get(account_id) {
            let balance = u128::try_from_slice(&balance[..]).unwrap();
            if balance == 0 || force {
                self.accounts_remove(account_id);
                self.total_eth_supply_on_near -= balance;
                let amount = self.storage_balance_bounds().min + 1;
                let promise0 = sdk::promise_batch_create(&account_id_key);
                sdk::promise_batch_action_transfer(promise0, amount);
                Some((account_id.to_string(), balance))
            } else {
                sdk::panic_utf8(b"ERR_FAILED_UNREGISTER_ACCOUNT_POSITIVE_BALANCE")
            }
        } else {
            crate::log!(&format!("The account {} is not registered", &account_id));
            None
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

    pub fn internal_storage_balance_of(&self, account_id: &str) -> Option<StorageBalance> {
        if self.accounts_contains_key(account_id) {
            Some(StorageBalance {
                total: self.storage_balance_bounds().min,
                available: 0,
            })
        } else {
            None
        }
    }

    pub fn storage_balance_of(&self, account_id: &str) -> StorageBalance {
        self.internal_storage_balance_of(account_id)
            .unwrap_or_default()
    }

    // `registration_only` doesn't affect the implementation for vanilla fungible token.
    #[allow(unused_variables)]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<&AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let amount: Balance = sdk::attached_deposit();
        let predecessor_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        let account_id = account_id.unwrap_or(&predecessor_account_id);
        if self.accounts_contains_key(account_id) {
            crate::log!("The account is already registered, refunding the deposit");
            if amount > 0 {
                let promise0 = sdk::promise_batch_create(&sdk::predecessor_account_id());
                sdk::promise_batch_action_transfer(promise0, amount);
            }
        } else {
            let min_balance = self.storage_balance_bounds().min;
            if amount < min_balance {
                sdk::panic_utf8(b"ERR_ATTACHED_DEPOSIT_NOT_ENOUGH");
            }

            self.internal_register_account(account_id);
            let refund = amount - min_balance;
            if refund > 0 {
                let promise0 = sdk::promise_batch_create(&sdk::predecessor_account_id());
                sdk::promise_batch_action_transfer(promise0, refund);
            }
        }
        self.internal_storage_balance_of(account_id).unwrap()
    }

    #[allow(dead_code)]
    pub fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        self.internal_storage_unregister(force).is_some()
    }

    pub fn storage_withdraw(&mut self, amount: Option<u128>) -> StorageBalance {
        let predecessor_account_id_bytes = sdk::predecessor_account_id();
        let predecessor_account_id = str_from_slice(&predecessor_account_id_bytes);
        if let Some(storage_balance) = self.internal_storage_balance_of(predecessor_account_id) {
            match amount {
                Some(amount) if amount > 0 => {
                    sdk::panic_utf8(b"ERR_WRONG_AMOUNT");
                }
                _ => storage_balance,
            }
        } else {
            sdk::panic_utf8(b"ERR_ACCOUNT_NOT_REGISTERED");
        }
    }

    /// Insert account.
    /// Calculate total unique accounts
    pub fn accounts_insert(&self, account_id: &str, amount: Balance) {
        if !self.accounts_contains_key(account_id) {
            let key = Self::get_statistic_key();
            let accounts_counter = sdk::read_u64(&key)
                .unwrap_or(0)
                .checked_add(1)
                .expect("ERR_ACCOUNTS_COUNTER_OVERFLOW");
            sdk::write_storage(&key, &accounts_counter.to_le_bytes());
        }
        sdk::save_contract(&Self::account_to_key(account_id), &amount);
    }

    /// Get accounts counter for statistics
    /// It represents total unique accounts.
    pub fn get_accounts_counter(&self) -> u64 {
        sdk::read_u64(&Self::get_statistic_key()).unwrap_or(0)
    }

    fn accounts_contains_key(&self, account_id: &str) -> bool {
        sdk::storage_has_key(&Self::account_to_key(account_id))
    }

    fn accounts_remove(&self, account_id: &str) {
        sdk::remove_storage(&Self::account_to_key(account_id))
    }

    pub fn accounts_get(&self, account_id: &str) -> Option<Vec<u8>> {
        sdk::read_storage(&Self::account_to_key(account_id))
    }

    /// Fungible token key
    fn account_to_key(account_id: &str) -> Vec<u8> {
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
            storage::KeyPrefix::EthConnector,
            &[storage::EthConnectorStorageId::StatisticsAuroraAccountsCounter as u8],
        )
    }
}
