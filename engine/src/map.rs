use aurora_engine_sdk::io::{IO, StorageIntermediate};
use aurora_engine_types::ToBytes;
use aurora_engine_types::storage::KeyPrefix;

pub use crate::prelude::{Cow, PhantomData, Vec, bytes_to_key};

/// A map storing a 1:1 relation between elements of types L and R.
/// The map is backed by storage of type I.
pub struct BijectionMap<L, R, I> {
    left_prefix: KeyPrefix,
    right_prefix: KeyPrefix,
    io: I,
    left_phantom: PhantomData<L>,
    right_phantom: PhantomData<R>,
}

impl<L: ToBytes + TryFrom<Vec<u8>>, R: ToBytes + TryFrom<Vec<u8>>, I: IO> BijectionMap<L, R, I> {
    pub const fn new(left_prefix: KeyPrefix, right_prefix: KeyPrefix, io: I) -> Self {
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
        self.io.write_storage(&key, Self::to_bytes(right).as_ref());

        let key = self.right_key(right);
        self.io.write_storage(&key, Self::to_bytes(left).as_ref());
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
        bytes_to_key(self.left_prefix, Self::to_bytes(left))
    }

    fn right_key(&self, right: &R) -> Vec<u8> {
        bytes_to_key(self.right_prefix, Self::to_bytes(right))
    }

    fn to_bytes<T: ToBytes>(value: &T) -> Cow<'_, [u8]> {
        value.to_bytes().expect("Failed to serialize value")
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
        use std::cell::RefCell;

        let storage = RefCell::new(Storage::default());
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
        assert_eq!(
            storage
                .read_storage(&bytes_to_key(left_prefix, b"aurora"))
                .unwrap()
                .to_vec(),
            erc20_token.as_bytes(),
        );
        assert_eq!(
            storage
                .read_storage(&bytes_to_key(right_prefix, erc20_token.as_bytes()))
                .unwrap()
                .to_vec(),
            b"aurora",
        );
    }

    #[test]
    fn test_nep245_map_finds_composite_token_pair() {
        use crate::engine::{ERC20Address, nep245_erc20_map};
        use aurora_engine_test_doubles::io::{Storage, StoragePointer};
        use aurora_engine_types::parameters::connector::Nep245Token;
        use aurora_engine_types::types::make_address;
        use std::cell::RefCell;

        let storage = RefCell::new(Storage::default());
        let mut map = nep245_erc20_map(StoragePointer(&storage));
        let tokens = [
            ("first.near", "coin", make_address(1, 1)),
            ("first.near", "another", make_address(2, 2)),
            ("second.near", "coin", make_address(3, 3)),
        ];

        for (account_id, token_id, address) in tokens {
            let token = Nep245Token {
                account_id: account_id.parse().unwrap(),
                token_id: token_id.into(),
            };
            map.insert(&token, &ERC20Address(address));
        }

        for (account_id, token_id, address) in tokens {
            let token = Nep245Token {
                account_id: account_id.parse().unwrap(),
                token_id: token_id.into(),
            };
            assert_eq!(map.lookup_left(&token).unwrap().0, address);
            assert_eq!(map.lookup_right(&ERC20Address(address)).unwrap(), token);
        }
    }
}
