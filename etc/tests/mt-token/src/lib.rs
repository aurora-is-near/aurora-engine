//! Minimal NEP-245 multi-token contract for the Aurora Engine integration tests.
//!
//! It implements the transfer, `transfer_call`, balance and metadata methods that the
//! engine uses. Anyone can mint tokens. Transfers to an account marked with
//! `set_rejected_receiver` fail, so tests can exercise failed withdrawals.
//! `set_resolver_failure` makes the resolver fail after the transfer has committed.
use near_sdk::json_types::U128;
use near_sdk::store::{LookupMap, LookupSet};
use near_sdk::{
    AccountId, Gas, PromiseOrValue, assert_one_yocto, env, ext_contract, near, require, serde_json,
};

type TokenId = String;

/// Gas reserved for `mt_resolve_transfer`.
const GAS_FOR_RESOLVE_TRANSFER: Gas = Gas::from_tgas(5);
/// Gas kept by `mt_transfer_call`; the rest is attached to `mt_on_transfer`.
const GAS_FOR_MT_TRANSFER_CALL: Gas = Gas::from_tgas(30);
/// Longest `mt_on_transfer` result read by `mt_resolve_transfer`.
const MAX_RESULT_LENGTH: usize = 1024;

/// Token metadata set on mint.
#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: Option<String>,
    pub decimals: Option<String>,
}

/// NEP-245 base token metadata returned by `mt_metadata_base_by_token_id`.
#[near(serializers = [json])]
pub struct MtBaseTokenMetadata {
    pub id: String,
    pub name: String,
    pub symbol: Option<String>,
    pub icon: Option<String>,
    pub decimals: Option<String>,
    pub base_uri: Option<String>,
    pub reference: Option<String>,
    pub copies: Option<u64>,
    pub reference_hash: Option<String>,
}

#[ext_contract(ext_mt_receiver)]
pub trait MultiTokenReceiver {
    fn mt_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_ids: Vec<AccountId>,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>>;
}

#[near(contract_state)]
pub struct MultiToken {
    balances: LookupMap<(TokenId, AccountId), u128>,
    metadata: LookupMap<TokenId, TokenMetadata>,
    rejected_receivers: LookupSet<AccountId>,
    resolver_failure: bool,
}

impl Default for MultiToken {
    fn default() -> Self {
        Self {
            balances: LookupMap::new(b"b"),
            metadata: LookupMap::new(b"m"),
            rejected_receivers: LookupSet::new(b"r"),
            resolver_failure: false,
        }
    }
}

#[near]
impl MultiToken {
    /// Mints `amount` of `token_id` to `account_id` and replaces the token metadata if given.
    pub fn mint(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
        metadata: Option<TokenMetadata>,
    ) {
        let balance = self
            .balance(&token_id, &account_id)
            .checked_add(amount.0)
            .unwrap_or_else(|| env::panic_str("Balance overflow"));
        self.balances
            .insert((token_id.clone(), account_id), balance);

        if let Some(metadata) = metadata {
            self.metadata.insert(token_id, metadata);
        }
    }

    /// Makes transfers to `account_id` fail while `rejected` is true.
    pub fn set_rejected_receiver(&mut self, account_id: AccountId, rejected: bool) {
        if rejected {
            self.rejected_receivers.insert(account_id);
        } else {
            self.rejected_receivers.remove(&account_id);
        }
    }

    pub fn set_resolver_failure(&mut self, enabled: bool) {
        self.resolver_failure = enabled;
    }

    pub fn mt_balance_of(&self, account_id: AccountId, token_id: TokenId) -> U128 {
        U128(self.balance(&token_id, &account_id))
    }

    pub fn mt_batch_balance_of(&self, account_id: AccountId, token_ids: Vec<TokenId>) -> Vec<U128> {
        token_ids
            .iter()
            .map(|token_id| U128(self.balance(token_id, &account_id)))
            .collect()
    }

    pub fn mt_metadata_base_by_token_id(
        &self,
        token_ids: Vec<TokenId>,
    ) -> Vec<Option<MtBaseTokenMetadata>> {
        token_ids
            .into_iter()
            .map(|token_id| {
                self.metadata
                    .get(&token_id)
                    .map(|metadata| MtBaseTokenMetadata {
                        id: token_id.clone(),
                        name: metadata.name.clone(),
                        symbol: metadata.symbol.clone(),
                        icon: None,
                        decimals: metadata.decimals.clone(),
                        base_uri: None,
                        reference: None,
                        copies: None,
                        reference_hash: None,
                    })
            })
            .collect()
    }

    #[payable]
    pub fn mt_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        amount: U128,
        approval: Option<(AccountId, u64)>,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        require!(approval.is_none(), "Approvals are not supported");
        env::log_str(&format!("mt_transfer memo: {memo:?}"));

        let sender_id = env::predecessor_account_id();
        self.transfer(&sender_id, &receiver_id, &token_id, amount.0);
    }

    #[payable]
    pub fn mt_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        amount: U128,
        approval: Option<(AccountId, u64)>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>> {
        self.mt_batch_transfer_call(
            receiver_id,
            vec![token_id],
            vec![amount],
            approval.map(|approval| vec![Some(approval)]),
            memo,
            msg,
        )
    }

    #[payable]
    pub fn mt_batch_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        approvals: Option<Vec<Option<(AccountId, u64)>>>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>> {
        assert_one_yocto();
        require!(approvals.is_none(), "Approvals are not supported");
        require!(
            !token_ids.is_empty() && token_ids.len() == amounts.len(),
            "Invalid token IDs or amounts"
        );
        env::log_str(&format!("mt_batch_transfer_call memo: {memo:?}"));

        let sender_id = env::predecessor_account_id();
        for (token_id, amount) in token_ids.iter().zip(&amounts) {
            self.transfer(&sender_id, &receiver_id, token_id, amount.0);
        }

        let receiver_gas = env::prepaid_gas()
            .checked_sub(GAS_FOR_MT_TRANSFER_CALL)
            .unwrap_or_else(|| env::panic_str("Not enough gas attached"));

        ext_mt_receiver::ext(receiver_id.clone())
            .with_static_gas(receiver_gas)
            .mt_on_transfer(
                sender_id.clone(),
                vec![sender_id.clone(); token_ids.len()],
                token_ids.clone(),
                amounts.clone(),
                msg,
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_TRANSFER)
                    .mt_resolve_transfer(sender_id, receiver_id, token_ids, amounts),
            )
            .into()
    }

    /// Returns unused tokens to the sender and the used amounts to the `mt_transfer_call` caller.
    #[private]
    pub fn mt_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
    ) -> Vec<U128> {
        require!(!self.resolver_failure, "Resolver failure requested");
        // A failed or malformed `mt_on_transfer` result refunds all tokens, as NEP-245 requires.
        let unused = env::promise_result_checked(0, MAX_RESULT_LENGTH)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<U128>>(&bytes).ok())
            .filter(|unused| unused.len() == amounts.len())
            .unwrap_or_else(|| amounts.clone());

        token_ids
            .iter()
            .zip(amounts)
            .zip(unused)
            .map(|((token_id, amount), unused)| {
                // The receiver may have moved the tokens already, so refund only what is left.
                let refund = unused
                    .0
                    .min(amount.0)
                    .min(self.balance(token_id, &receiver_id));
                if refund > 0 {
                    self.move_tokens(&receiver_id, &sender_id, token_id, refund);
                }
                U128(amount.0 - refund)
            })
            .collect()
    }
}

impl MultiToken {
    fn balance(&self, token_id: &str, account_id: &AccountId) -> u128 {
        self.balances
            .get(&(token_id.to_owned(), account_id.clone()))
            .copied()
            .unwrap_or_default()
    }

    fn transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        token_id: &str,
        amount: u128,
    ) {
        require!(sender_id != receiver_id, "Sender and receiver must differ");
        require!(amount > 0, "Amount must be positive");
        require!(
            !self.rejected_receivers.contains(receiver_id),
            "Receiver rejects the tokens"
        );
        self.move_tokens(sender_id, receiver_id, token_id, amount);
    }

    fn move_tokens(&mut self, from: &AccountId, to: &AccountId, token_id: &str, amount: u128) {
        let from_balance = self
            .balance(token_id, from)
            .checked_sub(amount)
            .unwrap_or_else(|| env::panic_str("Not enough balance"));
        let to_balance = self
            .balance(token_id, to)
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("Balance overflow"));
        self.balances
            .insert((token_id.to_owned(), from.clone()), from_balance);
        self.balances
            .insert((token_id.to_owned(), to.clone()), to_balance);
    }
}
