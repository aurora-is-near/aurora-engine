#![cfg_attr(not(feature = "std"), no_std)]
#![deny(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::similar_names,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::unreadable_literal
)]
extern crate alloc;

#[cfg(feature = "precompiles-sputnikvm")]
pub mod account_ids;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod alt_bn256;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod blake2;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod hash;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod identity;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod modexp;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod native;
mod prelude;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod prepaid_gas;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod promise_result;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod random;
#[cfg(feature = "precompiles-revm")]
mod revm;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod secp256k1;
#[cfg(feature = "precompiles-sputnikvm")]
mod sputnikvm;
pub mod types;
#[cfg(feature = "precompiles-sputnikvm")]
mod utils;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod xcc;

#[cfg(feature = "precompiles-revm")]
pub use revm::*;
#[cfg(feature = "precompiles-sputnikvm")]
pub use sputnikvm::*;
