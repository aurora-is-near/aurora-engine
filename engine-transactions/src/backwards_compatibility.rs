//! Warning: this module _incorrectly_ parses RLP-serialized Ethereum transactions.
//! This is intentional and needed for our "standalone engine" to be able to reproduce
//! the Aurora state on the NEAR blockchain before the time a bug was fixed. See
//! https://github.com/aurora-is-near/aurora-engine/pull/458 for more details, but external
//! users of this library should _never_ use the adapter in this module.

use crate::{Error, EthTransactionKind};
use aurora_engine_types::{types::Address, H160};

const ZERO_ADDRESS: Option<Address> = Some(Address::new(H160::zero()));

/// This struct is a modification to the usual `EthTransactionKind` parsing logic.
/// For blocks strictly less than `bug_fix_height`, it still has the bug where the
/// zero address in the `to` field is converted to `None`. For blocks greater than
/// or equal to `bug_fix_height` it correctly parses the transaction.
pub struct EthTransactionKindAdapter {
    bug_fix_height: u64,
}

impl EthTransactionKindAdapter {
    pub const fn new(bug_fix_height: u64) -> Self {
        Self { bug_fix_height }
    }

    pub fn try_parse_bytes(
        &self,
        bytes: &[u8],
        block_height: u64,
    ) -> Result<EthTransactionKind, Error> {
        let mut result = EthTransactionKind::try_from(bytes)?;

        // Prior to the bug fix, the zero address was always parsed as None if
        // it was in the `to` field.
        if block_height < self.bug_fix_height {
            match &mut result {
                EthTransactionKind::Legacy(tx) => {
                    if tx.transaction.to == ZERO_ADDRESS {
                        tx.transaction.to = None;
                    }
                }
                EthTransactionKind::Eip1559(tx) => {
                    if tx.transaction.to == ZERO_ADDRESS {
                        tx.transaction.to = None;
                    }
                }
                EthTransactionKind::Eip2930(tx) => {
                    if tx.transaction.to == ZERO_ADDRESS {
                        tx.transaction.to = None;
                    }
                }
            }
        }

        Ok(result)
    }
}
