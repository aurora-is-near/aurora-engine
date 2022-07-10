use crate::prelude::BTreeMap;
use aurora_engine_precompiles::account_ids::{predecessor_account, CurrentAccount};
use aurora_engine_precompiles::blake2::Blake2F;
use aurora_engine_precompiles::bn128::{Bn128Add, Bn128Mul, Bn128Pair};
use aurora_engine_precompiles::hash::{RIPEMD160, SHA256};
use aurora_engine_precompiles::identity::Identity;
use aurora_engine_precompiles::modexp::ModExp;
use aurora_engine_precompiles::native::{exit_to_ethereum, exit_to_near};
use aurora_engine_precompiles::random::RandomSeed;
use aurora_engine_precompiles::secp256k1::ECRecover;
use aurora_engine_precompiles::{prepaid_gas, Byzantium};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::types::Address;
use bitflags::bitflags;
use borsh::{BorshDeserialize, BorshSerialize};

bitflags! {
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct PermissionFlags: u32 {
        /// Grants the ability to pause precompiles, resuming requires an owner.
        const PAUSE_PRECOMPILES = 0b1;
    }
}

bitflags! {
    #[derive(BorshSerialize, BorshDeserialize, Default)]
    pub struct PauseFlags: u32 {
        const SECP256K1_ECRECOVER = 0b1;
        const HASH_SHA256 = 0b10;
        const HASH_RIPEMD160 = 0b100;
        const IDENTITY_IDENTITY = 0b1000;
        const MODEXP = 0b10000;
        const BN128_ADD = 0b100000;
        const BN128_MUL = 0b1000000;
        const BN128_PAIR = 0b10000000;
        const BLAKE2_BLAKE2F = 0b100000000;
        const RANDOM_SEED = 0b1000000000;
        const CURRENT_ACCOUNT = 0b10000000000;
        const PREDECESSOR_ACCOUNT = 0b100000000000;
        const EXIT_TO_ETHEREUM = 0b1000000000000;
        const EXIT_TO_NEAR = 0b10000000000000;
        const PREPAID_GAS = 0b100000000000000;
    }
}

impl PauseFlags {
    pub fn is_not_paused_by_address(&self, address: &Address) -> bool {
        let precompile_flag = if address == &ECRecover::ADDRESS {
            PauseFlags::SECP256K1_ECRECOVER
        } else if address == &SHA256::ADDRESS {
            PauseFlags::HASH_SHA256
        } else if address == &RIPEMD160::ADDRESS {
            PauseFlags::HASH_RIPEMD160
        } else if address == &Identity::ADDRESS {
            PauseFlags::IDENTITY_IDENTITY
        } else if address == &ModExp::<Byzantium>::ADDRESS {
            PauseFlags::MODEXP
        } else if address == &Bn128Add::<Byzantium>::ADDRESS {
            PauseFlags::BN128_ADD
        } else if address == &Bn128Mul::<Byzantium>::ADDRESS {
            PauseFlags::BN128_MUL
        } else if address == &Bn128Pair::<Byzantium>::ADDRESS {
            PauseFlags::BN128_PAIR
        } else if address == &Blake2F::ADDRESS {
            PauseFlags::BLAKE2_BLAKE2F
        } else if address == &RandomSeed::ADDRESS {
            PauseFlags::RANDOM_SEED
        } else if address == &CurrentAccount::ADDRESS {
            PauseFlags::CURRENT_ACCOUNT
        } else if address == &predecessor_account::ADDRESS {
            PauseFlags::PREDECESSOR_ACCOUNT
        } else if address == &exit_to_ethereum::ADDRESS {
            PauseFlags::EXIT_TO_ETHEREUM
        } else if address == &exit_to_near::ADDRESS {
            PauseFlags::EXIT_TO_NEAR
        } else if address == &prepaid_gas::ADDRESS {
            PauseFlags::PREPAID_GAS
        } else {
            return true;
        };

        !self.contains(precompile_flag)
    }
}

pub trait Authorizer {
    /// Checks if the `account` is has every `permission`.
    fn is_authorized(&self, account: &AccountId, permissions: PermissionFlags) -> bool;
}

pub trait PermissionKeeper {
    fn grant_permissions(&mut self, account: AccountId, permissions_to_grant: u32);

    fn revoke_permissions(&mut self, account: &AccountId, permissions_to_revoke: u32);
}

pub trait Pauser {
    /// Checks if all of the `precompiles` are paused.
    fn is_paused(&self, precompiles: PauseFlags) -> bool;
}

pub trait PausedPrecompilesKeeper {
    fn resume_precompiles(&mut self, precompiles_to_resume: u32);

    fn pause_precompiles(&mut self, precompiles_to_pause: u32);
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Default, Clone)]
pub struct EngineAuthorizer {
    /// Permissions for certain actions are authenticated as [AccountId] and authorized by [PermissionMask].
    pub acl: BTreeMap<AccountId, PermissionFlags>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Default, Clone)]
pub struct EnginePrecompilesPauser {
    /// Determines which precompiled are currently paused, where off bit means running and on bit means paused.
    pub precompiles_pause_flags: PauseFlags,
}

impl Authorizer for EngineAuthorizer {
    fn is_authorized(&self, account: &AccountId, permissions: PermissionFlags) -> bool {
        self.acl
            .get(account)
            .map(|v| v.contains(permissions))
            .unwrap_or(false)
    }
}

impl PermissionKeeper for EngineAuthorizer {
    fn grant_permissions(&mut self, account: AccountId, permissions_to_grant: u32) {
        self.acl
            .entry(account)
            .or_insert(PermissionFlags::empty())
            .insert(PermissionFlags::from_bits_truncate(permissions_to_grant))
    }

    fn revoke_permissions(&mut self, account: &AccountId, permissions_to_revoke: u32) {
        if let Some(permissions) = self.acl.get_mut(account) {
            permissions.remove(PermissionFlags::from_bits_truncate(permissions_to_revoke));
        }
    }
}

impl Pauser for EnginePrecompilesPauser {
    fn is_paused(&self, precompiles: PauseFlags) -> bool {
        self.precompiles_pause_flags.contains(precompiles)
    }
}

impl PausedPrecompilesKeeper for EnginePrecompilesPauser {
    fn resume_precompiles(&mut self, precompiles_to_resume: u32) {
        self.precompiles_pause_flags
            .remove(PauseFlags::from_bits_truncate(precompiles_to_resume));
    }

    fn pause_precompiles(&mut self, precompiles_to_pause: u32) {
        self.precompiles_pause_flags
            .insert(PauseFlags::from_bits_truncate(precompiles_to_pause));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(PauseFlags::SECP256K1_ECRECOVER, ECRecover::ADDRESS)]
    #[test_case(PauseFlags::HASH_SHA256, SHA256::ADDRESS)]
    #[test_case(PauseFlags::HASH_RIPEMD160, RIPEMD160::ADDRESS)]
    #[test_case(PauseFlags::IDENTITY_IDENTITY, Identity::ADDRESS)]
    #[test_case(PauseFlags::MODEXP, ModExp::<Byzantium>::ADDRESS)]
    #[test_case(PauseFlags::BN128_ADD, Bn128Add::<Byzantium>::ADDRESS)]
    #[test_case(PauseFlags::BN128_MUL, Bn128Mul::<Byzantium>::ADDRESS)]
    #[test_case(PauseFlags::BN128_PAIR, Bn128Pair::<Byzantium>::ADDRESS)]
    #[test_case(PauseFlags::BLAKE2_BLAKE2F, Blake2F::ADDRESS)]
    #[test_case(PauseFlags::RANDOM_SEED, RandomSeed::ADDRESS)]
    #[test_case(PauseFlags::CURRENT_ACCOUNT, CurrentAccount::ADDRESS)]
    #[test_case(PauseFlags::PREDECESSOR_ACCOUNT, predecessor_account::ADDRESS)]
    #[test_case(PauseFlags::EXIT_TO_ETHEREUM, exit_to_ethereum::ADDRESS)]
    #[test_case(PauseFlags::EXIT_TO_NEAR, exit_to_near::ADDRESS)]
    #[test_case(PauseFlags::PREPAID_GAS, prepaid_gas::ADDRESS)]
    fn test_paused_flag_marks_precompiles_address_as_paused(flags: PauseFlags, address: Address) {
        assert!(!flags.is_not_paused_by_address(&address));
    }

    #[test]
    fn test_unknown_precompile_address_is_not_marked_as_paused() {
        let flags = PauseFlags::all();
        let address = Address::zero();
        assert!(flags.is_not_paused_by_address(&address));
    }

    #[test]
    fn test_pausing_precompile_marks_it_as_paused() {
        let flags = PauseFlags::EXIT_TO_NEAR;
        let mut pauser = EnginePrecompilesPauser {
            precompiles_pause_flags: PauseFlags::empty(),
        };

        assert!(!pauser.is_paused(flags));
        pauser.pause_precompiles(flags.bits);
        assert!(pauser.is_paused(flags));
    }

    #[test]
    fn test_resume_precompile_removes_its_mark_as_paused() {
        let flags = PauseFlags::EXIT_TO_NEAR;
        let mut pauser = EnginePrecompilesPauser {
            precompiles_pause_flags: PauseFlags::empty(),
        };
        pauser.pause_precompiles(flags.bits);

        assert!(pauser.is_paused(flags));
        pauser.resume_precompiles(flags.bits);
        assert!(!pauser.is_paused(flags));
    }

    #[test]
    fn test_granting_permission_to_account_authorizes_it() {
        let account = AccountId::default();
        let flags = PermissionFlags::PAUSE_PRECOMPILES;
        let mut authorizer = EngineAuthorizer {
            acl: BTreeMap::new(),
        };

        authorizer.grant_permissions(account.clone(), flags.bits);
        assert!(authorizer.is_authorized(&account, flags));
    }

    #[test]
    fn test_revoking_permission_from_account_authorizes_it() {
        let account = AccountId::default();
        let flags = PermissionFlags::PAUSE_PRECOMPILES;
        let mut authorizer = EngineAuthorizer {
            acl: BTreeMap::new(),
        };
        authorizer.grant_permissions(account.clone(), flags.bits);

        assert!(authorizer.is_authorized(&account, flags));
        authorizer.revoke_permissions(&account, flags.bits);
        assert!(!authorizer.is_authorized(&account, flags));
    }
}
