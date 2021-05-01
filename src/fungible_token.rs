#![allow(dead_code)]
use super::*;
use crate::connector::{CONTRACT_FT_KEY, NO_DEPOSIT};
use crate::engine::Engine;
use crate::parameters::*;
use crate::prelude;
use crate::prelude::U256;
use crate::types::*;
#[cfg(feature = "log")]
use alloc::format;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use borsh::{BorshDeserialize, BorshSerialize};

const GAS_FOR_RESOLVE_TRANSFER: Gas = 5_000_000_000_000;
const GAS_FOR_FT_ON_TRANSFER: Gas = 10_000_000_000_000;

#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct FungibleToken {
    /// Total supply of the all token.
    pub total_supply: Balance,

    /// Total supply of the all NEAR token.
    pub total_supply_near: Balance,

    /// Total supply of the all ETH token.
    pub total_supply_eth: Balance,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

impl Default for fungible_token::FungibleToken {
    fn default() -> Self {
        Self::new()
    }
}

impl FungibleToken {
    pub fn new() -> Self {
        Self {
            total_supply: 0,
            total_supply_near: 0,
            total_supply_eth: 0,
            account_storage_usage: 0,
        }
    }

    /// Balance of NEAR tokens
    pub fn internal_unwrap_balance_of(&self, account_id: &str) -> Balance {
        match self.accounts_get(account_id) {
            Some(balance) => u128::try_from_slice(&balance[..]).unwrap(),
            None => sdk::panic_utf8(b"ERR_ACCOUNT_NOT_EXIST"),
        }
    }

    /// Balance of ETH tokens
    pub fn internal_unwrap_balance_of_eth(&self, address: EthAddress) -> Balance {
        Engine::get_balance(&prelude::Address(address)).as_u128()
    }

    /// Internal deposit NEAR - NEP-141
    pub fn internal_deposit(&mut self, account_id: &str, amount: Balance) {
        let balance = self.internal_unwrap_balance_of(account_id);
        if let Some(new_balance) = balance.checked_add(amount) {
            self.accounts_insert(account_id, new_balance);
            self.total_supply_near = self
                .total_supply_near
                .checked_add(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
            self.total_supply = self
                .total_supply
                .checked_add(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_BALANCE_OVERFLOW");
        }
    }

    /// Internal deposit ETH (nETH)
    pub fn internal_deposit_eth(&mut self, address: EthAddress, amount: Balance) {
        let balance = self.internal_unwrap_balance_of_eth(address);
        if let Some(new_balance) = balance.checked_add(amount) {
            Engine::set_balance(&prelude::Address(address), &U256::from(new_balance));
            self.total_supply_eth = self
                .total_supply_eth
                .checked_add(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
            self.total_supply = self
                .total_supply
                .checked_add(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_BALANCE_OVERFLOW");
        }
    }

    /// Withdraw NEAR tokens
    pub fn internal_withdraw(&mut self, account_id: &str, amount: Balance) {
        let balance = self.internal_unwrap_balance_of(account_id);
        if let Some(new_balance) = balance.checked_sub(amount) {
            self.accounts_insert(account_id, new_balance);
            self.total_supply_near = self
                .total_supply_near
                .checked_sub(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
            self.total_supply = self
                .total_supply
                .checked_sub(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_NOT_ENOUGH_BALANCE");
        }
    }

    /// Withdraw ETH tokens
    pub fn internal_withdraw_eth(&mut self, address: EthAddress, amount: Balance) {
        let balance = self.internal_unwrap_balance_of_eth(address);
        if let Some(new_balance) = balance.checked_sub(amount) {
            Engine::set_balance(&prelude::Address(address), &U256::from(new_balance));
            self.total_supply_eth = self
                .total_supply_eth
                .checked_sub(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
            self.total_supply = self
                .total_supply
                .checked_sub(amount)
                .expect("ERR_TOTAL_SUPPLY_OVERFLOW");
        } else {
            sdk::panic_utf8(b"ERR_NOT_ENOUGH_BALANCE");
        }
    }

    /// Transfer NEAR tokens
    pub fn internal_transfer(
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
        self.internal_withdraw(sender_id, amount);
        self.internal_deposit(receiver_id, amount);
        #[cfg(feature = "log")]
        sdk::log(format!(
            "Transfer {} from {} to {}",
            amount, sender_id, receiver_id
        ));
        #[cfg(feature = "log")]
        if let Some(memo) = memo {
            sdk::log(format!("Memo: {}", memo));
        }
    }

    pub fn internal_register_account(&mut self, account_id: &str) {
        self.accounts_insert(account_id, 0)
    }

    pub fn ft_transfer(&mut self, receiver_id: &str, amount: Balance, memo: &Option<String>) {
        sdk::assert_one_yocto();
        let predecessor_account_id = sdk::predecessor_account_id();
        let sender_id = str_from_slice(&predecessor_account_id);
        self.internal_transfer(sender_id, receiver_id, amount, memo);
    }

    pub fn ft_total_supply(&self) -> u128 {
        self.total_supply
    }

    pub fn ft_total_supply_near(&self) -> u128 {
        self.total_supply_near
    }

    pub fn ft_total_supply_eth(&self) -> u128 {
        self.total_supply_eth
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
        sdk::assert_one_yocto();
        let predecessor_account_id = sdk::predecessor_account_id();
        let sender_id = str_from_slice(&predecessor_account_id);
        // Special case for Aurora transfer itself - we shouldn't transfer
        if sender_id != receiver_id {
            self.internal_transfer(sender_id, receiver_id, amount, memo);
        }

        let data1 = FtOnTransfer {
            amount,
            msg,
            receiver_id: receiver_id.to_string(),
        }
        .try_to_vec()
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
            &data1[..],
            NO_DEPOSIT,
            GAS_FOR_FT_ON_TRANSFER,
        );
        let promise1 = sdk::promise_then(
            promise0,
            &sdk::current_account_id(),
            b"ft_resolve_transfer",
            &data2[..],
            NO_DEPOSIT,
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
        assert_eq!(sdk::promise_results_count(), 1);
        // Get the unused amount from the `ft_on_transfer` call result.
        let unused_amount = match sdk::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(value) => {
                if let Ok(unused_amount) = Balance::try_from_slice(&value[..]) {
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
                #[cfg(feature = "log")]
                sdk::log(format!(
                    "Decrease receiver {} balance to: {}",
                    receiver_id,
                    receiver_balance - refund_amount
                ));

                return if let Some(sender_balance) = self.accounts_get(sender_id) {
                    let sender_balance = u128::try_from_slice(&sender_balance[..]).unwrap();
                    self.accounts_insert(sender_id, sender_balance + refund_amount);
                    #[cfg(feature = "log")]
                    sdk::log(format!(
                        "Refund amount {} from {} to {}",
                        refund_amount, receiver_id, sender_id
                    ));
                    (amount - refund_amount, 0)
                } else {
                    // Sender's account was deleted, so we need to burn tokens.
                    self.total_supply -= refund_amount;
                    #[cfg(feature = "log")]
                    sdk::log("The account of the sender was deleted".into());
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
                self.total_supply -= balance;
                let amount = self.storage_balance_bounds().min + 1;
                let promise0 = sdk::promise_batch_create(&account_id_key);
                sdk::promise_batch_action_transfer(promise0, amount);
                Some((account_id.to_string(), balance))
            } else {
                sdk::panic_utf8(b"ERR_FAILED_UNREGISTER_ACCOUNT_POSITIVE_BALANCE")
            }
        } else {
            #[cfg(feature = "log")]
            sdk::log(format!("The account {} is not registered", &account_id));
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

    pub fn storage_balance_of(&self, account_id: &str) -> Option<StorageBalance> {
        self.internal_storage_balance_of(account_id)
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
            #[cfg(feature = "log")]
            sdk::log("The account is already registered, refunding the deposit".into());
            if amount > 0 {
                let promise0 = sdk::promise_batch_create(&sdk::predecessor_account_id());
                sdk::promise_batch_action_transfer(promise0, amount);
            }
        } else {
            let min_balance = self.storage_balance_bounds().min;
            if amount < min_balance {
                #[cfg(feature = "log")]
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

    pub fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        self.internal_storage_unregister(force).is_some()
    }

    pub fn storage_withdraw(&mut self, amount: Option<u128>) -> StorageBalance {
        sdk::assert_one_yocto();
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

    fn ft_key(&self, account_id: &str) -> Vec<u8> {
        [CONTRACT_FT_KEY, &account_id].join(".").as_bytes().to_vec()
    }

    pub fn accounts_insert(&self, account_id: &str, amount: Balance) {
        sdk::save_contract(&self.ft_key(account_id), &amount)
    }

    fn accounts_contains_key(&self, account_id: &str) -> bool {
        sdk::storage_has_key(&self.ft_key(account_id))
    }

    fn accounts_remove(&self, account_id: &str) {
        sdk::remove_storage(&self.ft_key(account_id))
    }

    pub fn accounts_get(&self, account_id: &str) -> Option<Vec<u8>> {
        sdk::read_storage(&self.ft_key(account_id)[..])
    }

    pub fn accounts_get_eth(&self, account_id: &str) -> Option<Vec<u8>> {
        // TODO: modify
        sdk::read_storage(&self.ft_key(account_id)[..])
    }
}
