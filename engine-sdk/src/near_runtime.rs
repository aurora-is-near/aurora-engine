use crate::io::StorageIntermediate;
use crate::prelude::NearGas;
use crate::promise::PromiseId;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::{
    NearPublicKey, PromiseAction, PromiseBatchAction, PromiseCreateArgs,
};
use aurora_engine_types::types::PromiseResult;
use aurora_engine_types::H256;

#[cfg(all(feature = "mainnet", not(feature = "testnet")))]
/// The mainnet eth_custodian address 0x6BFaD42cFC4EfC96f529D786D643Ff4A8B89FA52
const CUSTODIAN_ADDRESS: &[u8] = &[
    107, 250, 212, 44, 252, 78, 252, 150, 245, 41, 215, 134, 214, 67, 255, 74, 139, 137, 250, 82,
];

#[cfg(feature = "testnet")]
/// The testnet eth_custodian address 0x84a82Bb39c83989D5Dc07e1310281923D2544dC2
const CUSTODIAN_ADDRESS: &[u8] = &[
    132, 168, 43, 179, 156, 131, 152, 157, 93, 192, 126, 19, 16, 40, 25, 35, 210, 84, 77, 194,
];

macro_rules! feature_gated {
    ($feature_name:literal, $code:block) => {
        if cfg!(feature = $feature_name) {
            $code
        } else {
            unimplemented!("Not implemented without feature {}", $feature_name)
        }
    };
}

/// Wrapper type for indices in NEAR's register API.
pub struct RegisterIndex(u64);

/// Singleton type used to implement the IO traits in the case of using NEAR's
/// runtime (i.e. for wasm contracts).
#[derive(Copy, Clone, Default)]
pub struct Runtime;

impl Runtime {
    const READ_STORAGE_REGISTER_ID: RegisterIndex = RegisterIndex(0);
    const INPUT_REGISTER_ID: RegisterIndex = RegisterIndex(1);
    const WRITE_REGISTER_ID: RegisterIndex = RegisterIndex(2);
    const EVICT_REGISTER_ID: RegisterIndex = RegisterIndex(3);
    const ENV_REGISTER_ID: RegisterIndex = RegisterIndex(4);
    const PROMISE_REGISTER_ID: RegisterIndex = RegisterIndex(5);

    const GAS_FOR_STATE_MIGRATION: NearGas = NearGas::new(100_000_000_000_000);

    /// Deploy code from given key in place of the current contract.
    /// Not implemented in terms of higher level traits (eg IO) for efficiency reasons.
    pub fn self_deploy(code_key: &[u8]) {
        unsafe {
            // Load current account id into register 0.
            exports::current_account_id(0);
            // Use register 0 as the destination for the promise.
            let promise_id = exports::promise_batch_create(u64::MAX as _, 0);
            // Remove code from storage and store it in register 1.
            exports::storage_remove(code_key.len() as _, code_key.as_ptr() as _, 1);
            exports::promise_batch_action_deploy_contract(promise_id, u64::MAX, 1);
            Self::promise_batch_action_function_call(
                promise_id,
                b"state_migration",
                &[],
                0,
                Self::GAS_FOR_STATE_MIGRATION.as_u64(),
            )
        }
    }

    /// Assumes a valid account ID has been written to ENV_REGISTER_ID
    /// by a previous call.
    fn read_account_id() -> AccountId {
        let bytes = Self::ENV_REGISTER_ID.to_vec();
        match AccountId::try_from(bytes) {
            Ok(account_id) => account_id,
            // the environment must give us a valid Account ID.
            Err(_) => unreachable!(),
        }
    }

    /// Convenience wrapper around `exports::promise_batch_action_function_call`
    fn promise_batch_action_function_call(
        promise_idx: u64,
        method_name: &[u8],
        arguments: &[u8],
        amount: u128,
        gas: u64,
    ) {
        unsafe {
            exports::promise_batch_action_function_call(
                promise_idx,
                method_name.len() as _,
                method_name.as_ptr() as _,
                arguments.len() as _,
                arguments.as_ptr() as _,
                &amount as *const u128 as _,
                gas,
            )
        }
    }
}

impl StorageIntermediate for RegisterIndex {
    fn len(&self) -> usize {
        unsafe {
            let result = exports::register_len(self.0);
            // By convention, an unused register will return a length of U64::MAX
            // (see https://nomicon.io/RuntimeSpec/Components/BindingsSpec/RegistersAPI.html).
            if result < u64::MAX {
                result as usize
            } else {
                0
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        unsafe { exports::read_register(self.0, buffer.as_ptr() as u64) }
    }
}

impl crate::io::IO for Runtime {
    type StorageValue = RegisterIndex;

    fn read_input(&self) -> Self::StorageValue {
        unsafe {
            exports::input(Runtime::INPUT_REGISTER_ID.0);
        }
        Runtime::INPUT_REGISTER_ID
    }

    fn return_output(&mut self, value: &[u8]) {
        unsafe {
            #[cfg(any(feature = "mainnet", feature = "testnet"))]
            if value.len() >= 56 && &value[36..56] == CUSTODIAN_ADDRESS {
                panic!("ERR_ILLEGAL_RETURN");
            }
            exports::value_return(value.len() as u64, value.as_ptr() as u64);
        }
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_read(
                key.len() as u64,
                key.as_ptr() as u64,
                Runtime::READ_STORAGE_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::READ_STORAGE_REGISTER_ID)
            } else {
                None
            }
        }
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        unsafe { exports::storage_has_key(key.len() as _, key.as_ptr() as _) == 1 }
    }

    fn write_storage(&mut self, key: &[u8], value: &[u8]) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_write(
                key.len() as u64,
                key.as_ptr() as u64,
                value.len() as u64,
                value.as_ptr() as u64,
                Runtime::WRITE_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::WRITE_REGISTER_ID)
            } else {
                None
            }
        }
    }

    fn write_storage_direct(
        &mut self,
        key: &[u8],
        value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_write(
                key.len() as _,
                key.as_ptr() as _,
                u64::MAX,
                value.0,
                Runtime::WRITE_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::WRITE_REGISTER_ID)
            } else {
                None
            }
        }
    }

    fn remove_storage(&mut self, key: &[u8]) -> Option<Self::StorageValue> {
        unsafe {
            if exports::storage_remove(
                key.len() as _,
                key.as_ptr() as _,
                Runtime::EVICT_REGISTER_ID.0,
            ) == 1
            {
                Some(Runtime::EVICT_REGISTER_ID)
            } else {
                None
            }
        }
    }
}

impl crate::env::Env for Runtime {
    fn signer_account_id(&self) -> AccountId {
        unsafe {
            exports::signer_account_id(Self::ENV_REGISTER_ID.0);
        }
        Self::read_account_id()
    }

    fn current_account_id(&self) -> AccountId {
        unsafe {
            exports::current_account_id(Self::ENV_REGISTER_ID.0);
        }
        Self::read_account_id()
    }

    fn predecessor_account_id(&self) -> AccountId {
        unsafe {
            exports::predecessor_account_id(Self::ENV_REGISTER_ID.0);
        }
        Self::read_account_id()
    }

    fn block_height(&self) -> u64 {
        unsafe { exports::block_index() }
    }

    fn block_timestamp(&self) -> crate::env::Timestamp {
        let ns = unsafe { exports::block_timestamp() };
        crate::env::Timestamp::new(ns)
    }

    fn attached_deposit(&self) -> u128 {
        unsafe {
            let data = [0u8; core::mem::size_of::<u128>()];
            exports::attached_deposit(data.as_ptr() as u64);
            u128::from_le_bytes(data)
        }
    }

    fn random_seed(&self) -> H256 {
        unsafe {
            exports::random_seed(0);
            let bytes = H256::zero();
            exports::read_register(0, bytes.0.as_ptr() as *const u64 as u64);
            bytes
        }
    }

    fn prepaid_gas(&self) -> NearGas {
        NearGas::new(unsafe { exports::prepaid_gas() })
    }
}

impl crate::promise::PromiseHandler for Runtime {
    type ReadOnly = Self;

    fn promise_results_count(&self) -> u64 {
        unsafe { exports::promise_results_count() }
    }

    fn promise_result(&self, index: u64) -> Option<PromiseResult> {
        unsafe {
            match exports::promise_result(index, Self::PROMISE_REGISTER_ID.0) {
                0 => Some(PromiseResult::NotReady),
                1 => {
                    let bytes = Self::PROMISE_REGISTER_ID.to_vec();
                    Some(PromiseResult::Successful(bytes))
                }
                2 => Some(PromiseResult::Failed),
                _ => None,
            }
        }
    }

    unsafe fn promise_create_call(&mut self, args: &PromiseCreateArgs) -> PromiseId {
        let account_id = args.target_account_id.as_bytes();
        let method_name = args.method.as_bytes();
        let arguments = args.args.as_slice();
        let amount = args.attached_balance.as_u128();
        let gas = args.attached_gas.as_u64();

        let id = {
            exports::promise_create(
                account_id.len() as _,
                account_id.as_ptr() as _,
                method_name.len() as _,
                method_name.as_ptr() as _,
                arguments.len() as _,
                arguments.as_ptr() as _,
                &amount as *const u128 as _,
                gas,
            )
        };
        PromiseId::new(id)
    }

    unsafe fn promise_attach_callback(
        &mut self,
        base: PromiseId,
        callback: &PromiseCreateArgs,
    ) -> PromiseId {
        let account_id = callback.target_account_id.as_bytes();
        let method_name = callback.method.as_bytes();
        let arguments = callback.args.as_slice();
        let amount = callback.attached_balance.as_u128();
        let gas = callback.attached_gas.as_u64();

        let id = {
            exports::promise_then(
                base.raw(),
                account_id.len() as _,
                account_id.as_ptr() as _,
                method_name.len() as _,
                method_name.as_ptr() as _,
                arguments.len() as _,
                arguments.as_ptr() as _,
                &amount as *const u128 as _,
                gas,
            )
        };

        PromiseId::new(id)
    }

    unsafe fn promise_create_batch(&mut self, args: &PromiseBatchAction) -> PromiseId {
        let account_id = args.target_account_id.as_bytes();

        let id = { exports::promise_batch_create(account_id.len() as _, account_id.as_ptr() as _) };

        for action in args.actions.iter() {
            match action {
                PromiseAction::CreateAccount => {
                    exports::promise_batch_action_create_account(id);
                }
                PromiseAction::Transfer { amount } => {
                    let amount = amount.as_u128();
                    exports::promise_batch_action_transfer(id, &amount as *const u128 as _);
                }
                PromiseAction::DeployContract { code } => {
                    let code = code.as_slice();
                    exports::promise_batch_action_deploy_contract(
                        id,
                        code.len() as _,
                        code.as_ptr() as _,
                    );
                }
                PromiseAction::FunctionCall {
                    name,
                    gas,
                    attached_yocto,
                    args,
                } => {
                    let method_name = name.as_bytes();
                    let arguments = args.as_slice();
                    let amount = attached_yocto.as_u128();
                    exports::promise_batch_action_function_call(
                        id,
                        method_name.len() as _,
                        method_name.as_ptr() as _,
                        arguments.len() as _,
                        arguments.as_ptr() as _,
                        &amount as *const u128 as _,
                        gas.as_u64(),
                    )
                }
                PromiseAction::Stake { amount, public_key } => {
                    feature_gated!("all-promise-actions", {
                        let amount = amount.as_u128();
                        let pk: RawPublicKey = public_key.into();
                        let pk_bytes = pk.as_bytes();
                        exports::promise_batch_action_stake(
                            id,
                            &amount as *const u128 as _,
                            pk_bytes.len() as _,
                            pk_bytes.as_ptr() as _,
                        )
                    });
                }
                PromiseAction::AddFullAccessKey { public_key, nonce } => {
                    feature_gated!("all-promise-actions", {
                        let pk: RawPublicKey = public_key.into();
                        let pk_bytes = pk.as_bytes();
                        exports::promise_batch_action_add_key_with_full_access(
                            id,
                            pk_bytes.len() as _,
                            pk_bytes.as_ptr() as _,
                            *nonce,
                        )
                    });
                }
                PromiseAction::AddFunctionCallKey {
                    public_key,
                    nonce,
                    allowance,
                    receiver_id,
                    function_names,
                } => {
                    feature_gated!("all-promise-actions", {
                        let pk: RawPublicKey = public_key.into();
                        let pk_bytes = pk.as_bytes();
                        let allowance = allowance.as_u128();
                        let receiver_id = receiver_id.as_bytes();
                        let function_names = function_names.as_bytes();
                        exports::promise_batch_action_add_key_with_function_call(
                            id,
                            pk_bytes.len() as _,
                            pk_bytes.as_ptr() as _,
                            *nonce,
                            &allowance as *const u128 as _,
                            receiver_id.len() as _,
                            receiver_id.as_ptr() as _,
                            function_names.len() as _,
                            function_names.as_ptr() as _,
                        )
                    });
                }
                PromiseAction::DeleteKey { public_key } => {
                    feature_gated!("all-promise-actions", {
                        let pk: RawPublicKey = public_key.into();
                        let pk_bytes = pk.as_bytes();
                        exports::promise_batch_action_delete_key(
                            id,
                            pk_bytes.len() as _,
                            pk_bytes.as_ptr() as _,
                        )
                    });
                }
                PromiseAction::DeleteAccount { beneficiary_id } => {
                    feature_gated!("all-promise-actions", {
                        let beneficiary_id = beneficiary_id.as_bytes();
                        exports::promise_batch_action_delete_key(
                            id,
                            beneficiary_id.len() as _,
                            beneficiary_id.as_ptr() as _,
                        )
                    });
                }
            }
        }

        PromiseId::new(id)
    }

    fn promise_return(&mut self, promise: PromiseId) {
        unsafe {
            exports::promise_return(promise.raw());
        }
    }

    fn read_only(&self) -> Self::ReadOnly {
        Self
    }
}

/// Similar to NearPublicKey, except the first byte includes
/// the curve identifier.
enum RawPublicKey {
    Ed25519([u8; 33]),
    Secp256k1([u8; 65]),
}

impl RawPublicKey {
    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Ed25519(bytes) => bytes,
            Self::Secp256k1(bytes) => bytes,
        }
    }
}

impl<'a> From<&'a NearPublicKey> for RawPublicKey {
    fn from(key: &'a NearPublicKey) -> Self {
        match key {
            NearPublicKey::Ed25519(bytes) => {
                let mut buf = [0u8; 33];
                buf[1..33].copy_from_slice(bytes);
                Self::Ed25519(buf)
            }
            NearPublicKey::Secp256k1(bytes) => {
                let mut buf = [0u8; 65];
                buf[0] = 0x01;
                buf[1..65].copy_from_slice(bytes);
                Self::Secp256k1(buf)
            }
        }
    }
}

/// Some host functions are not usable in NEAR view calls.
/// This struct puts in default values for those calls instead.
pub struct ViewEnv;

impl crate::env::Env for ViewEnv {
    fn signer_account_id(&self) -> AccountId {
        AccountId::new("system").unwrap()
    }

    fn current_account_id(&self) -> AccountId {
        unsafe {
            exports::current_account_id(Runtime::ENV_REGISTER_ID.0);
        }
        Runtime::read_account_id()
    }

    fn predecessor_account_id(&self) -> AccountId {
        AccountId::new("system").unwrap()
    }

    fn block_height(&self) -> u64 {
        unsafe { exports::block_index() }
    }

    fn block_timestamp(&self) -> crate::env::Timestamp {
        let ns = unsafe { exports::block_timestamp() };
        crate::env::Timestamp::new(ns)
    }

    fn attached_deposit(&self) -> u128 {
        1
    }

    fn random_seed(&self) -> H256 {
        unsafe {
            exports::random_seed(0);
            let bytes = H256::zero();
            exports::read_register(0, bytes.0.as_ptr() as *const u64 as u64);
            bytes
        }
    }

    fn prepaid_gas(&self) -> NearGas {
        NearGas::new(300)
    }
}

pub(crate) mod exports {
    #[allow(unused)]
    extern "C" {
        // #############
        // # Registers #
        // #############
        pub(crate) fn read_register(register_id: u64, ptr: u64);
        pub(crate) fn register_len(register_id: u64) -> u64;
        // ###############
        // # Context API #
        // ###############
        pub(crate) fn current_account_id(register_id: u64);
        pub(crate) fn signer_account_id(register_id: u64);
        pub(crate) fn signer_account_pk(register_id: u64);
        pub(crate) fn predecessor_account_id(register_id: u64);
        pub(crate) fn input(register_id: u64);
        // TODO #1903 fn block_height() -> u64;
        pub(crate) fn block_index() -> u64;
        pub(crate) fn block_timestamp() -> u64;
        fn epoch_height() -> u64;
        pub(crate) fn storage_usage() -> u64;
        // #################
        // # Economics API #
        // #################
        fn account_balance(balance_ptr: u64);
        pub(crate) fn attached_deposit(balance_ptr: u64);
        pub(crate) fn prepaid_gas() -> u64;
        fn used_gas() -> u64;
        // ############
        // # Math API #
        // ############
        pub(crate) fn random_seed(register_id: u64);
        pub(crate) fn sha256(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn keccak256(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn ripemd160(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn ecrecover(
            hash_len: u64,
            hash_ptr: u64,
            sig_len: u64,
            sig_ptr: u64,
            v: u64,
            malleability_flag: u64,
            register_id: u64,
        ) -> u64;
        pub(crate) fn alt_bn128_g1_sum(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn alt_bn128_g1_multiexp(value_len: u64, value_ptr: u64, register_id: u64);
        pub(crate) fn alt_bn128_pairing_check(value_len: u64, value_ptr: u64) -> u64;
        // #####################
        // # Miscellaneous API #
        // #####################
        pub(crate) fn value_return(value_len: u64, value_ptr: u64);
        pub(crate) fn panic();
        pub(crate) fn panic_utf8(len: u64, ptr: u64);
        pub(crate) fn log_utf8(len: u64, ptr: u64);
        fn log_utf16(len: u64, ptr: u64);
        fn abort(msg_ptr: u32, filename_ptr: u32, line: u32, col: u32);
        // ################
        // # Promises API #
        // ################
        pub(crate) fn promise_create(
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        pub(crate) fn promise_then(
            promise_index: u64,
            account_id_len: u64,
            account_id_ptr: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        ) -> u64;
        fn promise_and(promise_idx_ptr: u64, promise_idx_count: u64) -> u64;
        pub(crate) fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64;
        fn promise_batch_then(promise_index: u64, account_id_len: u64, account_id_ptr: u64) -> u64;
        // #######################
        // # Promise API actions #
        // #######################
        pub(crate) fn promise_batch_action_create_account(promise_index: u64);
        pub(crate) fn promise_batch_action_deploy_contract(
            promise_index: u64,
            code_len: u64,
            code_ptr: u64,
        );
        pub(crate) fn promise_batch_action_function_call(
            promise_index: u64,
            method_name_len: u64,
            method_name_ptr: u64,
            arguments_len: u64,
            arguments_ptr: u64,
            amount_ptr: u64,
            gas: u64,
        );
        pub(crate) fn promise_batch_action_transfer(promise_index: u64, amount_ptr: u64);
        pub(crate) fn promise_batch_action_stake(
            promise_index: u64,
            amount_ptr: u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
        pub(crate) fn promise_batch_action_add_key_with_full_access(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
            nonce: u64,
        );
        pub(crate) fn promise_batch_action_add_key_with_function_call(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
            nonce: u64,
            allowance_ptr: u64,
            receiver_id_len: u64,
            receiver_id_ptr: u64,
            method_names_len: u64,
            method_names_ptr: u64,
        );
        pub(crate) fn promise_batch_action_delete_key(
            promise_index: u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
        pub(crate) fn promise_batch_action_delete_account(
            promise_index: u64,
            beneficiary_id_len: u64,
            beneficiary_id_ptr: u64,
        );
        // #######################
        // # Promise API results #
        // #######################
        pub(crate) fn promise_results_count() -> u64;
        pub(crate) fn promise_result(result_idx: u64, register_id: u64) -> u64;
        pub(crate) fn promise_return(promise_id: u64);
        // ###############
        // # Storage API #
        // ###############
        pub(crate) fn storage_write(
            key_len: u64,
            key_ptr: u64,
            value_len: u64,
            value_ptr: u64,
            register_id: u64,
        ) -> u64;
        pub(crate) fn storage_read(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
        pub(crate) fn storage_remove(key_len: u64, key_ptr: u64, register_id: u64) -> u64;
        pub(crate) fn storage_has_key(key_len: u64, key_ptr: u64) -> u64;
        fn storage_iter_prefix(prefix_len: u64, prefix_ptr: u64) -> u64;
        fn storage_iter_range(start_len: u64, start_ptr: u64, end_len: u64, end_ptr: u64) -> u64;
        fn storage_iter_next(iterator_id: u64, key_register_id: u64, value_register_id: u64)
            -> u64;
        // ###############
        // # Validator API #
        // ###############
        fn validator_stake(account_id_len: u64, account_id_ptr: u64, stake_ptr: u64);
        fn validator_total_stake(stake_ptr: u64);
    }
}
