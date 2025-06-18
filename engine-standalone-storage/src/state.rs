#![allow(clippy::as_conversions)]

use aurora_engine_sdk::env::Fixed;
use sha2::digest::{FixedOutput, Update};
use std::cell::RefCell;
use std::{borrow::Cow, iter, slice};

use super::{sync::types::TransactionKindTag, Diff, Storage};

thread_local! {
    pub static STATE: RefCell<State> = panic!("State is not initialized");
}

pub struct State {
    inner: RefCell<StateInner>,
    registers: RefCell<Vec<Register>>,
    db: RefCell<Storage>,
}

#[derive(Default)]
struct Register(Option<Vec<u8>>);

struct StateInner {
    env: Option<Fixed>,
    input: Vec<u8>,
    output: Vec<u8>,
    promise_data: Box<[Option<Vec<u8>>]>,

    bound_block_height: u64,
    bound_tx_position: u16,
    transaction_diff: Diff,
}

const REGISTERS_NUMBER: usize = 6;

impl Default for StateInner {
    fn default() -> Self {
        Self {
            env: None,
            input: vec![],
            output: vec![],
            promise_data: Box::new([]),
            bound_block_height: 0,
            bound_tx_position: 0,
            transaction_diff: Diff::default(),
        }
    }
}

impl State {
    pub fn new(storage: Storage) -> Self {
        Self {
            inner: RefCell::new(StateInner::default()),
            registers: RefCell::new(
                iter::repeat_with(Register::default)
                    .take(REGISTERS_NUMBER)
                    .collect(),
            ),
            db: RefCell::new(storage),
        }
    }

    pub fn set_env(&self, env: Fixed) {
        self.inner.borrow_mut().env = Some(env);
    }

    pub fn set_promise_handler(&self, promise_data: Box<[Option<Vec<u8>>]>) {
        self.inner.borrow_mut().promise_data = promise_data;
    }

    #[must_use]
    pub fn take_output(&self) -> Vec<u8> {
        self.inner.borrow_mut().output.clone()
    }

    #[must_use]
    pub fn get_transaction_diff(&self) -> Diff {
        self.inner.borrow_mut().transaction_diff.clone()
    }

    pub fn init(
        &self,
        storage: Storage,
        block_height: u64,
        transaction_position: u16,
        input: Vec<u8>,
    ) {
        *self.db.borrow_mut() = storage;
        let mut lock = self.inner.borrow_mut();
        lock.bound_block_height = block_height;
        lock.bound_tx_position = transaction_position;
        lock.input = input;
    }

    pub fn reset(&self) {
        *self.inner.borrow_mut() = StateInner::default();
        *self.registers.borrow_mut() = iter::repeat_with(Register::default)
            .take(REGISTERS_NUMBER)
            .collect();
    }

    #[cfg(not(feature = "integration-test"))]
    fn dbg(&self, _args: std::fmt::Arguments) {}

    #[cfg(feature = "integration-test")]
    fn dbg(&self, args: std::fmt::Arguments) {
        use std::{fs::File, io::Write, ptr};

        let mut dst = File::options()
            .append(true)
            .create(true)
            .open("../target/dbg.txt")
            .unwrap();
        dst.write_fmt(format_args!("{:?}: {args}", ptr::from_ref(self)))
            .unwrap();
        dst.flush().unwrap();
    }

    pub fn store_dbg_diff(&self) {
        let lock = self.inner.borrow();
        self.dbg(format_args!("diff: {:?}\n", lock.transaction_diff));
        self.dbg(format_args!("output: {}\n", hex::encode(&lock.output)));
    }

    pub fn store_dbg_info(&self, call: TransactionKindTag) {
        let lock = self.inner.borrow();
        self.dbg(format_args!(
            "block {}.{}, promise {}\n",
            lock.bound_block_height,
            lock.bound_tx_position,
            lock.promise_data.len(),
        ));
        self.dbg(format_args!(
            "{call:?} with input \"{}\"\n",
            hex::encode(&lock.input),
        ));
        self.dbg(format_args!("env: {:?}\n", lock.env));
    }

    #[allow(clippy::significant_drop_tightening)]
    fn read_reg<F, T>(&self, register_id: u64, mut op: F) -> T
    where
        F: FnMut(&Register) -> T,
    {
        let index = usize::try_from(register_id).expect("pointer size must be wide enough");
        let registers = self.registers.borrow();
        let reg = registers
            .get(index)
            .unwrap_or_else(|| panic!("no such register {register_id}"));
        self.dbg(format_args!(
            "register {register_id} -> {}\n",
            reg.0.as_ref().map_or("deadbeef".to_string(), hex::encode)
        ));
        op(reg)
    }

    fn set_reg(&self, register_id: u64, data: Cow<[u8]>) {
        let index = usize::try_from(register_id).expect("pointer size must be wide enough");
        self.dbg(format_args!("register {index} <- {}\n", hex::encode(&data)));

        let mut registers = self.registers.borrow_mut();
        *registers
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
            let len = usize::try_from(len).expect("pointer size must be wide enough");
            Cow::Borrowed(unsafe { slice::from_raw_parts(ptr as *const u8, len) })
        }
    }

    //// Near API

    pub(crate) fn read_register(&self, register_id: u64, ptr: u64) {
        self.read_reg(register_id, |reg| {
            if let Some(reg) = &reg.0 {
                unsafe { (ptr as *mut u8).copy_from_nonoverlapping(reg.as_ptr(), reg.len()) };
            }
        });
    }

    pub(crate) fn register_len(&self, register_id: u64) -> u64 {
        self.read_reg(register_id, |reg| {
            reg.0.as_ref().map_or(u64::MAX, |reg| {
                reg.len()
                    .try_into()
                    .expect("pointer size must be wide enough")
            })
        })
    }

    pub(crate) fn current_account_id(&self, register_id: u64) {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.current_account_id.as_bytes().into());
    }

    pub(crate) fn signer_account_id(&self, register_id: u64) {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.signer_account_id.as_bytes().into());
    }

    pub(crate) fn predecessor_account_id(&self, register_id: u64) {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.predecessor_account_id.as_bytes().into());
    }

    pub(crate) fn input(&self, register_id: u64) {
        let input = &self.inner.borrow().input;
        self.set_reg(register_id, input.into());
    }

    pub(crate) fn block_index(&self) -> u64 {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        env.block_height
    }

    pub(crate) fn block_timestamp(&self) -> u64 {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        env.block_timestamp.nanos()
    }

    pub(crate) fn attached_deposit(&self, balance_ptr: u64) {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        unsafe { (balance_ptr as *mut u128).write(env.attached_deposit) }
    }

    pub(crate) fn prepaid_gas(&self) -> u64 {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        env.prepaid_gas.as_u64()
    }

    pub(crate) fn used_gas(&self) -> u64 {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        env.used_gas.as_u64()
    }

    pub(crate) fn random_seed(&self, register_id: u64) {
        let Some(env) = &self.inner.borrow().env else {
            panic!("environment is not set");
        };
        self.set_reg(register_id, env.random_seed.as_bytes().into());
    }

    pub(crate) fn digest<D: Default + Update + FixedOutput>(
        &self,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) {
        let data = self.get_data(value_ptr, value_len);
        let hash = D::default().chain(data).finalize_fixed();
        self.set_reg(register_id, hash.as_slice().into());
    }

    pub(crate) fn ecrecover(
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
            0..=26 => u8::try_from(v).expect("checked above"),
            _ => u8::try_from(v - 27).expect("bad value of `v`"),
        };
        let recovery_id = libsecp256k1::RecoveryId::parse(bit).map_err(|_| ())?;

        let public_key = libsecp256k1::recover(&hash, &sig, &recovery_id).map_err(|_| ())?;
        self.set_reg(register_id, public_key.serialize()[1..].into());

        Ok(())
    }

    pub(crate) fn value_return(&self, value_len: u64, value_ptr: u64) {
        let data = self.get_data(value_ptr, value_len);
        self.inner.borrow_mut().output = data.into_owned();
    }

    pub(crate) fn promise_results_count(&self) -> u64 {
        u64::try_from(self.inner.borrow().promise_data.len()).unwrap_or_default()
    }

    pub(crate) fn promise_result(&self, result_idx: u64, register_id: u64) -> u64 {
        let i = usize::try_from(result_idx).expect("index too big");
        let lock = self.inner.borrow();
        let Some(data) = lock.promise_data.get(i) else {
            // not ready
            return 0;
        };
        let Some(data) = data else {
            // failed
            return 2;
        };
        // ready
        self.set_reg(register_id, data.as_slice().into());
        1
    }

    pub(crate) fn storage_write(
        &self,
        key_len: u64,
        key_ptr: u64,
        value_len: u64,
        value_ptr: u64,
        register_id: u64,
    ) -> u64 {
        // preserve the register value
        let value = self.get_data(value_ptr, value_len);

        // fetch original value into register
        let res = self.storage_read(key_len, key_ptr, register_id);

        let key = self.get_data(key_ptr, key_len);
        self.dbg(format_args!(
            "diff write {register_id} {} <- {}\n",
            hex::encode(&key),
            hex::encode(&value)
        ));

        self.inner
            .borrow_mut()
            .transaction_diff
            .modify(key.to_vec(), value.to_vec());
        res
    }

    pub(crate) fn storage_read(&self, key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
        let key = self.get_data(key_ptr, key_len);
        self.dbg(format_args!(
            "try to read {register_id} {}\n",
            hex::encode(&key),
        ));

        let lock = self.inner.borrow();
        if let Some(diff) = lock.transaction_diff.get(&key) {
            return diff.value().map_or(0, |bytes| {
                self.set_reg(register_id, bytes.into());
                self.dbg(format_args!(
                    "diff read {register_id} {} <- {}\n",
                    hex::encode(&key),
                    hex::encode(bytes),
                ));
                1
            });
        }

        if let Ok(value) =
            self.db
                .borrow()
                .read_by_key(&key, lock.bound_block_height, lock.bound_tx_position)
        {
            return value.value().map_or(0, |bytes| {
                self.set_reg(register_id, bytes.into());
                self.dbg(format_args!(
                    "db read {register_id} {} <- {}\n",
                    hex::encode(&key),
                    hex::encode(bytes),
                ));
                1
            });
        }

        0
    }

    pub(crate) fn storage_remove(&self, key_len: u64, key_ptr: u64, register_id: u64) -> u64 {
        // fetch original value into register
        let res = self.storage_read(key_len, key_ptr, register_id);

        let key = self.get_data(key_ptr, key_len);
        self.inner
            .borrow_mut()
            .transaction_diff
            .delete(key.into_owned());
        res
    }

    pub(crate) fn storage_has_key(&self, key_len: u64, key_ptr: u64) -> u64 {
        let key = self.get_data(key_ptr, key_len);
        let lock = self.inner.borrow();
        if lock.transaction_diff.get(&key).is_some() {
            return 1;
        }

        self.db
            .borrow()
            .read_by_key(&key, lock.bound_block_height, lock.bound_tx_position)
            .map_or(0, |diff| u64::from(diff.value().is_some()))
    }
}
