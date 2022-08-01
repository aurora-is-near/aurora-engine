use aurora_engine_types::parameters::{PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap};
use near_sdk::json_types::U64;
use near_sdk::{env, near_bindgen, AccountId, PanicOnDefault, PromiseIndex};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests;

const VERSION_PREFIX: &[u8] = &[0x00];
const PARENT_PREFIX: &[u8] = &[0x01];
const NONCE_PREFIX: &[u8] = &[0x02];
const MAP_PREFIX: &[u8] = &[0x03];

const CURRENT_VERSION: u32 = 0;

const ERR_ILLEGAL_CALLER: &[u8] = b"ERR_ILLEGAL_CALLER";

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
struct Router {
    /// The account id of the Aurora Engine instance that controls this router.
    parent: LazyOption<AccountId>,
    /// The version of the router contract that was last deployed
    version: LazyOption<u32>,
    /// A sequential id to keep track of how many scheduled promises this router has executed.
    /// This allows multiple promises to be scheduled before any of them are executed.
    nonce: LazyOption<u64>,
    /// The storage for the scheduled promises.
    scheduled_promises: LookupMap<u64, PromiseArgs>,
}

#[near_bindgen]
impl Router {
    #[init(ignore_state)]
    pub fn initialize() -> Self {
        // The first time this function is called there is no state and the parent is set to be
        // the predecessor account id. In subsequent calls, only the original parent is allowed to
        // call this function. The idea is that the Create, Deploy and Initialize actions are done in a single
        // NEAR batch when a new router is deployed by the engine, so the caller will be the Aurora
        // engine instance that the user's address belongs to. If we update this contract and deploy
        // a new version of it, again the Deploy and Initialize actions will be done in a single batch
        // by the engine.
        let caller = env::predecessor_account_id();
        let mut parent = LazyOption::new(PARENT_PREFIX, None);
        match parent.get() {
            None => {
                parent.set(&caller);
            }
            Some(parent) => {
                if caller != parent {
                    env::panic(ERR_ILLEGAL_CALLER);
                }
            }
        }

        let mut version = LazyOption::new(VERSION_PREFIX, None);
        if version.get().unwrap_or_default() != CURRENT_VERSION {
            // Future migrations would go here

            version.set(&CURRENT_VERSION);
        }

        let nonce = LazyOption::new(NONCE_PREFIX, None);
        let scheduled_promises = LookupMap::new(MAP_PREFIX);
        Self {
            parent,
            version,
            nonce,
            scheduled_promises,
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

        env::log(format!("Promise scheduled at nonce {}", nonce).as_bytes());
    }

    /// It is intentional that this function can be called by anyone (not just the parent).
    /// There is no security risk to allowing this function to be open because it can only
    /// act on promises that were created via `schedule`.
    #[payable]
    pub fn execute_scheduled(&mut self, nonce: U64) {
        let promise = match self.scheduled_promises.remove(&nonce.0) {
            Some(promise) => promise,
            None => env::panic(b"ERR_PROMISE_NOT_FOUND"),
        };

        let promise_id = Router::promise_create(promise);
        env::promise_return(promise_id)
    }
}

impl Router {
    fn require_parent_caller(&self) {
        let caller = env::predecessor_account_id();
        let parent = self
            .parent
            .get()
            .unwrap_or_else(|| env::panic(b"ERR_CONTRACT_NOT_INITIALIZED"));
        if caller != parent {
            env::panic(ERR_ILLEGAL_CALLER)
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
            promise.target_account_id.to_string(),
            promise.method.as_bytes(),
            &promise.args,
            promise.attached_balance.as_u128(),
            promise.attached_gas.as_u64(),
        )
    }

    fn base_promise_create(promise: PromiseCreateArgs) -> PromiseIndex {
        env::promise_create(
            promise.target_account_id.to_string(),
            promise.method.as_bytes(),
            &promise.args,
            promise.attached_balance.as_u128(),
            promise.attached_gas.as_u64(),
        )
    }
}
