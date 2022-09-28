pub use crate::prelude::{bytes_to_key, PhantomData, Vec};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage::KeyPrefix;

/// A map storing a 1:1 relation between elements of types L and R.
/// The map is backed by storage of type I.
pub struct BijectionMap<L, R, I> {
    left_prefix: KeyPrefix,
    right_prefix: KeyPrefix,
    io: I,
    left_phantom: PhantomData<L>,
    right_phantom: PhantomData<R>,
}

impl<L: AsRef<[u8]> + TryFrom<Vec<u8>>, R: AsRef<[u8]> + TryFrom<Vec<u8>>, I: IO>
    BijectionMap<L, R, I>
{
    pub fn new(left_prefix: KeyPrefix, right_prefix: KeyPrefix, io: I) -> Self {
        Self {
            left_prefix,
            right_prefix,
            io,
            left_phantom: PhantomData,
            right_phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, left: &L, right: &R) {
        let key = self.left_key(left);
        self.io.write_storage(&key, right.as_ref());

        let key = self.right_key(right);
        self.io.write_storage(&key, left.as_ref());
    }

    pub fn lookup_left(&self, left: &L) -> Option<R> {
        let key = self.left_key(left);
        self.io
            .read_storage(&key)
            .and_then(|v| v.to_vec().try_into().ok())
    }

    pub fn lookup_right(&self, right: &R) -> Option<L> {
        let key = self.right_key(right);
        self.io
            .read_storage(&key)
            .and_then(|v| v.to_vec().try_into().ok())
    }

    fn left_key(&self, left: &L) -> Vec<u8> {
        bytes_to_key(self.left_prefix, left.as_ref())
    }

    fn right_key(&self, right: &R) -> Vec<u8> {
        bytes_to_key(self.right_prefix, right.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_finds_inserted_pair() {
        use crate::engine::{ERC20Address, NEP141Account};
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::account_id::AccountId;
        use aurora_engine_types::types::Address;
        use std::sync::RwLock;

        let storage = RwLock::new(Storage::default());
        let storage = StoragePointer(&storage);
        let left_prefix = KeyPrefix::Nep141Erc20Map;
        let right_prefix = KeyPrefix::Erc20Nep141Map;

        let mut map: BijectionMap<NEP141Account, ERC20Address, _> =
            BijectionMap::new(left_prefix, right_prefix, storage);

        let erc20_token = Address::zero();
        let nep141_token = AccountId::new("aurora").unwrap();
        let expected_left = NEP141Account(nep141_token);
        let expected_right = ERC20Address(erc20_token);
        map.insert(&expected_left, &expected_right);

        let actual_right = map.lookup_left(&expected_left).unwrap();

        assert_eq!(expected_right.0, actual_right.0);

        let actual_left = map.lookup_right(&expected_right).unwrap();

        assert_eq!(expected_left.0, actual_left.0);
    }
}
