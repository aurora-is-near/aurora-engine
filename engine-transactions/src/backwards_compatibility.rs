//! Warning: this module _incorrectly_ parses RLP-serialized Ethereum transactions.
//! This is intentional and needed for our "standalone engine" to be able to reproduce
//! the Aurora state on the NEAR blockchain before the time a bug was fixed. See
//! `https://github.com/aurora-is-near/aurora-engine/pull/458` for more details, but external
//! users of this library should _never_ use the adapter in this module.

use aurora_engine_types::types::Address;

use crate::{Error, EthTransactionKind};

/// This struct is a modification to the usual `EthTransactionKind` parsing logic.
/// For blocks strictly less than `bug_fix_height`, it still has the bug where the
/// zero address in the `to` field is converted to `None`. For blocks greater than
/// or equal to `bug_fix_height` it correctly parses the transaction.
pub struct EthTransactionKindAdapter {
    bug_fix_height: u64,
}

impl EthTransactionKindAdapter {
    #[must_use]
    pub const fn new(bug_fix_height: u64) -> Self {
        Self { bug_fix_height }
    }

    pub fn try_parse_bytes(
        &self,
        bytes: &[u8],
        block_height: u64,
    ) -> Result<EthTransactionKind, Error> {
        let mut transaction = EthTransactionKind::try_from(bytes)?;

        // Prior to the bug fix, the zero address was always parsed as None if
        // it was in the `to` field.
        if block_height < self.bug_fix_height {
            zero_address_fix(&mut transaction);
        }

        Ok(transaction)
    }
}

fn zero_address_fix(transaction: &mut EthTransactionKind) {
    const ZERO_ADDRESS: Address = Address::zero();

    let to = match transaction {
        EthTransactionKind::Legacy(tx) => &mut tx.transaction.to,
        EthTransactionKind::Eip1559(tx) => &mut tx.transaction.to,
        EthTransactionKind::Eip2930(tx) => &mut tx.transaction.to,
        EthTransactionKind::Eip7702(_) => {
            // The 'to' field is mandatory in the EIP-7702 transaction type.
            return;
        }
    };

    if *to == Some(ZERO_ADDRESS) {
        *to = None;
    }
}
