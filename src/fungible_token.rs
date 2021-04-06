#![allow(dead_code)]
use super::*;
use crate::connector::{CONTRACT_FT_KEY, NO_DEPOSIT};
use crate::parameters::*;
use crate::types::*;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use borsh::{BorshDeserialize, BorshSerialize};

const GAS_FOR_RESOLVE_TRANSFER: Gas = 5_000_000_000_000;
const GAS_FOR_FT_TRANSFER_CALL: Gas = 25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER;

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

    pub fn internal_unwrap_balance_of(&self, account_id: AccountId) -> Balance {
        match self.accounts_get(account_id) {
            Some(balance) => u128::try_from_slice(&balance[..]).unwrap(),
            None => sdk::panic_utf8(b"ERR_ACCOUNT_NOT_EXIST"),
        }
    }

    pub fn internal_deposit(&mut self, account_id: AccountId, amount: Balance) {
        let balance = self.internal_unwrap_balance_of(account_id.clone());
        if let Some(new_balance) = balance.checked_add(amount) {
            self.accounts_insert(account_id, new_balance);
            self.total_supply = self
                .total_supply
                .checked_add(amount)
                .expect("Total supply overflow");
        } else {
            sdk::panic_utf8(b"ERR_BALANCE_OVERFLOW");
        }
    }

    pub fn internal_withdraw(&mut self, account_id: AccountId, amount: Balance) {
        let balance = self.internal_unwrap_balance_of(account_id.clone());
        if let Some(new_balance) = balance.checked_sub(amount) {
            self.accounts_insert(account_id, new_balance);
            self.total_supply = self
                .total_supply
                .checked_sub(amount)
                .expect("Total supply overflow");
        } else {
            sdk::panic_utf8(b"ERR_NOT_ENOUGH_BALANCE");
        }
    }

    pub fn internal_transfer(
        &mut self,
        sender_id: &str,
        receiver_id: &str,
        amount: Balance,
        #[allow(unused_variables)] memo: Option<String>,
    ) {
        assert_ne!(
            sender_id, receiver_id,
            "Sender and receiver should be different"
        );
        assert!(amount > 0, "The amount should be a positive number");
        self.internal_withdraw(sender_id.to_string(), amount);
        self.internal_deposit(receiver_id.to_string(), amount);
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

    pub fn internal_register_account(&mut self, account_id: AccountId) {
        self.accounts_insert(account_id, 0)
    }

    pub fn ft_transfer(&mut self, receiver_id: AccountId, amount: Balance, memo: Option<String>) {
        sdk::assert_one_yocto();
        let predecessor_account_id = sdk::predecessor_account_id();
        let sender_id = alloc::str::from_utf8(&predecessor_account_id).unwrap();
        self.internal_transfer(&sender_id, &receiver_id, amount, memo);
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

    pub fn ft_balance_of(&self, account_id: AccountId) -> u128 {
        if let Some(data) = self.accounts_get(account_id) {
            u128::try_from_slice(&data[..]).unwrap()
        } else {
            0
        }
    }

    pub fn ft_balance_of_eth(&self, account_id: AccountId) -> u128 {
        if let Some(data) = self.accounts_get_eth(account_id) {
            u128::try_from_slice(&data[..]).unwrap()
        } else {
            0
        }
    }

    pub fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: Balance,
        memo: Option<String>,
        msg: String,
    ) {
        sdk::assert_one_yocto();
        let predecessor_account_id = sdk::predecessor_account_id();
        let sender_id = alloc::str::from_utf8(&predecessor_account_id).unwrap();
        self.internal_transfer(&sender_id, &receiver_id, amount, memo);
        let data1 = FtOnTransfer {
            amount,
            msg,
            receiver_id: receiver_id.clone(),
        }
        .try_to_vec()
        .unwrap();
        let account_id = String::from_utf8(sdk::current_account_id()).unwrap();
        let data2 = FtResolveTransfer {
            receiver_id: receiver_id.clone(),
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
            sdk::prepaid_gas() - GAS_FOR_FT_TRANSFER_CALL,
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
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: Balance,
    ) -> (u128, u128) {
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
            let receiver_balance =
                if let Some(receiver_balance) = self.accounts_get(receiver_id.clone()) {
                    u128::try_from_slice(&receiver_balance[..]).unwrap()
                } else {
                    self.accounts_insert(receiver_id.clone(), 0);
                    0
                };
            if receiver_balance > 0 {
                let refund_amount = if receiver_balance > unused_amount {
                    unused_amount
                } else {
                    receiver_balance
                };
                self.accounts_insert(receiver_id.clone(), receiver_balance - refund_amount);
                #[cfg(feature = "log")]
                sdk::log(format!(
                    "Decrease receiver {} balance to: {}",
                    receiver_id.clone(),
                    receiver_balance - refund_amount
                ));

                return if let Some(sender_balance) = self.accounts_get(sender_id.clone()) {
                    let sender_balance = u128::try_from_slice(&sender_balance[..]).unwrap();
                    self.accounts_insert(sender_id.clone(), sender_balance + refund_amount);
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
        sender_id: AccountId,
        receiver_id: AccountId,
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
        let account_id = String::from_utf8(account_id_key.clone()).unwrap();
        let force = force.unwrap_or(false);
        if let Some(balance) = self.accounts_get(account_id.clone()) {
            let balance = u128::try_from_slice(&balance[..]).unwrap();
            if balance == 0 || force {
                self.accounts_remove(account_id.clone());
                self.total_supply -= balance;
                let amount = self.storage_balance_bounds().min + 1;
                let promise0 = sdk::promise_batch_create(&account_id_key);
                sdk::promise_batch_action_transfer(promise0, amount);
                Some((account_id, balance))
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

    pub fn internal_storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        if self.accounts_contains_key(account_id) {
            Some(StorageBalance {
                total: self.storage_balance_bounds().min,
                available: 0,
            })
        } else {
            None
        }
    }

    pub fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.internal_storage_balance_of(account_id)
    }

    // `registration_only` doesn't affect the implementation for vanilla fungible token.
    #[allow(unused_variables)]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let amount: Balance = sdk::attached_deposit();
        let predecessor_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
        let account_id = account_id.unwrap_or(predecessor_account_id);
        if self.accounts_contains_key(account_id.clone()) {
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

            self.internal_register_account(account_id.clone());
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
        let predecessor_account_id = String::from_utf8(sdk::predecessor_account_id()).unwrap();
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

    fn ft_key(&self, account_id: AccountId) -> Vec<u8> {
        [CONTRACT_FT_KEY, &account_id].join(".").as_bytes().to_vec()
    }

    pub fn accounts_insert(&self, account_id: AccountId, amount: Balance) {
        sdk::save_contract(&self.ft_key(account_id), &amount)
    }

    fn accounts_contains_key(&self, account_id: AccountId) -> bool {
        sdk::storage_has_key(&self.ft_key(account_id))
    }

    fn accounts_remove(&self, account_id: AccountId) {
        sdk::remove_storage(&self.ft_key(account_id))
    }

    pub fn accounts_get(&self, account_id: AccountId) -> Option<Vec<u8>> {
        sdk::read_storage(&self.ft_key(account_id)[..])
    }

    pub fn accounts_get_eth(&self, account_id: AccountId) -> Option<Vec<u8>> {
        // TODO: modify
        sdk::read_storage(&self.ft_key(account_id)[..])
    }
}
