//! NEP-141 and NEP-245 receiver for the Aurora Engine integration tests.
//!
//! The transfer message selects the behavior:
//! - `return:N` reports `N` tokens of every transferred token as unused;
//! - `panic` fails the call, so the token contract refunds everything;
//! - `spend:account:N` forwards `N` tokens, then requests a refund of the whole transfer;
//! - any other message keeps all tokens.
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;
use near_sdk::{AccountId, Gas, NearToken, PromiseOrValue, env, ext_contract, log, near, require};

const TRANSFER_GAS: Gas = Gas::from_tgas(15);
const CALLBACK_GAS: Gas = Gas::from_tgas(5);

#[ext_contract(ext_token)]
pub trait Token {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128);
    fn mt_transfer(&mut self, receiver_id: AccountId, token_id: String, amount: U128);
}

/// Will happily take any NEP-141 or NEP-245 token unless the message says otherwise.
#[near(contract_state)]
#[derive(Default)]
struct DummyFungibleTokenReceiver;

#[near]
impl FungibleTokenReceiver for DummyFungibleTokenReceiver {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        log!(
            "in {} tokens from @{} ft_on_transfer, msg = {}",
            amount.0,
            sender_id,
            msg
        );
        if let Some((receiver_id, spent)) = spend_args(&msg) {
            require!(spent.0 <= amount.0, "Cannot spend more than received");
            ext_token::ext(env::predecessor_account_id())
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .with_static_gas(TRANSFER_GAS)
                .ft_transfer(receiver_id, spent)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(CALLBACK_GAS)
                        .return_unused_ft(amount),
                )
                .into()
        } else {
            PromiseOrValue::Value(U128(unused_amount(&msg)))
        }
    }
}

#[near]
impl DummyFungibleTokenReceiver {
    pub fn mt_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_ids: Vec<AccountId>,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>> {
        log!(
            "in {amounts:?} of {token_ids:?} from @{sender_id} (owners {previous_owner_ids:?}) mt_on_transfer, msg = {msg}"
        );
        if let Some((receiver_id, spent)) = spend_args(&msg) {
            require!(
                token_ids.len() == 1 && amounts.len() == 1,
                "Expected one token"
            );
            require!(spent.0 <= amounts[0].0, "Cannot spend more than received");
            return ext_token::ext(env::predecessor_account_id())
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .with_static_gas(TRANSFER_GAS)
                .mt_transfer(receiver_id, token_ids[0].clone(), spent)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(CALLBACK_GAS)
                        .return_unused_mt(amounts),
                )
                .into();
        }
        let unused = unused_amount(&msg);
        PromiseOrValue::Value(amounts.iter().map(|_| U128(unused)).collect())
    }

    #[private]
    pub fn return_unused_ft(&self, amount: U128) -> U128 {
        require!(
            env::promise_result_checked(0, 16).is_ok(),
            "Forward transfer failed"
        );
        amount
    }

    #[private]
    pub fn return_unused_mt(&self, amounts: Vec<U128>) -> Vec<U128> {
        require!(
            env::promise_result_checked(0, 16).is_ok(),
            "Forward transfer failed"
        );
        amounts
    }
}

fn spend_args(msg: &str) -> Option<(AccountId, U128)> {
    msg.strip_prefix("spend:").map(|args| {
        let (account, amount) = args
            .split_once(':')
            .unwrap_or_else(|| env::panic_str("ft-receiver: invalid spend message"));
        let receiver_id = account
            .parse()
            .unwrap_or_else(|_| env::panic_str("ft-receiver: invalid spend account"));
        let amount = amount
            .parse()
            .unwrap_or_else(|_| env::panic_str("ft-receiver: invalid spend amount"));
        (receiver_id, U128(amount))
    })
}

fn unused_amount(msg: &str) -> u128 {
    if msg == "panic" {
        env::panic_str("ft-receiver: panic requested");
    }

    msg.strip_prefix("return:").map_or(0, |amount| {
        amount
            .parse()
            .unwrap_or_else(|_| env::panic_str("ft-receiver: invalid amount"))
    })
}
