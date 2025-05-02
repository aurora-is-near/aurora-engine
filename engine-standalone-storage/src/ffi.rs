use std::{
    borrow::Cow,
    env, iter,
    path::Path,
    slice,
    sync::{LazyLock, Mutex},
};

use sha2::digest::{FixedOutput, Update};

use rocksdb::DB;

use aurora_engine_sdk::env::Fixed;

pub static STATE: LazyLock<State> = LazyLock::new(|| {
    let path = env::var("storage_path").unwrap_or_else(|_| "target/storage".to_owned());
    State::open(path).expect("bad DB")
});

pub struct State {
    inner: Mutex<StateInner>,
    #[allow(dead_code)]
    db: DB,
}

#[derive(Default)]
struct Register(Option<Vec<u8>>);

struct StateInner {
    registers: Vec<Register>,
    env: Option<Fixed>,
    input: Vec<u8>,
    output: Vec<u8>,
    promise_data: Box<[Option<Vec<u8>>]>,
}

const REGISTERS_NUMBER: usize = 6;

impl Default for StateInner {
    fn default() -> Self {
        StateInner {
            registers: iter::repeat_with(|| Register::default())
                .take(REGISTERS_NUMBER)
                .collect(),
            env: None,
            input: vec![],
            output: vec![],
            promise_data: Box::new([]),
        }
    }
}

impl State {
    pub fn open<P>(path: P) -> Result<Self, rocksdb::Error>
    where
        P: AsRef<Path>,
    {
        Ok(State {
            inner: Mutex::new(StateInner::default()),
            db: DB::open_default(path)?,
        })
    }

    #[allow(dead_code)]
    pub fn set_env(&self, env: Fixed) {
        self.inner.lock().expect("poisoned").env = Some(env);
    }

    #[allow(dead_code)]
    pub fn set_promise_handler(&self, promise_data: Box<[Option<Vec<u8>>]>) {
        self.inner.lock().expect("poisoned").promise_data = promise_data;
    }

    #[allow(dead_code)]
    pub fn set_input(&self, input: Vec<u8>) {
        self.inner.lock().expect("poisoned").input = input;
    }

    #[allow(dead_code)]
    pub fn take_output(&self) -> Vec<u8> {
        self.inner.lock().expect("poisoned").output.clone()
    }

    fn read_reg<F, T>(&self, register_id: u64, mut op: F) -> T
    where
        F: FnMut(&Register) -> T,
    {
        let index = register_id as usize;
        let lock = self.inner.lock().expect("poisoned");
        let reg = lock
            .registers
            .get(index)
            .unwrap_or_else(|| panic!("no such register {register_id}"));
        op(reg)
    }

    fn set_reg<'a>(&self, register_id: u64, data: Cow<'a, [u8]>) {
        let index = register_id as usize;
        let mut lock = self.inner.lock().expect("poisoned");
        *lock
            .registers
            .get_mut(index)
            .unwrap_or_else(|| panic!("no such register {register_id}")) =
            Register(Some(data.into_owned()));
    }

    /// The lifetime is static because it comes from the caller.
    /// This function is supposed to be external, so the caller has the highest possible lifetime.
    fn get_data(&self, ptr: u64, len: u64) -> Cow<'static, [u8]> {
        if len == u64::MAX {
            self.read_reg(ptr, |reg| {
                let data = reg.0.as_ref().expect("register must exist").clone();
                Cow::Owned(data)
            })
        } else {
            Cow::Borrowed(unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) })
        }
    }

    //// Near API

    fn read_register(&self, register_id: u64, ptr: u64) {
        self.read_reg(register_id, |reg| {
            if let Some(reg) = &reg.0 {
                unsafe { (ptr as *mut u8).copy_from_nonoverlapping(reg.as_ptr(), reg.len()) };
            }
        });
    }

    fn register_len(&self, register_id: u64) -> u64 {
        self.read_reg(register_id, |reg| {
            reg.0.as_ref().map_or(u64::MAX, |reg| reg.len() as u64)
        })
    }

    fn current_account_id(&self, register_id: u64) {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.current_account_id.as_bytes().into());
    }

    fn signer_account_id(&self, register_id: u64) {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.signer_account_id.as_bytes().into());
    }

    fn predecessor_account_id(&self, register_id: u64) {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.predecessor_account_id.as_bytes().into());
    }

    fn input(&self, register_id: u64) {
        let input = &self.inner.lock().expect("poisoned").input;
        self.set_reg(register_id, input.into());
    }

    fn block_index(&self) -> u64 {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        env.block_height
    }

    fn block_timestamp(&self) -> u64 {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        env.block_timestamp.nanos()
    }

    fn attached_deposit(&self, balance_ptr: u64) {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        unsafe { (balance_ptr as *mut u128).write(env.attached_deposit) }
    }

    fn prepaid_gas(&self) -> u64 {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        env.prepaid_gas.as_u64()
    }

    fn used_gas(&self) -> u64 {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        env.used_gas.as_u64()
    }

    fn random_seed(&self, register_id: u64) {
        let Some(env) = &self.inner.lock().expect("poisoned").env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.random_seed.as_bytes().into());
    }

    fn digest<D: Default + Update + FixedOutput>(
        &self,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) {
        let data = self.get_data(value_ptr, value_len);
        let hash = D::default().chain(data).finalize_fixed();
        self.set_reg(register_id, hash.as_slice().into());
    }

    fn ecrecover(
        &self,
        hash_len: u64,
        hash_ptr: u64,
        sig_len: u64,
        sig_ptr: u64,
        v: u64,
        register_id: u64,
    ) -> Result<(), ()> {
        let hash = self.get_data(hash_ptr, hash_len);
        let hash = libsecp256k1::Message::parse_slice(&hash).map_err(|_| ())?;
        let sig = self.get_data(sig_ptr, sig_len);
        let sig = libsecp256k1::Signature::parse_standard_slice(&sig).map_err(|_| ())?;
        let bit = match v {
            0..=26 => v as u8,
            _ => (v - 27) as u8,
        };
        let recovery_id = libsecp256k1::RecoveryId::parse(bit).map_err(|_| ())?;

        let public_key = libsecp256k1::recover(&hash, &sig, &recovery_id).map_err(|_| ())?;
        self.set_reg(register_id, public_key.serialize()[1..].into());

        Ok(())
    }

    fn value_return(&self, value_len: u64, value_ptr: u64) {
        let data = self.get_data(value_ptr, value_len);
        self.inner.lock().expect("poisoned").output = data.into_owned();
    }

    fn promise_results_count(&self) -> u64 {
        u64::try_from(self.inner.lock().expect("poisoned").promise_data.len()).unwrap_or_default()
    }

    fn promise_result(&self, result_idx: u64, register_id: u64) -> u64 {
        let i = usize::try_from(result_idx).expect("index too big");
        let lock = self.inner.lock().expect("poisoned");
        let Some(data) = lock.promise_data.get(i) else {
            return 3;
        };
        let Some(data) = data else {
            return 2;
        };
        self.set_reg(register_id, data.as_slice().into());
        1
    }

    fn storage_write(
        &self,
        key_len: u64,
        key_ptr: u64,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) -> u64 {
        let key = self.get_data(key_ptr, key_len);
        let value = self.get_data(value_ptr, value_len);
        if value_ptr != register_id {
            self.set_reg(register_id, value.as_ref().into());
        }
        self.db.put(key, value).is_ok().into()
    }

    fn storage_read(&self, key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
        let key = self.get_data(key_ptr, key_len);
        let Ok(value) = self.db.get(key) else {
            return 0;
        };
        if let Some(data) = value {
            self.set_reg(register_id, Cow::Owned(data));
        }
        1
    }

    fn storage_remove(&self, key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
        let key = self.get_data(key_ptr, key_len);
        if let Ok(Some(value)) = self.db.get(&key) {
            self.set_reg(register_id, value.into());
        }
        self.db.delete(key).is_ok().into()
    }

    fn storage_has_key(&self, key_len: u64, key_ptr: u64) -> u64 {
        let key = self.get_data(key_ptr, key_len);
        self.db.get(key).map_or(false, |x| x.is_some()).into()
    }
}

// #############
// # Registers #
// #############

#[unsafe(no_mangle)]
extern "C" fn read_register(register_id: u64, ptr: u64) {
    STATE.read_register(register_id, ptr)
}

#[unsafe(no_mangle)]
extern "C" fn register_len(register_id: u64) -> u64 {
    STATE.register_len(register_id)
}

// ###############
// # Context API #
// ###############

#[unsafe(no_mangle)]
extern "C" fn current_account_id(register_id: u64) {
    STATE.current_account_id(register_id)
}

#[unsafe(no_mangle)]
extern "C" fn signer_account_id(register_id: u64) {
    STATE.signer_account_id(register_id)
}

#[unsafe(no_mangle)]
extern "C" fn signer_account_pk(register_id: u64) {
    let _ = register_id;
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn predecessor_account_id(register_id: u64) {
    STATE.predecessor_account_id(register_id)
}

#[unsafe(no_mangle)]
extern "C" fn input(register_id: u64) {
    STATE.input(register_id)
}

#[unsafe(no_mangle)]
extern "C" fn block_index() -> u64 {
    STATE.block_index()
}

#[unsafe(no_mangle)]
extern "C" fn block_timestamp() -> u64 {
    STATE.block_timestamp()
}

#[unsafe(no_mangle)]
extern "C" fn epoch_height() -> u64 {
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn storage_usage() -> u64 {
    unimplemented!()
}

// #################
// # Economics API #
// #################

#[unsafe(no_mangle)]
extern "C" fn account_balance(balance_ptr: u64) {
    let _ = balance_ptr;
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn attached_deposit(balance_ptr: u64) {
    STATE.attached_deposit(balance_ptr)
}

#[unsafe(no_mangle)]
extern "C" fn prepaid_gas() -> u64 {
    STATE.prepaid_gas()
}

#[unsafe(no_mangle)]
extern "C" fn used_gas() -> u64 {
    STATE.used_gas()
}

// ############
// # Math API #
// ############

#[unsafe(no_mangle)]
extern "C" fn random_seed(register_id: u64) {
    STATE.random_seed(register_id)
}

#[unsafe(no_mangle)]
extern "C" fn sha256(value_len: u64, value_ptr: u64, register_id: u64) {
    STATE.digest::<sha2::Sha256>(value_len, value_ptr, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn keccak256(value_len: u64, value_ptr: u64, register_id: u64) {
    STATE.digest::<sha3::Keccak256>(value_len, value_ptr, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn ripemd160(value_len: u64, value_ptr: u64, register_id: u64) {
    STATE.digest::<ripemd::Ripemd160>(value_len, value_ptr, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn ecrecover(
    hash_len: u64,
    hash_ptr: u64,
    sig_len: u64,
    sig_ptr: u64,
    v: u64,
    malleability_flag: u64,
    register_id: u64,
) -> u64 {
    if malleability_flag == 0 {
        STATE
            .ecrecover(hash_len, hash_ptr, sig_len, sig_ptr, v, register_id)
            .is_ok()
            .into()
    } else {
        unimplemented!()
    }
}

#[unsafe(no_mangle)]
extern "C" fn alt_bn128_g1_sum(value_len: u64, value_ptr: u64, register_id: u64) {
    let _ = (value_len, value_ptr, register_id);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn alt_bn128_g1_multiexp(value_len: u64, value_ptr: u64, register_id: u64) {
    let _ = (value_len, value_ptr, register_id);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn alt_bn128_pairing_check(value_len: u64, value_ptr: u64) {
    let _ = (value_len, value_ptr);
    unimplemented!()
}

// #####################
// # Miscellaneous API #
// #####################

#[unsafe(no_mangle)]
extern "C" fn value_return(value_len: u64, value_ptr: u64) {
    STATE.value_return(value_len, value_ptr)
}

#[unsafe(no_mangle)]
extern "C" fn panic() {
    panic!()
}

#[unsafe(no_mangle)]
extern "C" fn panic_utf8(len: u64, ptr: u64) {
    let str = unsafe {
        std::str::from_utf8_unchecked(slice::from_raw_parts(ptr as *const u8, len as usize))
    };
    panic!("{str}");
}

#[unsafe(no_mangle)]
extern "C" fn log_utf8(len: u64, ptr: u64) {
    let str = unsafe {
        std::str::from_utf8_unchecked(slice::from_raw_parts(ptr as *const u8, len as usize))
    };
    println!("{str}");
}

#[unsafe(no_mangle)]
extern "C" fn log_utf16(len: u64, ptr: u64) {
    let _ = (len, ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn abort(msg_ptr: u32, filename_ptr: u32, line: u32, col: u32) {
    let _ = (msg_ptr, filename_ptr, line, col);
    unimplemented!()
}

// ################
// # Promises API #
// ################

#[unsafe(no_mangle)]
extern "C" fn promise_create(
    account_id_len: u64,
    account_id_ptr: u64,
    method_name_len: u64,
    method_name_ptr: u64,
    arguments_len: u64,
    arguments_ptr: u64,
    amount_ptr: u64,
    gas: u64,
) -> u64 {
    let _ = (account_id_len, account_id_ptr);
    let _ = (method_name_len, method_name_ptr);
    let _ = (arguments_len, arguments_ptr);
    let _ = (amount_ptr, gas);
    // TODO:
    0
}

#[unsafe(no_mangle)]
extern "C" fn promise_then(
    promise_index: u64,
    account_id_len: u64,
    account_id_ptr: u64,
    method_name_len: u64,
    method_name_ptr: u64,
    arguments_len: u64,
    arguments_ptr: u64,
    amount_ptr: u64,
    gas: u64,
) -> u64 {
    let _ = promise_index;
    let _ = (account_id_len, account_id_ptr);
    let _ = (method_name_len, method_name_ptr);
    let _ = (arguments_len, arguments_ptr);
    let _ = (amount_ptr, gas);
    // TODO:
    0
}

#[unsafe(no_mangle)]
extern "C" fn promise_and(promise_idx_ptr: u64, promise_idx_count: u64) -> u64 {
    let _ = (promise_idx_ptr, promise_idx_count);
    // TODO:
    0
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64 {
    let _ = (account_id_len, account_id_ptr);
    // TODO:
    0
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_then(
    promise_index: u64,
    account_id_len: u64,
    account_id_ptr: u64,
) -> u64 {
    let _ = promise_index;
    let _ = (account_id_len, account_id_ptr);
    // TODO:
    0
}

// #######################
// # Promise API actions #
// #######################

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_create_account(promise_index: u64) {
    let _ = promise_index;
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_deploy_contract(
    promise_index: u64,
    code_len: u64,
    code_ptr: u64,
) {
    let _ = promise_index;
    let _ = (code_len, code_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_function_call(
    promise_index: u64,
    method_name_len: u64,
    method_name_ptr: u64,
    arguments_len: u64,
    arguments_ptr: u64,
    amount_ptr: u64,
    gas: u64,
) {
    let _ = promise_index;
    let _ = (method_name_len, method_name_ptr);
    let _ = (arguments_len, arguments_ptr);
    let _ = (amount_ptr, gas);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_transfer(promise_index: u64, amount_ptr: u64) {
    let _ = promise_index;
    let _ = amount_ptr;
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_stake(
    promise_index: u64,
    amount_ptr: u64,
    public_key_len: u64,
    public_key_ptr: u64,
) {
    let _ = promise_index;
    let _ = amount_ptr;
    let _ = (public_key_len, public_key_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_add_key_with_full_access(
    promise_index: u64,
    public_key_len: u64,
    public_key_ptr: u64,
    nonce: u64,
) {
    let _ = promise_index;
    let _ = (public_key_len, public_key_ptr);
    let _ = nonce;
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_add_key_with_function_call(
    promise_index: u64,
    public_key_len: u64,
    public_key_ptr: u64,
    nonce: u64,
    allowance_ptr: u64,
    receiver_id_len: u64,
    receiver_id_ptr: u64,
    method_names_len: u64,
    method_names_ptr: u64,
) {
    let _ = promise_index;
    let _ = (public_key_len, public_key_ptr);
    let _ = nonce;
    let _ = allowance_ptr;
    let _ = (receiver_id_len, receiver_id_ptr);
    let _ = (method_names_len, method_names_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_delete_key(
    promise_index: u64,
    public_key_len: u64,
    public_key_ptr: u64,
) {
    let _ = promise_index;
    let _ = (public_key_len, public_key_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn promise_batch_action_delete_account(
    promise_index: u64,
    beneficiary_id_len: u64,
    beneficiary_id_ptr: u64,
) {
    let _ = promise_index;
    let _ = (beneficiary_id_len, beneficiary_id_ptr);
    unimplemented!()
}

// #######################
// # Promise API results #
// #######################

#[unsafe(no_mangle)]
extern "C" fn promise_results_count() -> u64 {
    STATE.promise_results_count()
}

#[unsafe(no_mangle)]
extern "C" fn promise_result(result_idx: u64, register_id: u64) -> u64 {
    STATE.promise_result(result_idx, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn promise_return(promise_id: u64) {
    let _ = promise_id;
    unimplemented!()
}

// ###############
// # Storage API #
// ###############

#[unsafe(no_mangle)]
extern "C" fn storage_write(
    key_len: u64,
    key_ptr: u64,
    value_len: u64,
    value_ptr: u64,
    register_id: u64,
) -> u64 {
    STATE.storage_write(key_len, key_ptr, value_len, value_ptr, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
    STATE.storage_read(key_len, key_ptr, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
    STATE.storage_remove(key_len, key_ptr, register_id)
}

#[unsafe(no_mangle)]
extern "C" fn storage_has_key(key_len: u64, key_ptr: u64) -> u64 {
    STATE.storage_has_key(key_len, key_ptr)
}

#[unsafe(no_mangle)]
extern "C" fn storage_iter_prefix(prefix_len: u64, prefix_ptr: u64) -> u64 {
    let _ = (prefix_len, prefix_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn storage_iter_range(
    start_len: u64,
    start_ptr: u64,
    end_len: u64,
    end_ptr: u64,
) -> u64 {
    let _ = (start_len, start_ptr);
    let _ = (end_len, end_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn storage_iter_next(
    iterator_id: u64,
    key_register_id: u64,
    value_register_id: u64,
) -> u64 {
    let _ = (iterator_id, key_register_id, value_register_id);
    unimplemented!()
}

// ###############
// # Validator API #
// ###############

#[unsafe(no_mangle)]
extern "C" fn validator_stake(account_id_len: u64, account_id_ptr: u64, stake_ptr: u64) {
    let _ = (account_id_len, account_id_ptr, stake_ptr);
    unimplemented!()
}

#[unsafe(no_mangle)]
extern "C" fn validator_total_stake(stake_ptr: u64) {
    let _ = stake_ptr;
    unimplemented!()
}
