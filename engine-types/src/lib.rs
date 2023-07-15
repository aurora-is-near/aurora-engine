#![cfg_attr(not(any(feature = "std", feature = "contracts-std")), no_std)]
#![deny(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
)]

pub mod account_id;
pub mod parameters;
pub mod public_key;
pub mod storage;
pub mod types;

mod v0 {
    extern crate alloc;
    extern crate core;

    pub use alloc::{
        borrow::ToOwned,
        borrow::{Cow, Cow::*},
        boxed::Box,
        collections::BTreeMap as HashMap,
        collections::BTreeMap,
        collections::BTreeSet,
        fmt, format, str,
        string::String,
        string::ToString,
        vec,
        vec::Vec,
    };
    pub use core::{
        cmp::Ordering, fmt::Display, marker::PhantomData, mem, ops::Add, ops::AddAssign, ops::Div,
        ops::Mul, ops::Sub, ops::SubAssign,
    };
    pub use primitive_types::{H160, H256, U256};

    #[cfg(not(feature = "borsh-compat"))]
    pub use borsh;
    #[cfg(feature = "borsh-compat")]
    pub use borsh_compat::{self as borsh};
}

pub use v0::*;
