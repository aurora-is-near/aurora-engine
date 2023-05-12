use aurora_engine_sdk::io::IO;
#[cfg(feature = "contract")]
use aurora_engine_sdk::{env::Env, types::SdkUnwrap};
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::AsBytes;

#[cfg(feature = "contract")]
use crate::engine::EngineErrorKind;
use crate::prelude::Vec;

use parameters::{WhitelistArgs, WhitelistKindArgs, WhitelistStatusArgs};
use whitelist::Whitelist;
pub use whitelist::WhitelistKind;

pub mod parameters;
mod whitelist;

const GAS_COST_KEY: &[u8] = b"GAS_COST_KEY";

/// Return fixed gas cost.
pub fn get_fixed_gas_cost<I: IO>(io: &I) -> Option<Wei> {
    let key = fixed_gas_cost_key();
    io.read_u256(&key).ok().map(Wei::new)
}

/// Set fixed gas cost.
pub fn set_fixed_gas_cost<I: IO>(io: &mut I, cost: Option<Wei>) {
    let key = fixed_gas_cost_key();

    if let Some(cost) = cost {
        io.write_storage(&key, &cost.to_bytes());
    } else {
        io.remove_storage(&key);
    }
}

/// Add an entry to a white list depending on a kind of list types in provided arguments.
pub fn add_entity_to_whitelist<I: IO + Copy>(io: &I, args: &WhitelistArgs) {
    let (kind, entry) = get_kind_and_entry(args);
    Whitelist::init(io, kind).add(entry);
}

/// Add an entries to a white list depending on a kind of list types in provided arguments.
pub fn add_entry_to_whitelist_batch<I: IO + Copy, A: IntoIterator<Item = WhitelistArgs>>(
    io: &I,
    entries: A,
) {
    for entry in entries {
        add_entity_to_whitelist(io, &entry);
    }
}

/// Remove an entries to a white list depending on a kind of list types in provided arguments.
pub fn remove_entry_from_whitelist<I: IO + Copy>(io: &I, args: &WhitelistArgs) {
    let (kind, entry) = get_kind_and_entry(args);
    Whitelist::init(io, kind).remove(entry);
}

/// Set status of the provided white list.
pub fn set_whitelist_status<I: IO + Copy>(io: &I, args: &WhitelistStatusArgs) {
    whitelist::set_whitelist_status(io, args);
}

/// Return status of the provided white list.
pub fn get_whitelist_status<I: IO + Copy>(io: &I, args: &WhitelistKindArgs) -> WhitelistStatusArgs {
    whitelist::get_whitelist_status(io, args)
}

/// Check if the calling user is admin or owner.
#[cfg(feature = "contract")]
pub fn assert_admin<I: IO + Env + Copy>(io: &I) -> Result<(), EngineErrorKind> {
    if !is_owner(io) && !is_admin(io) {
        return Err(EngineErrorKind::NotAllowed);
    }

    Ok(())
}

/// Check if user has writes to deploy EVM code.
pub fn is_allow_deploy<I: IO + Copy>(io: &I, account: &AccountId, address: &Address) -> bool {
    let admin_list = Whitelist::init(io, WhitelistKind::Admin);
    let evm_admin_list = Whitelist::init(io, WhitelistKind::EvmAdmin);

    (!admin_list.is_enabled() || admin_list.is_exist(account))
        && (!evm_admin_list.is_enabled() || evm_admin_list.is_exist(address))
}

/// Check if user has writes to submit transaction.
pub fn is_allow_submit<I: IO + Copy>(io: &I, account: &AccountId, address: &Address) -> bool {
    is_address_allowed(io, address) && is_account_allowed(io, account)
}

#[cfg(feature = "contract")]
fn is_admin<I: IO + Env + Copy>(io: &I) -> bool {
    let list = Whitelist::init(io, WhitelistKind::Admin);
    !list.is_enabled() || list.is_exist(&io.predecessor_account_id())
}

#[cfg(feature = "contract")]
fn is_owner<I: IO + Env + Copy>(io: &I) -> bool {
    let state = crate::state::get_state(io).sdk_unwrap();
    state.owner_id == io.predecessor_account_id()
}

fn is_address_allowed<I: IO + Copy>(io: &I, address: &Address) -> bool {
    let list = Whitelist::init(io, WhitelistKind::Address);
    !list.is_enabled() || list.is_exist(address)
}

fn is_account_allowed<I: IO + Copy>(io: &I, account: &AccountId) -> bool {
    let list = Whitelist::init(io, WhitelistKind::Account);
    !list.is_enabled() || list.is_exist(account)
}

fn fixed_gas_cost_key() -> Vec<u8> {
    bytes_to_key(KeyPrefix::FixedGasCost, GAS_COST_KEY)
}

fn get_kind_and_entry(args: &WhitelistArgs) -> (WhitelistKind, &dyn AsBytes) {
    match args {
        WhitelistArgs::WhitelistAddressArgs(args) => (args.kind, &args.address),
        WhitelistArgs::WhitelistAccountArgs(args) => (args.kind, &args.account_id),
    }
}

#[cfg(test)]
mod access_test {
    use super::*;
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use std::cell::RefCell;

    #[test]
    fn test_set_fixed_gas_cost() {
        let cost = Some(Wei::new_u64(1000));
        let storage = RefCell::new(Storage::default());
        let mut io = StoragePointer(&storage);

        assert_eq!(get_fixed_gas_cost(&io), None);
        set_fixed_gas_cost(&mut io, cost);
        assert_eq!(get_fixed_gas_cost(&io), cost);
    }

    #[test]
    fn test_adding_entry_to_whitelist() {
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let account_id = "some-account.near".parse().unwrap();
        let address = Address::zero();
        let mut list = Whitelist::init(&io, WhitelistKind::Account);

        assert!(!is_account_allowed(&io, &account_id));
        list.add(&account_id);
        assert!(is_account_allowed(&io, &account_id));

        let mut list = Whitelist::init(&io, WhitelistKind::Address);
        assert!(!is_address_allowed(&io, &address));
        list.add(&address);
        assert!(is_address_allowed(&io, &address));

        assert!(is_allow_submit(&io, &account_id, &address));
    }

    #[test]
    fn test_check_set_whitelist_status() {
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);

        let status = get_whitelist_status(
            &io,
            &WhitelistKindArgs {
                kind: WhitelistKind::Admin,
            },
        );

        assert!(status.active);

        set_whitelist_status(
            &io,
            &WhitelistStatusArgs {
                kind: WhitelistKind::Admin,
                active: false,
            },
        );

        let status = get_whitelist_status(
            &io,
            &WhitelistKindArgs {
                kind: WhitelistKind::Admin,
            },
        );

        assert!(!status.active);
    }
}
