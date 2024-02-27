use aurora_engine_types::parameters::{
    NearPromise, PromiseAction, PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs,
    SimpleNearPromise,
};
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::U64;
use near_sdk::store::LookupMap;
use near_sdk::BorshStorageKey;
use near_sdk::{
    env, near_bindgen, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseIndex,
    PromiseResult,
};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests;

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Version,
    Parent,
    Nonce,
    Map,
}

const INITIALIZE: &str = "initialize";
const CURRENT_VERSION: u32 = include!("VERSION");

const ERR_ILLEGAL_CALLER: &str = "ERR_ILLEGAL_CALLER";
const INITIALIZE_GAS: Gas = Gas::from_tgas(15);
/// Gas cost estimated from mainnet data. Example:
/// https://explorer.mainnet.near.org/transactions/5NbZ7SfrodNxeLcSkCmLAEdbZfbkk9cjqz3zSDwktKrk#D7un3c3Nxv7Ee3JpQSKiM97LbwCDFPbMo5iLoijGPXPM
const WNEAR_REGISTER_GAS: Gas = Gas::from_tgas(5);
/// Registration amount computed from FT token source code, see
/// https://github.com/near/near-sdk-rs/blob/master/near-contract-standards/src/fungible_token/core_impl.rs#L50
/// https://github.com/near/near-sdk-rs/blob/master/near-contract-standards/src/fungible_token/storage_impl.rs#L101
const WNEAR_REGISTER_AMOUNT: NearToken = NearToken::from_yoctonear(1_250_000_000_000_000_000_000);
/// Must match aurora_engine_precompiles::xcc::state::STORAGE_AMOUNT
const REFUND_AMOUNT: NearToken = NearToken::from_near(2);

#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct DeployUpgradeParams {
    pub code: Vec<u8>,
    pub initialize_args: Vec<u8>,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
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
    #[must_use]
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
                // Allow self-calls to `initialize` also.
                // This happens during the upgrade flow.
                if (caller != parent) && (caller != env::current_account_id()) {
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

    pub fn get_version(&self) -> u32 {
        self.version.get().unwrap_or_default()
    }

    /// This function can only be called by the parent account (i.e. Aurora engine) to ensure that
    /// no one can create calls on behalf of the user this router contract is deployed for.
    /// The engine only calls this function when the special precompile in the EVM for NEAR cross
    /// contract calls is used by the address associated with the sub-account this router contract
    /// is deployed at.
    pub fn execute(&self, #[serializer(borsh)] promise: PromiseArgs) {
        self.assert_preconditions();

        let promise_id = Self::promise_create(promise);
        env::promise_return(promise_id);
    }

    /// Similar security considerations here as for `execute`.
    pub fn schedule(&mut self, #[serializer(borsh)] promise: PromiseArgs) {
        self.assert_preconditions();

        let nonce = self.nonce.get().unwrap_or_default();
        self.scheduled_promises.insert(nonce, promise);
        self.nonce.set(&(nonce + 1));

        near_sdk::log!("Promise scheduled at nonce {}", nonce);
    }

    /// It is intentional that this function can be called by anyone (not just the parent).
    /// There is no security risk to allowing this function to be open because it can only
    /// act on promises that were created via `schedule`.
    #[payable]
    pub fn execute_scheduled(&mut self, nonce: U64) {
        let Some(promise) = self.scheduled_promises.remove(&nonce.0) else {
            env::panic_str("ERR_PROMISE_NOT_FOUND")
        };
        let promise_id = Self::promise_create(promise);
        env::promise_return(promise_id);
    }

    /// Allows the parent contract to trigger an update to the logic of this contract
    /// (by deploying a new contract to this account);
    #[payable]
    pub fn deploy_upgrade(&mut self, #[serializer(borsh)] args: DeployUpgradeParams) {
        self.assert_preconditions();

        let promise_id = env::promise_batch_create(&env::current_account_id());
        env::promise_batch_action_deploy_contract(promise_id, &args.code);
        env::promise_batch_action_function_call(
            promise_id,
            INITIALIZE,
            &args.initialize_args,
            NearToken::default(),
            INITIALIZE_GAS,
        );
        env::promise_return(promise_id);
    }

    pub fn send_refund(&self) -> Promise {
        let parent = self.get_parent().unwrap_or_else(env_panic);

        require_caller(&parent)
            .and_then(|_| require_no_failed_promises())
            .unwrap_or_else(env_panic);

        Promise::new(parent).transfer(REFUND_AMOUNT)
    }
}

impl Router {
    fn get_parent(&self) -> Result<AccountId, Error> {
        self.parent.get().ok_or(Error::ContractNotInitialized)
    }

    /// Checks the following preconditions:
    ///   1. Contract is initialized
    ///   2. predecessor_account_id == self.parent
    ///   3. There are no failed promise results
    /// These preconditions must be checked on methods where are important for
    /// the security of the contract (e.g. `execute`).
    fn require_preconditions(&self) -> Result<(), Error> {
        let parent = self.get_parent()?;
        require_caller(&parent)?;
        require_no_failed_promises()?;
        Ok(())
    }

    /// Panics if any of the preconditions checked in `require_preconditions` are not met.
    fn assert_preconditions(&self) {
        self.require_preconditions().unwrap_or_else(env_panic);
    }

    fn promise_create(promise: PromiseArgs) -> PromiseIndex {
        match promise {
            PromiseArgs::Create(call) => Self::base_promise_create(&call),
            PromiseArgs::Callback(cb) => Self::cb_promise_create(&cb),
            PromiseArgs::Recursive(p) => Self::recursive_promise_create(&p),
        }
    }

    fn cb_promise_create(promise: &PromiseWithCallbackArgs) -> PromiseIndex {
        let base = Self::base_promise_create(&promise.base);
        let promise = &promise.callback;

        env::promise_then(
            base,
            promise.target_account_id.as_ref().parse().unwrap(),
            promise.method.as_str(),
            &promise.args,
            NearToken::from_yoctonear(promise.attached_balance.as_u128()),
            Gas::from_gas(promise.attached_gas.as_u64()),
        )
    }

    fn base_promise_create(promise: &PromiseCreateArgs) -> PromiseIndex {
        env::promise_create(
            promise.target_account_id.as_ref().parse().unwrap(),
            promise.method.as_str(),
            &promise.args,
            NearToken::from_yoctonear(promise.attached_balance.as_u128()),
            Gas::from_gas(promise.attached_gas.as_u64()),
        )
    }

    fn recursive_promise_create(promise: &NearPromise) -> PromiseIndex {
        match promise {
            NearPromise::Simple(x) => match x {
                SimpleNearPromise::Create(call) => Self::base_promise_create(call),
                SimpleNearPromise::Batch(batch) => {
                    let target = batch.target_account_id.as_ref().parse().unwrap();
                    let id = env::promise_batch_create(&target);
                    Self::add_batch_actions(id, &batch.actions);
                    id
                }
            },
            NearPromise::Then { base, callback } => {
                let base_index = Self::recursive_promise_create(base);
                match callback {
                    SimpleNearPromise::Create(call) => env::promise_then(
                        base_index,
                        call.target_account_id.as_ref().parse().unwrap(),
                        call.method.as_str(),
                        &call.args,
                        NearToken::from_yoctonear(call.attached_balance.as_u128()),
                        Gas::from_gas(call.attached_gas.as_u64()),
                    ),
                    SimpleNearPromise::Batch(batch) => {
                        let id = env::promise_batch_then(
                            base_index,
                            &batch.target_account_id.as_ref().parse().unwrap(),
                        );
                        Self::add_batch_actions(id, &batch.actions);
                        id
                    }
                }
            }
            NearPromise::And(promises) => {
                let indices: Vec<PromiseIndex> = promises
                    .iter()
                    .map(Self::recursive_promise_create)
                    .collect();
                env::promise_and(&indices)
            }
        }
    }

    #[cfg(not(feature = "all-promise-actions"))]
    fn add_batch_actions(_id: PromiseIndex, _actions: &[PromiseAction]) {
        unimplemented!("NEAR batch transactions are not supported. Please file an issue at https://github.com/aurora-is-near/aurora-engine")
    }

    #[cfg(feature = "all-promise-actions")]
    fn add_batch_actions(id: PromiseIndex, actions: &[PromiseAction]) {
        for action in actions.iter() {
            match action {
                PromiseAction::CreateAccount => env::promise_batch_action_create_account(id),
                PromiseAction::Transfer { amount } => env::promise_batch_action_transfer(
                    id,
                    NearToken::from_yoctonear(amount.as_u128()),
                ),
                PromiseAction::DeployContract { code } => {
                    env::promise_batch_action_deploy_contract(id, code)
                }
                PromiseAction::FunctionCall {
                    name,
                    args,
                    attached_yocto,
                    gas,
                } => env::promise_batch_action_function_call(
                    id,
                    name,
                    args,
                    NearToken::from_yoctonear(attached_yocto.as_u128()),
                    Gas::from_gas(gas.as_u64()),
                ),
                PromiseAction::Stake { amount, public_key } => env::promise_batch_action_stake(
                    id,
                    NearToken::from_yoctonear(amount.as_u128()),
                    &to_sdk_pk(public_key),
                ),
                PromiseAction::AddFullAccessKey { public_key, nonce } => {
                    env::promise_batch_action_add_key_with_full_access(
                        id,
                        &to_sdk_pk(public_key),
                        *nonce,
                    )
                }
                PromiseAction::AddFunctionCallKey {
                    public_key,
                    nonce,
                    allowance,
                    receiver_id,
                    function_names,
                } => {
                    let receiver_id = receiver_id.as_ref().parse().unwrap();
                    env::promise_batch_action_add_key_allowance_with_function_call(
                        id,
                        &to_sdk_pk(public_key),
                        *nonce,
                        near_sdk::Allowance::limited(NearToken::from_yoctonear(
                            allowance.as_u128(),
                        ))
                        .unwrap(),
                        &receiver_id,
                        function_names,
                    )
                }
                PromiseAction::DeleteKey { public_key } => {
                    env::promise_batch_action_delete_key(id, &to_sdk_pk(public_key))
                }
                PromiseAction::DeleteAccount { beneficiary_id } => {
                    let beneficiary_id = beneficiary_id.as_ref().parse().unwrap();
                    env::promise_batch_action_delete_account(id, &beneficiary_id)
                }
            }
        }
    }
}

#[cfg(feature = "all-promise-actions")]
fn to_sdk_pk(key: &aurora_engine_types::public_key::PublicKey) -> near_sdk::PublicKey {
    let (curve_type, key_bytes): (near_sdk::CurveType, &[u8]) = match key {
        aurora_engine_types::public_key::PublicKey::Ed25519(bytes) => {
            (near_sdk::CurveType::ED25519, bytes)
        }
        aurora_engine_types::public_key::PublicKey::Secp256k1(bytes) => {
            (near_sdk::CurveType::SECP256K1, bytes)
        }
    };
    let mut data = Vec::with_capacity(1 + key_bytes.len());
    data.push(curve_type as u8);
    data.extend_from_slice(key_bytes);

    // Unwrap should be safe because we only encode valid public keys
    data.try_into().unwrap()
}

fn require_caller(caller: &AccountId) -> Result<(), Error> {
    if caller != &env::predecessor_account_id() {
        return Err(Error::IllegalCaller);
    }
    Ok(())
}

fn require_no_failed_promises() -> Result<(), Error> {
    let num_promises = env::promise_results_count();
    for index in 0..num_promises {
        if env::promise_result(index) == PromiseResult::Failed {
            return Err(Error::CallbackOfFailedPromise);
        }
    }
    Ok(())
}

fn env_panic<T>(e: Error) -> T {
    env::panic_str(e.as_ref())
}

#[derive(Debug)]
enum Error {
    ContractNotInitialized,
    IllegalCaller,
    CallbackOfFailedPromise,
}

impl AsRef<str> for Error {
    fn as_ref(&self) -> &str {
        match self {
            Self::ContractNotInitialized => "ERR_CONTRACT_NOT_INITIALIZED",
            Self::IllegalCaller => ERR_ILLEGAL_CALLER,
            Self::CallbackOfFailedPromise => "ERR_CALLBACK_OF_FAILED_PROMISE",
        }
    }
}
