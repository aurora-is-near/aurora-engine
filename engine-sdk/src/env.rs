use crate::error::{OneYoctoAttachError, PrivateCallError};
use crate::prelude::{NearGas, H256};
use aurora_engine_types::account_id::AccountId;

pub const DEFAULT_PREPAID_GAS: NearGas = NearGas::new(300_000_000_000_000);

/// Timestamp represented by the number of nanoseconds since the Unix Epoch.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    pub fn new(ns: u64) -> Self {
        Self(ns)
    }

    pub fn nanos(&self) -> u64 {
        self.0
    }

    pub fn millis(&self) -> u64 {
        self.0 / 1_000_000
    }

    pub fn secs(&self) -> u64 {
        self.0 / 1_000_000_000
    }
}

/// Returns information about the NEAR context in which the
/// transaction is executing. In the case of a standalone binary,
/// independent of NEAR these values would need to be mocked or otherwise
/// passed in from an external source.
pub trait Env {
    /// Account ID that signed the transaction.
    fn signer_account_id(&self) -> AccountId;
    /// Account ID of the currently executing contract.
    fn current_account_id(&self) -> AccountId;
    /// Account ID which called the current contract.
    fn predecessor_account_id(&self) -> AccountId;
    /// Height of the current block.
    fn block_height(&self) -> u64;
    /// Timestamp (in ns) of the current block.
    fn block_timestamp(&self) -> Timestamp;
    /// Amount of NEAR attached to current call
    fn attached_deposit(&self) -> u128;
    /// Random seed generated for the current block
    fn random_seed(&self) -> H256;
    /// Prepaid NEAR Gas
    fn prepaid_gas(&self) -> NearGas;

    fn assert_private_call(&self) -> Result<(), PrivateCallError> {
        if self.predecessor_account_id() == self.current_account_id() {
            Ok(())
        } else {
            Err(PrivateCallError)
        }
    }

    fn assert_one_yocto(&self) -> Result<(), OneYoctoAttachError> {
        if self.attached_deposit() == 1 {
            Ok(())
        } else {
            Err(OneYoctoAttachError)
        }
    }
}

/// Fully in-memory implementation of the blockchain environment with
/// fixed values for all the fields.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub signer_account_id: AccountId,
    pub current_account_id: AccountId,
    pub predecessor_account_id: AccountId,
    pub block_height: u64,
    pub block_timestamp: Timestamp,
    pub attached_deposit: u128,
    pub random_seed: H256,
    pub prepaid_gas: NearGas,
}

impl Env for Fixed {
    fn signer_account_id(&self) -> AccountId {
        self.signer_account_id.clone()
    }

    fn current_account_id(&self) -> AccountId {
        self.current_account_id.clone()
    }

    fn predecessor_account_id(&self) -> AccountId {
        self.predecessor_account_id.clone()
    }

    fn block_height(&self) -> u64 {
        self.block_height
    }

    fn block_timestamp(&self) -> Timestamp {
        self.block_timestamp
    }

    fn attached_deposit(&self) -> u128 {
        self.attached_deposit
    }

    fn random_seed(&self) -> H256 {
        self.random_seed
    }

    fn prepaid_gas(&self) -> NearGas {
        self.prepaid_gas
    }
}
