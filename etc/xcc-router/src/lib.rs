use aurora_engine_types::parameters::{PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap};
use near_sdk::json_types::{U128, U64};
use near_sdk::BorshStorageKey;
use near_sdk::{env, near_bindgen, AccountId, Gas, PanicOnDefault, Promise, PromiseIndex};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests;

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Version,
    Parent,
    Nonce,
    Map,
}

const CURRENT_VERSION: u32 = 1;

const ERR_ILLEGAL_CALLER: &str = "ERR_ILLEGAL_CALLER";
/// Gas cost estimated from mainnet data. Cost seems to consistently be 3 Tgas, but we add a
/// little more to be safe. Example:
/// https://explorer.mainnet.near.org/transactions/3U9SKbGKM3MchLa2hLTNuYLdErcEDneJGbGv1cHZXuvE#HsHabUdJ7DRJcseNa4GQTYwm8KtbB4mqsq2AUssJWWv6
const WNEAR_WITHDRAW_GAS: Gas = Gas(5_000_000_000_000);
/// Gas cost estimated from mainnet data. Example:
/// https://explorer.mainnet.near.org/transactions/5NbZ7SfrodNxeLcSkCmLAEdbZfbkk9cjqz3zSDwktKrk#D7un3c3Nxv7Ee3JpQSKiM97LbwCDFPbMo5iLoijGPXPM
const WNEAR_REGISTER_GAS: Gas = Gas(5_000_000_000_000);
/// Gas cost estimated from simulation tests.
const REFUND_GAS: Gas = Gas(5_000_000_000_000);
/// Registration amount computed from FT token source code, see
/// https://github.com/near/near-sdk-rs/blob/master/near-contract-standards/src/fungible_token/core_impl.rs#L50
/// https://github.com/near/near-sdk-rs/blob/master/near-contract-standards/src/fungible_token/storage_impl.rs#L101
const WNEAR_REGISTER_AMOUNT: u128 = 1_250_000_000_000_000_000_000;
/// Must match arora_engine_precompiles::xcc::state::STORAGE_AMOUNT
const REFUND_AMOUNT: u128 = 2_000_000_000_000_000_000_000_000;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Router {
    /// The account id of the Aurora Engine instance that controls this router.
    parent: LazyOption<AccountId>,
    /// The version of the router contract that was last deployed
    version: LazyOption<u32>,
    /// A sequential id to keep track of how many scheduled promises this router has executed.
    /// This allows multiple promises to be scheduled before any of them are executed.
    nonce: LazyOption<u64>,
    /// The storage for the scheduled promises.
    scheduled_promises: LookupMap<u64, PromiseArgs>,
    /// Account ID for the wNEAR contract.
    wnear_account: AccountId,
}

#[near_bindgen]
impl Router {
    #[init(ignore_state)]
    pub fn initialize(wnear_account: AccountId, must_register: bool) -> Self {
        // The first time this function is called there is no state and the parent is set to be
        // the predecessor account id. In subsequent calls, only the original parent is allowed to
        // call this function. The idea is that the Create, Deploy and Initialize actions are done in a single
        // NEAR batch when a new router is deployed by the engine, so the caller will be the Aurora
        // engine instance that the user's address belongs to. If we update this contract and deploy
        // a new version of it, again the Deploy and Initialize actions will be done in a single batch
        // by the engine.
        let caller = env::predecessor_account_id();
        let mut parent = LazyOption::new(StorageKey::Parent, None);
        match parent.get() {
            None => {
                parent.set(&caller);
            }
            Some(parent) => {
                if caller != parent {
                    env::panic_str(ERR_ILLEGAL_CALLER);
                }
            }
        }

        if must_register {
            env::promise_create(
                wnear_account.clone(),
                "storage_deposit",
                b"{}",
                WNEAR_REGISTER_AMOUNT,
                WNEAR_REGISTER_GAS,
            );
        }

        let mut version = LazyOption::new(StorageKey::Version, None);
        if version.get().unwrap_or_default() != CURRENT_VERSION {
            // Future migrations would go here

            version.set(&CURRENT_VERSION);
        }

        let nonce = LazyOption::new(StorageKey::Nonce, None);
        let scheduled_promises = LookupMap::new(StorageKey::Map);
        Self {
            parent,
            version,
            nonce,
            scheduled_promises,
            wnear_account,
        }
    }

    /// This function can only be called by the parent account (i.e. Aurora engine) to ensure that
    /// no one can create calls on behalf of the user this router contract is deployed for.
    /// The engine only calls this function when the special precompile in the EVM for NEAR cross
    /// contract calls is used by the address associated with the sub-account this router contract
    /// is deployed at.
    pub fn execute(&self, #[serializer(borsh)] promise: PromiseArgs) {
        self.require_parent_caller();

        let promise_id = Router::promise_create(promise);
        env::promise_return(promise_id)
    }

    /// Similar security considerations here as for `execute`.
    pub fn schedule(&mut self, #[serializer(borsh)] promise: PromiseArgs) {
        self.require_parent_caller();

        let nonce = self.nonce.get().unwrap_or_default();
        self.scheduled_promises.insert(&nonce, &promise);
        self.nonce.set(&(nonce + 1));

        near_sdk::log!("Promise scheduled at nonce {}", nonce);
    }

    /// It is intentional that this function can be called by anyone (not just the parent).
    /// There is no security risk to allowing this function to be open because it can only
    /// act on promises that were created via `schedule`.
    #[payable]
    pub fn execute_scheduled(&mut self, nonce: U64) {
        let promise = match self.scheduled_promises.remove(&nonce.0) {
            Some(promise) => promise,
            None => env::panic_str("ERR_PROMISE_NOT_FOUND"),
        };

        let promise_id = Router::promise_create(promise);
        env::promise_return(promise_id)
    }

    /// The router will receive wNEAR deposits from its user. This function is to
    /// unwrap that wNEAR into NEAR. Additionally, this function will transfer some
    /// NEAR back to its parent, if needed. This transfer is done because the parent
    /// must cover the storage staking cost with the router account is first created,
    /// but the user ultimately is responsible to pay for it.
    pub fn unwrap_and_refund_storage(&self, amount: U128, refund_needed: bool) {
        self.require_parent_caller();

        let args = format!(r#"{{"amount": "{}"}}"#, amount.0);
        let id = env::promise_create(
            self.wnear_account.clone(),
            "near_withdraw",
            args.as_bytes(),
            1,
            WNEAR_WITHDRAW_GAS,
        );
        let final_id = if refund_needed {
            env::promise_then(
                id,
                env::current_account_id(),
                "send_refund",
                &[],
                0,
                REFUND_GAS,
            )
        } else {
            id
        };
        env::promise_return(final_id);
    }

    #[private]
    pub fn send_refund(&self) -> Promise {
        let parent = self
            .parent
            .get()
            .unwrap_or_else(|| env::panic_str("ERR_CONTRACT_NOT_INITIALIZED"));

        Promise::new(parent).transfer(REFUND_AMOUNT)
    }
}

impl Router {
    fn require_parent_caller(&self) {
        let caller = env::predecessor_account_id();
        let parent = self
            .parent
            .get()
            .unwrap_or_else(|| env::panic_str("ERR_CONTRACT_NOT_INITIALIZED"));
        if caller != parent {
            env::panic_str(ERR_ILLEGAL_CALLER)
        }
    }

    fn promise_create(promise: PromiseArgs) -> PromiseIndex {
        match promise {
            PromiseArgs::Create(call) => Self::base_promise_create(call),
            PromiseArgs::Callback(cb) => Self::cb_promise_create(cb),
        }
    }

    fn cb_promise_create(promise: PromiseWithCallbackArgs) -> PromiseIndex {
        let base = Self::base_promise_create(promise.base);
        let promise = promise.callback;

        env::promise_then(
            base,
            near_sdk::AccountId::new_unchecked(promise.target_account_id.to_string()),
            promise.method.as_str(),
            &promise.args,
            promise.attached_balance.as_u128(),
            promise.attached_gas.as_u64().into(),
        )
    }

    fn base_promise_create(promise: PromiseCreateArgs) -> PromiseIndex {
        env::promise_create(
            near_sdk::AccountId::new_unchecked(promise.target_account_id.to_string()),
            promise.method.as_str(),
            &promise.args,
            promise.attached_balance.as_u128(),
            promise.attached_gas.as_u64().into(),
        )
    }
}
