use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::parameters::silo::{
    WhitelistKind, WhitelistKindArgs, WhitelistStatusArgs,
};
use aurora_engine_types::storage::{bytes_to_key, KeyPrefix};
use aurora_engine_types::AsBytes;

use crate::prelude::Vec;

const STATUS: &[u8] = b"LIST_STATUS";

/// `Whitelist` for checking access before interacting with the Aurora EVM.
/// * io - I/O trait handler
pub struct Whitelist<I> {
    io: I,
    kind: WhitelistKind,
}

impl<I> Whitelist<I>
where
    I: IO + Copy,
{
    /// Init a new whitelist of `WhitelistKind`.
    pub const fn init(io: &I, kind: WhitelistKind) -> Self {
        Self { io: *io, kind }
    }

    /// Enable a whitelist. (A whitelist is disabled after creation).
    pub fn enable(&mut self) {
        let key = self.key(STATUS);
        self.io.write_storage(&key, &[1]);
    }

    /// Disable a whitelist.
    pub fn disable(&mut self) {
        let key = self.key(STATUS);
        self.io.write_storage(&key, &[0]);
    }

    /// Check if the whitelist is enabled.
    pub fn is_enabled(&self) -> bool {
        // White list is disabled by default. So return `false` if the key doesn't exist.
        let key = self.key(STATUS);
        self.io
            .read_storage(&key)
            .map_or(false, |value| value.to_vec() == [1])
    }

    fn key(&self, value: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + value.len());

        bytes.push(u8::from(self.kind));
        bytes.extend_from_slice(value);
        bytes_to_key(KeyPrefix::Whitelist, &bytes)
    }

    /// Add a new element to the whitelist.
    pub fn add<A: AsBytes + ?Sized>(&mut self, element: &A) {
        let key = self.key(element.as_bytes());
        self.io.write_storage(&key, &[]);
    }

    /// Remove a new element from the whitelist.
    pub fn remove<A: AsBytes + ?Sized>(&mut self, element: &A) {
        let key = self.key(element.as_bytes());
        self.io.remove_storage(&key);
    }

    /// Check if the element is present in the whitelist.
    pub fn is_exist<A: AsBytes + ?Sized>(&self, element: &A) -> bool {
        let key = self.key(element.as_bytes());
        self.io.storage_has_key(&key)
    }
}

/// Set status of the whitelist.
pub fn set_whitelist_status<I: IO + Copy>(io: &I, args: &WhitelistStatusArgs) {
    let mut list = Whitelist::init(io, args.kind);

    if args.active {
        list.enable();
    } else {
        list.disable();
    }
}

/// Get status of the whitelist.
pub fn get_whitelist_status<I: IO + Copy>(io: &I, args: &WhitelistKindArgs) -> WhitelistStatusArgs {
    WhitelistStatusArgs {
        kind: args.kind,
        active: Whitelist::init(io, args.kind).is_enabled(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Whitelist, WhitelistKind};
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use aurora_engine_types::account_id::AccountId;
    use aurora_engine_types::types::Address;
    use std::cell::RefCell;

    #[test]
    fn test_init_white_list() {
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut white_list = Whitelist::init(&io, WhitelistKind::Admin);
        let account: AccountId = "aurora".parse().unwrap();
        let address = Address::zero();

        white_list.add(&account);
        assert!(white_list.is_exist(&account));
        white_list.remove(&account);
        assert!(!white_list.is_exist(&account));

        let mut white_list = Whitelist::init(&io, WhitelistKind::Account);
        white_list.add(&account);
        assert!(white_list.is_exist(&account));
        white_list.remove(&account);
        assert!(!white_list.is_exist(&account));

        let mut white_list = Whitelist::init(&io, WhitelistKind::EvmAdmin);

        white_list.add(&address);
        assert!(white_list.is_exist(&address));
        white_list.remove(&address);
        assert!(!white_list.is_exist(&address));

        let mut white_list = Whitelist::init(&io, WhitelistKind::Address);

        white_list.add(&address);
        assert!(white_list.is_exist(&address));
        white_list.remove(&address);
        assert!(!white_list.is_exist(&address));
    }

    #[test]
    fn test_disable_whitelist() {
        let storage = RefCell::new(Storage::default());
        let io = StoragePointer(&storage);
        let mut white_list = Whitelist::init(&io, WhitelistKind::Account);
        // Whitelist is enabled after creation.
        assert!(!white_list.is_enabled());
        white_list.enable();
        assert!(white_list.is_enabled());
    }
}
