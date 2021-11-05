pub use crate::prelude::{bytes_to_key, PhantomData, TryFrom, TryInto, Vec};
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
