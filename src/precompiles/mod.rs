mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
#[cfg(feature = "contract")]
mod native;
mod secp256k1;
use evm::executor::PrecompileOutput;
use evm::{Context, ExitError};

use crate::precompiles::blake2::Blake2F;
use crate::precompiles::bn128::{BN128Add, BN128Mul, BN128Pair};
use crate::precompiles::hash::{RIPEMD160, SHA256};
use crate::precompiles::identity::Identity;
use crate::precompiles::modexp::ModExp;
#[cfg(feature = "contract")]
use crate::precompiles::native::{ExitToEthereum, ExitToNear};
pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::precompiles::secp256k1::ECRecover;
use crate::prelude::Address;
#[cfg(feature = "contract")]
use crate::state::AuroraStackState;
use crate::AuroraState;

/// Exit to Ethereum precompile address (truncated to 8 bytes)
///
/// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
/// This address is computed as: `&keccak("exitToEthereum")[12..]`
const EXIT_TO_ETHEREUM_ID: u64 = 17176159495920586411;

pub fn exit_to_ethereum_address() -> Address {
    Address::from_slice(
        hex::decode("b0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab")
            .unwrap()
            .as_slice(),
    )
}

/// Exit to NEAR precompile address (truncated to 8 bytes)
///
/// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
/// This address is computed as: `&keccak("exitToNear")[12..]`
const EXIT_TO_NEAR_ID: u64 = 11421322804619973199;

pub fn exit_to_near_address() -> Address {
    Address::from_slice(
        hex::decode("e9217bc70b7ed1f598ddd3199e80b093fa71124f")
            .unwrap()
            .as_slice(),
    )
}

/// A precompile operation result.
pub type PrecompileResult = Result<PrecompileOutput, ExitError>;

/// A precompiled function for use in the EVM.
pub trait Precompile<S: AuroraState> {
    /// The required gas in order to run the precompile function.
    fn required_gas(input: &[u8]) -> Result<u64, ExitError>;

    /// Runs the precompile function.
    fn run(input: &[u8], target_gas: u64, context: &Context, state: &mut S) -> PrecompileResult;
}

/// Hard fork marker.
pub trait HardFork {}

/// Homestead hard fork marker.
pub struct Homestead;

/// Homestead hard fork marker.
pub struct Byzantium;

/// Homestead hard fork marker.
pub struct Istanbul;

/// Homestead hard fork marker.
pub struct Berlin;

impl HardFork for Homestead {}

impl HardFork for Byzantium {}

impl HardFork for Istanbul {}

impl HardFork for Berlin {}

/// No precompiles, returns `None`.
#[cfg(feature = "contract")]
#[allow(dead_code)]
pub fn no_precompiles(
    _address: Address,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
    _state: &mut AuroraStackState,
) -> Option<PrecompileResult> {
    None // no precompiles supported
}

/// Matches the address given to Homestead precompiles.
#[cfg(feature = "contract")]
#[allow(dead_code)]
pub fn homestead_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
    state: &mut AuroraStackState,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        2 => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        3 => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        _ => None,
    }
}

/// Matches the address given to Byzantium precompiles.
#[cfg(feature = "contract")]
#[allow(dead_code)]
pub fn byzantium_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
    state: &mut AuroraStackState,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        2 => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        3 => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        4 => Some(Identity::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        5 => Some(ModExp::<Byzantium, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        6 => Some(BN128Add::<Byzantium, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        7 => Some(BN128Mul::<Byzantium, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        8 => Some(BN128Pair::<Byzantium, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        _ => None,
    }
}

/// Matches the address given to Istanbul precompiles.
#[cfg(feature = "contract")]
#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
    state: &mut AuroraStackState,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        2 => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        3 => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        4 => Some(Identity::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        5 => Some(ModExp::<Byzantium, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        6 => Some(BN128Add::<Istanbul, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        7 => Some(BN128Mul::<Istanbul, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        8 => Some(BN128Pair::<Istanbul, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        9 => Some(Blake2F::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        _ => None,
    }
}

/// Matches the address given to Berlin precompiles.
#[cfg(feature = "contract")]
#[allow(dead_code)]
pub fn berlin_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
    state: &mut AuroraStackState,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        2 => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        3 => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        4 => Some(Identity::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        5 => Some(ModExp::<Berlin, AuroraStackState>::run(
            input, target_gas, context, state,
        )), // TODO gas changes
        6 => Some(BN128Add::<Istanbul, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        7 => Some(BN128Mul::<Istanbul, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        8 => Some(BN128Pair::<Istanbul, AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        9 => Some(Blake2F::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::<AuroraStackState>::run(
            input, target_gas, context, state,
        )),
        _ => None,
    }
}
