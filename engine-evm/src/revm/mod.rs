use crate::revm::utility::{
    compute_block_hash, from_address, get_balance, get_code, get_code_by_code_hash, get_generation,
    get_nonce, get_storage, h160_to_address, h256_to_u256, is_account_empty, remove_account,
    remove_storage, set_balance, set_code, set_code_hash, set_nonce, set_storage, u256_to_u256,
    wei_to_u256,
};
use crate::{BlockInfo, EVMHandler, Log, TransactExecutionResult, TransactResult, TransactionInfo};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::parameters::engine::TransactionStatus;
use aurora_engine_types::Vec;
use revm::handler::LoadPrecompilesHandle;
use revm::primitives::{
    Account, AccountInfo, Address, Bytecode, Env, HashMap, ResultAndState, SpecId, TransactTo,
    B256, KECCAK_EMPTY, U256,
};
use revm::{Database, DatabaseCommit, Evm};

mod accounting;
mod utility;

pub const EVM_FORK: SpecId = SpecId::LATEST;

/// REVM handler
pub struct REVMHandler<'env, I: IO, E: aurora_engine_sdk::env::Env> {
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> REVMHandler<'env, I, E> {
    pub fn new(
        io: I,
        env: &'env E,
        transaction: &'env TransactionInfo,
        block: &'env BlockInfo,
    ) -> Self {
        Self {
            io,
            env,
            transaction,
            block,
        }
        /*
        // TRANSACT_CREATE
        // caller: H160,
        // value: U256,
        // init_code: Vec<u8>,
        // gas_limit: u64,
        // access_list: Vec<(H160, Vec<H256>)>,

        // TRANSACT_CALL
        // caller: H160,
        // address: H160,
        // value: U256,
        // data: Vec<u8>,
        // gas_limit: u64,
        // access_list: Vec<(H160, Vec<H256>)>,
        */
    }

    /// EVM precompiles
    /// TODO: adjust it
    fn set_precompiles<'a>(precompiles: &LoadPrecompilesHandle<'a>) -> LoadPrecompilesHandle<'a> {
        // TODO: extend precompiles
        // let c = precompiles();
        // Arc::new(move || c.clone())
        precompiles.clone()
    }

    /// REVM Environment
    fn evm_env(&self) -> Env {
        let mut env = Env::default();
        // Set Config data
        let chain_id = aurora_engine_types::U256::from(self.block.chain_id).as_u64();
        env.cfg.chain_id = chain_id;
        // Set Block data
        env.block.gas_limit = U256::MAX;
        env.block.number = U256::from(self.env.block_height());
        env.block.coinbase = Address::new([
            0x44, 0x44, 0x58, 0x84, 0x43, 0xC3, 0xa9, 0x12, 0x88, 0xc5, 0x00, 0x24, 0x83, 0x44,
            0x9A, 0xba, 0x10, 0x54, 0x19, 0x2b,
        ]);
        env.block.timestamp = U256::from(self.env.block_timestamp().secs());
        env.block.difficulty = U256::ZERO;
        env.block.basefee = U256::ZERO;
        // Set transaction data
        env.tx.caller = h160_to_address(&self.transaction.origin);
        env.tx.gas_limit = self.transaction.gas_limit;
        env.tx.data = self.transaction.input.clone().into();
        // For Deploy it's value from CREATE
        env.tx.transact_to = if let Some(addr) = self.transaction.address {
            TransactTo::call(h160_to_address(&addr))
        } else {
            TransactTo::create()
        };
        env.tx.value = wei_to_u256(&self.transaction.value);
        env.tx.access_list = self
            .transaction
            .access_list
            .iter()
            .map(|(key, addr)| {
                (
                    h160_to_address(key),
                    addr.iter().map(|val| h256_to_u256(val)).collect(),
                )
            })
            .collect();
        env.tx.gas_price = u256_to_u256(&self.block.gas_price);
        // env.tx.nonce = get_nonce(origin)
        // env.tx.gas_priority_fee
        env
    }
}

/// REVM contract state handler
/// Operates with REVM `DB`
pub struct ContractState<'env, I: IO, E: aurora_engine_sdk::env::Env> {
    io: I,
    env: &'env E,
    transaction: &'env TransactionInfo,
    block: &'env BlockInfo,
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> ContractState<'env, I, E> {
    pub fn new(
        io: I,
        env: &'env E,
        transaction: &'env TransactionInfo,
        block: &'env BlockInfo,
    ) -> Self {
        Self {
            io,
            env,
            transaction,
            block,
        }
    }
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> Database for ContractState<'env, I, E> {
    type Error = ();

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let balance = get_balance(&self.io, &address);
        let nonce = get_nonce(&self.io, &address);
        let code_raw = get_code(&self.io, &address);
        let (code_hash, code) = if code_raw.is_empty() {
            (KECCAK_EMPTY, None)
        } else {
            let bytes = Bytecode::new_raw(code_raw.into());
            (bytes.hash_slow(), Some(bytes))
        };
        let acc = Some(AccountInfo {
            balance,
            nonce,
            code_hash,
            code,
        });
        Ok(acc)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        let bytes = if code_hash == KECCAK_EMPTY {
            Bytecode::default()
        } else {
            let code_raw = get_code_by_code_hash(&self.io, &code_hash);
            Bytecode::new_raw(code_raw.into())
        };
        Ok(bytes)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let generation = get_generation(&self.io, &address);
        Ok(get_storage(&self.io, &address, &index, generation))
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        let idx = U256::from(self.env.block_height());
        if idx.saturating_sub(U256::from(256)) <= number && number < idx {
            Ok(compute_block_hash(
                self.block.chain_id,
                number,
                self.block.current_account_id.as_bytes(),
            ))
        } else {
            Ok(B256::ZERO)
        }
    }
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> DatabaseCommit
    for ContractState<'env, I, E>
{
    fn commit(&mut self, evm_state: HashMap<Address, Account>) {
        let mut writes_counter: usize = 0;
        let mut code_bytes_written: usize = 0;
        let mut accounting = accounting::Accounting::default();
        for (address, account) in evm_state {
            if !account.is_touched() {
                continue;
            }

            let old_nonce = get_nonce(&self.io, &address);
            let old_balance = get_balance(&self.io, &address);
            if account.is_selfdestructed() {
                accounting.remove(old_balance);
                let generation = get_generation(&self.io, &address);
                remove_account(&mut self.io, &address, generation);
                writes_counter += 1;
                continue;
            }

            accounting.change(accounting::Change {
                new_value: account.info.balance,
                old_value: old_balance,
            });
            if old_nonce != account.info.nonce {
                set_nonce(&mut self.io, &address, account.info.nonce);
                writes_counter += 1;
            }
            if old_balance != account.info.balance {
                set_balance(&mut self.io, &address, &account.info.balance);
                writes_counter += 1;
            }
            if let Some(code) = account.info.code {
                if !code.is_empty() {
                    let code_hash = if account.info.code_hash == KECCAK_EMPTY {
                        code.hash_slow()
                    } else {
                        account.info.code_hash
                    };
                    set_code_hash(&mut self.io, &code_hash, code.bytes());
                    writes_counter += 1;

                    set_code(&mut self.io, &address, code.bytes());
                    writes_counter += 1;
                    code_bytes_written = code.len();
                    aurora_engine_sdk::log!(
                        "code_write_at_address {:?} {}",
                        address,
                        code_bytes_written
                    );
                }
            }

            // TODO: Reset storage - it's no corresponding flag `reset_storage` for REVM
            let generation = get_generation(&self.io, &address);
            // remove_all_storage(&mut self.io, &address, generation);
            // let next_generation = generation + 1;
            let next_generation = generation;

            // TODO: it's unknown behavior
            for (index, value) in account.storage {
                if value.present_value() == U256::default() {
                    remove_storage(&mut self.io, &address, &index, next_generation);
                } else {
                    set_storage(
                        &mut self.io,
                        &address,
                        &index,
                        &value.present_value(),
                        next_generation,
                    );
                }
                writes_counter += 1;
            }

            // We only need to remove the account if:
            // 1. we are supposed to delete an empty account
            // 2. the account is empty
            // 3. we didn't already clear out the storage (because if we did then there is
            //    nothing to do)
            if is_account_empty(&self.io, &address)
            // && generation == next_generation
            {
                remove_account(&mut self.io, &address, generation);
                writes_counter += 1;
            }
        }

        match accounting.net() {
            // Net loss is possible if `SELFDESTRUCT(self)` calls are made.
            accounting::Net::Lost(amount) => {
                let _ = amount;
                aurora_engine_sdk::log!("Burn {} ETH due to SELFDESTRUCT", amount);
                // TODO: implement for REVM
                // Apply changes for eth-connector. We ignore the `StorageReadError` intentionally since
                // if we cannot read the storage then there is nothing to remove.
                // if let Some(remove_eth) = self.remove_eth_fn.take() {
                //     //remove_eth(Wei::new(amount));
                // }
            }
            accounting::Net::Zero => (),
            accounting::Net::Gained(_) => {
                // It should be impossible to gain ETH using normal EVM operations in production.
                // In tests, we have convenience functions that can poof addresses with ETH out of nowhere.
                #[cfg(all(not(feature = "integration-test"), feature = "std"))]
                {
                    panic!("ERR_INVALID_ETH_SUPPLY_INCREASE");
                }
            }
        }

        // These variable are only used if logging feature is enabled.
        // In production logging is always enabled, so we can ignore the warnings.
        #[allow(unused_variables)]
        let total_bytes = 32 * writes_counter + code_bytes_written;
        aurora_engine_sdk::log!(
            "total_writes_count {}\ntotal_written_bytes {}",
            writes_counter,
            total_bytes
        );
    }
}

impl<'env, I: IO + Copy, E: aurora_engine_sdk::env::Env> EVMHandler for REVMHandler<'env, I, E> {
    fn transact_create(&mut self) -> TransactExecutionResult<TransactResult> {
        todo!()
    }

    fn transact_call(&mut self) -> TransactExecutionResult<TransactResult> {
        let mut state = ContractState {
            io: self.io,
            env: self.env,
            transaction: self.transaction,
            block: self.block,
        };
        let mut evm = Evm::builder()
            .with_db(&mut state)
            .modify_env(|e| *e = self.evm_env())
            .spec_id(EVM_FORK)
            .build();
        let exec_result = evm.transact();
        if let Ok(ResultAndState { result, state }) = evm.transact() {
            evm.context.evm.db.commit(state);
            // let mut logs = Vec::new();
            // for log in result.logs() {
            //     let address = from_address(&log.address).raw();
            //     logs.push(Log { address })
            // }
            // TransactResult {
            //     submit_result: (),
            //     logs,
            // }
            todo!()
        } else {
            // TODO: implement error `from` convert
            let err = exec_result.unwrap_err();
            aurora_engine_sdk::log(&aurora_engine_types::format!("{err:#?}"));
            unreachable!("NOT")
        }
    }

    fn view(&mut self) -> TransactExecutionResult<TransactionStatus> {
        todo!()
    }
}
