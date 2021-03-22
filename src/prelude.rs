#[cfg(not(feature = "std"))]
pub use alloc::{
    borrow::ToOwned, boxed::Box, collections::BTreeMap as HashMap, string::String,
    string::ToString, vec, vec::Vec,
};
#[cfg(feature = "std")]
pub use std::{
    borrow::ToOwned, boxed::Box, collections::HashMap, string::String, string::ToString, vec,
    vec::Vec,
};

pub use primitive_types::{H160, H256, U256};

/// See: https://ethereum-magicians.org/t/increasing-address-size-from-20-to-32-bytes/5485
pub type Address = H160;

#[allow(non_snake_case, dead_code)]
pub fn Address(input: [u8; 20]) -> Address {
    H160(input)
}
