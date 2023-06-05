use crate::account_id::AccountId;
use crate::borsh::{self, BorshDeserialize, BorshSerialize};
use crate::types::Address;

#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct AddressVersionUpdateArgs {
    pub address: Address,
    pub version: CodeVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct FundXccArgs {
    pub target: Address,
    pub wnear_account_id: Option<AccountId>,
}

/// Type wrapper for version of router contracts.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, BorshDeserialize, BorshSerialize,
)]
pub struct CodeVersion(pub u32);

impl CodeVersion {
    #[must_use]
    pub const fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}
