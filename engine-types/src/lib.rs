#![feature(array_methods)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![cfg_attr(feature = "log", feature(panic_info_message))]

pub mod account_id;
pub mod parameters;
pub mod storage;
pub mod types;

mod v0 {
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    extern crate core;

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
    pub use primitive_types::{H160, H256, U256};
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
}

pub use v0::*;

/// See: https://ethereum-magicians.org/t/increasing-address-size-from-20-to-32-bytes/5485
pub type Address = H160;

#[allow(non_snake_case, dead_code)]
// Gets around the fact that you can't contract pub fields with types.
pub const fn Address(input: [u8; 20]) -> Address {
    H160(input)
}
