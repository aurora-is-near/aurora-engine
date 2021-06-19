#[cfg(not(feature = "std"))]
pub use alloc::{
    borrow::ToOwned,
    borrow::{Cow, Cow::*},
    boxed::Box,
    collections::BTreeMap as HashMap,
    collections::BTreeMap,
    fmt, format, str,
    string::String,
    string::ToString,
    vec,
    vec::Vec,
};
#[cfg(not(feature = "std"))]
pub use core::{
    cmp::Ordering,
    convert::TryFrom,
    convert::TryInto,
    marker::PhantomData,
    mem,
    ops::{Add, Sub},
};
#[cfg(feature = "std")]
pub use std::{
    borrow::Cow,
    borrow::Cow::Borrowed,
    borrow::ToOwned,
    boxed::Box,
    cmp::Ordering,
    collections::BTreeMap,
    collections::HashMap,
    convert::TryFrom,
    convert::TryInto,
    error::Error,
    fmt, format,
    marker::PhantomData,
    mem,
    ops::{Add, Sub},
    str,
    string::String,
    string::ToString,
    vec,
    vec::Vec,
};

pub use primitive_types::{H160, H256, U256};

/// See: https://ethereum-magicians.org/t/increasing-address-size-from-20-to-32-bytes/5485
pub type Address = H160;

#[allow(non_snake_case, dead_code)]
pub fn Address(input: [u8; 20]) -> Address {
    H160(input)
}

/// The minimum length of a valid account ID.
const MIN_ACCOUNT_ID_LEN: u64 = 2;
/// The maximum length of a valid account ID.
const MAX_ACCOUNT_ID_LEN: u64 = 64;

/// Returns `true` if the given account ID is valid and `false` otherwise.
///
/// Taken from near-sdk-rs:
/// (https://github.com/near/near-sdk-rs/blob/42f62384c3acd024829501ee86e480917da03896/near-sdk/src/environment/env.rs#L816-L843)
pub fn is_valid_account_id(account_id: &[u8]) -> bool {
    if (account_id.len() as u64) < MIN_ACCOUNT_ID_LEN
        || (account_id.len() as u64) > MAX_ACCOUNT_ID_LEN
    {
        return false;
    }

    // NOTE: We don't want to use Regex here, because it requires extra time to compile it.
    // The valid account ID regex is /^(([a-z\d]+[-_])*[a-z\d]+\.)*([a-z\d]+[-_])*[a-z\d]+$/
    // Instead the implementation is based on the previous character checks.

    // We can safely assume that last char was a separator.
    let mut last_char_is_separator = true;

    for c in account_id {
        let current_char_is_separator = match *c {
            b'a'..=b'z' | b'0'..=b'9' => false,
            b'-' | b'_' | b'.' => true,
            _ => return false,
        };
        if current_char_is_separator && last_char_is_separator {
            return false;
        }
        last_char_is_separator = current_char_is_separator;
    }
    // The account can't end as separator.
    !last_char_is_separator
}
