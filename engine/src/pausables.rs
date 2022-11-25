use crate::prelude::{AccountId, Address, BTreeSet, Vec};
use aurora_engine_precompiles::native::{exit_to_ethereum, exit_to_near};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use bitflags::bitflags;
use borsh::{BorshDeserialize, BorshSerialize};

bitflags! {
    /// Wraps unsigned integer where each bit identifies a different precompile.
    #[derive(BorshSerialize, BorshDeserialize, Default)]
    pub struct PrecompileFlags: u32 {
        const EXIT_TO_NEAR        = 0b01;
        const EXIT_TO_ETHEREUM    = 0b10;
    }
}

impl PrecompileFlags {
    pub fn from_address(address: &Address) -> Option<Self> {
        Some(if address == &exit_to_ethereum::ADDRESS {
            PrecompileFlags::EXIT_TO_ETHEREUM
        } else if address == &exit_to_near::ADDRESS {
            PrecompileFlags::EXIT_TO_NEAR
        } else {
            return None;
        })
    }

    /// Checks if the precompile belonging to the `address` is marked as paused.
    pub fn is_paused_by_address(&self, address: &Address) -> bool {
        match Self::from_address(address) {
            Some(precompile_flag) => self.contains(precompile_flag),
            None => false,
        }
    }
}

/// Can check if given account has a permission to pause precompiles.
pub trait Authorizer {
    /// Checks if the `account` has the permission to pause precompiles.
    fn is_authorized(&self, account: &AccountId) -> bool;
}

/// Can check if a subset of precompiles is currently paused or not.
pub trait PausedPrecompilesChecker {
    /// Checks if all of the `precompiles` are paused.
    ///
    /// The `precompiles` mask can be a subset and every 1 bit is meant to be checked and every 0 bit is ignored.
    fn is_paused(&self, precompiles: PrecompileFlags) -> bool;

    /// Returns a set of all paused precompiles in a bitmask, where every 1 bit means paused and every 0 bit means
    /// the opposite.
    ///
    /// To determine which bit belongs to what precompile, you have to match it with appropriate constant, for example
    /// [`PrecompileFlags::EXIT_TO_NEAR`].
    ///
    /// # Example
    /// ```
    /// # use aurora_engine::pausables::{PausedPrecompilesChecker, PrecompileFlags};
    /// # fn check(checker: impl PausedPrecompilesChecker) {
    /// let flags = checker.paused();
    ///
    /// if flags.contains(PrecompileFlags::EXIT_TO_NEAR) {
    ///     println!("EXIT_TO_NEAR is paused!");
    /// }
    /// # }
    /// ```
    fn paused(&self) -> PrecompileFlags;
}

/// Responsible for resuming and pausing of precompiles.
pub trait PausedPrecompilesManager {
    /// Resumes all the given `precompiles_to_resume`.
    ///
    /// The `precompiles_to_resume` mask can be a subset and every 1 bit is meant to be resumed and every 0 bit is
    /// ignored.
    fn resume_precompiles(&mut self, precompiles_to_resume: PrecompileFlags);

    /// Pauses all the given precompiles.
    ///
    /// The `precompiles_to_pause` mask can be a subset and every 1 bit is meant to be paused and every 0 bit is
    /// ignored.
    fn pause_precompiles(&mut self, precompiles_to_pause: PrecompileFlags);
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Default, Clone)]
pub struct EngineAuthorizer {
    /// List of [AccountId]s with the permission to pause precompiles.
    pub acl: BTreeSet<AccountId>,
}

impl EngineAuthorizer {
    /// Creates new [EngineAuthorizer] and grants permission to pause precompiles for all given `accounts`.
    pub fn from_accounts(accounts: impl Iterator<Item = AccountId>) -> Self {
        Self {
            acl: accounts.collect(),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Default, Clone)]
pub struct EnginePrecompilesPauser<I: IO> {
    /// Storage to read pause flags from and write into.
    io: I,
}

impl<I: IO> EnginePrecompilesPauser<I> {
    /// Key for storing [PrecompileFlags].
    const PAUSE_FLAGS_KEY: &'static [u8; 11] = b"PAUSE_FLAGS";

    /// Creates new [EnginePrecompilesPauser] instance that reads from and writes into storage accessed using `io`.
    pub fn from_io(io: I) -> Self {
        Self { io }
    }

    fn read_flags_from_storage(&self) -> PrecompileFlags {
        match self.io.read_storage(&Self::storage_key()) {
            None => PrecompileFlags::empty(),
            Some(bytes) => {
                let int_length = core::mem::size_of::<u32>();
                let input = bytes.to_vec();

                if input.len() < int_length {
                    return PrecompileFlags::empty();
                }

                let (int_bytes, _) = input.split_at(int_length);
                PrecompileFlags::from_bits_truncate(u32::from_le_bytes(
                    int_bytes.try_into().unwrap(),
                ))
            }
        }
    }

    fn write_flags_into_storage(&mut self, pause_flags: PrecompileFlags) {
        self.io
            .write_storage(&Self::storage_key(), &pause_flags.bits().to_le_bytes());
    }

    fn storage_key() -> Vec<u8> {
        bytes_to_key(KeyPrefix::Config, Self::PAUSE_FLAGS_KEY)
    }
}

impl Authorizer for EngineAuthorizer {
    fn is_authorized(&self, account: &AccountId) -> bool {
        self.acl.get(account).is_some()
    }
}

impl<I: IO> PausedPrecompilesChecker for EnginePrecompilesPauser<I> {
    fn is_paused(&self, precompiles: PrecompileFlags) -> bool {
        self.read_flags_from_storage().contains(precompiles)
    }

    fn paused(&self) -> PrecompileFlags {
        self.read_flags_from_storage()
    }
}

impl<I: IO> PausedPrecompilesManager for EnginePrecompilesPauser<I> {
    fn resume_precompiles(&mut self, precompiles_to_resume: PrecompileFlags) {
        let mut pause_flags = self.read_flags_from_storage();
        pause_flags.remove(precompiles_to_resume);
        self.write_flags_into_storage(pause_flags);
    }

    fn pause_precompiles(&mut self, precompiles_to_pause: PrecompileFlags) {
        let mut pause_flags = self.read_flags_from_storage();
        pause_flags.insert(precompiles_to_pause);
        self.write_flags_into_storage(pause_flags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use std::iter::once;
    use std::sync::RwLock;
    use test_case::test_case;

    #[test_case(PrecompileFlags::EXIT_TO_ETHEREUM, exit_to_ethereum::ADDRESS)]
    #[test_case(PrecompileFlags::EXIT_TO_NEAR, exit_to_near::ADDRESS)]
    fn test_paused_flag_marks_precompiles_address_as_paused(
        flags: PrecompileFlags,
        address: Address,
    ) {
        assert!(flags.is_paused_by_address(&address));
    }

    #[test]
    fn test_unknown_precompile_address_is_not_marked_as_paused() {
        let flags = PrecompileFlags::all();
        let address = Address::zero();

        assert!(!flags.is_paused_by_address(&address));
    }

    #[test]
    fn test_pausing_precompile_marks_it_as_paused() {
        let storage = RwLock::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut pauser = EnginePrecompilesPauser::from_io(io);
        let flags = PrecompileFlags::EXIT_TO_NEAR;

        assert!(!pauser.is_paused(flags));
        pauser.pause_precompiles(flags);
        assert!(pauser.is_paused(flags));
    }

    #[test]
    fn test_resuming_precompile_removes_its_mark_as_paused() {
        let storage = RwLock::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut pauser = EnginePrecompilesPauser::from_io(io);
        let flags = PrecompileFlags::EXIT_TO_NEAR;
        pauser.pause_precompiles(flags);

        assert!(pauser.is_paused(flags));
        pauser.resume_precompiles(flags);
        assert!(!pauser.is_paused(flags));
    }

    #[test]
    fn test_granting_permission_to_account_authorizes_it() {
        let account = AccountId::default();
        let authorizer = EngineAuthorizer::from_accounts(once(account.clone()));

        assert!(authorizer.is_authorized(&account));
    }

    #[test]
    fn test_revoking_permission_from_account_unauthorizes_it() {
        let account = AccountId::default();
        let authorizer = EngineAuthorizer::default();

        assert!(!authorizer.is_authorized(&account));
    }

    #[test]
    fn test_no_precompile_is_paused_if_storage_contains_too_few_bytes() {
        let key = EnginePrecompilesPauser::<StoragePointer>::storage_key();
        let storage = RwLock::new(Storage::default());
        let mut io = StoragePointer(&storage);
        io.write_storage(key.as_slice(), &[7u8]);
        let pauser = EnginePrecompilesPauser::from_io(io);

        let expected_paused = PrecompileFlags::empty();
        let actual_paused = pauser.paused();
        assert_eq!(expected_paused, actual_paused);
    }
}
