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
pub mod identity;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod modexp;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod native;
#[cfg(feature = "precompiles-sputnikvm")]
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
#[cfg(feature = "precompiles-sputnikvm")]
mod utils;
#[cfg(feature = "precompiles-sputnikvm")]
pub mod xcc;
use aurora_engine_types::{Borrowed, Cow, Vec};
use core::num::TryFromIntError;
#[cfg(feature = "precompiles-revm")]
pub use revm::*;
#[cfg(feature = "precompiles-sputnikvm")]
pub use sputnikvm::*;

pub type PrecompileResult = Result<(u64, Vec<u8>), PrecompileError>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PrecompileError {
    /// out of gas is the main error. Others are here just for completeness
    OutOfGas,
    // Blake2 errors
    Blake2WrongLength,
    Blake2WrongFinalIndicatorFlag,
    // Modexp errors
    ModexpExpOverflow,
    ModexpBaseOverflow,
    ModexpModOverflow,
    // Bn128 errors
    Bn128FieldPointNotAMember,
    Bn128AffineGFailedToCreate,
    Bn128PairLength,
    // Blob errors
    /// The input length is not exactly 192 bytes.
    BlobInvalidInputLength,
    /// The commitment does not match the versioned hash.
    BlobMismatchedVersion,
    /// The proof verification failed.
    BlobVerifyKzgProofFailed,
    /// Catch-all variant for other errors.
    Other(Cow<'static, str>),
}

//===========================
// Utils

#[must_use]
pub const fn calc_linear_cost_u32(len: u64, base: u64, word: u64) -> u64 {
    (len + 32 - 1) / 32 * word + base
}

#[must_use]
pub const fn err_usize_conv(_e: TryFromIntError) -> PrecompileError {
    PrecompileError::Other(Borrowed("ERR_USIZE_CONVERSION"))
}
