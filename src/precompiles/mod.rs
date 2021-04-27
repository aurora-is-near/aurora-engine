mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
mod secp256k1;
mod connector_utils;

use crate::precompiles::blake2::Blake2F;
use crate::precompiles::bn128::{BN128Add, BN128Mul, BN128Pair};
use crate::precompiles::hash::{RIPEMD160, SHA256};
use crate::precompiles::identity::Identity;
use crate::precompiles::modexp::ModExp;
pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::precompiles::secp256k1::ECRecover;
use crate::precompiles::connector_utils::{ExitToNear, ExitToEthereum};
use crate::prelude::{Address, Vec};
use evm::{Context, ExitError, ExitSucceed};

/// A precompile operation result.
type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

/// A precompiled function for use in the EVM.
trait Precompile {
    /// The required gas in order to run the precompile function.
    fn required_gas(input: &[u8]) -> Result<u64, ExitError>;

    /// Runs the precompile function.
    fn run(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult;
}

/// Hard fork marker.
trait HardFork {}

/// Homestead hard fork marker.
struct Homestead;

/// Homestead hard fork marker.
struct Byzantium;

/// Homestead hard fork marker.
struct Istanbul;

/// Homestead hard fork marker.
struct Berlin;

impl HardFork for Homestead {}

impl HardFork for Byzantium {}

impl HardFork for Istanbul {}

impl HardFork for Berlin {}

/// No precompiles, returns `None`.
#[allow(dead_code)]
pub fn no_precompiles(
    _address: Address,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // no precompiles supported
}

/// Matches the address given to Homestead precompiles.
#[allow(dead_code)]
pub fn homestead_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::run(input, target_gas, context)),
        2 => Some(SHA256::run(input, target_gas, context)),
        3 => Some(RIPEMD160::run(input, target_gas, context)),
        // 4 => Some(identity::identity(input, target_gas)),
        _ => None,
    }
}

/// Matches the address given to Byzantium precompiles.
#[allow(dead_code)]
pub fn byzantium_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::run(input, target_gas, context)),
        2 => Some(SHA256::run(input, target_gas, context)),
        3 => Some(RIPEMD160::run(input, target_gas, context)),
        4 => Some(Identity::run(input, target_gas, context)),
        5 => Some(ModExp::<Byzantium>::run(input, target_gas, context)),
        6 => Some(BN128Add::<Byzantium>::run(input, target_gas, context)),
        7 => Some(BN128Mul::<Byzantium>::run(input, target_gas, context)),
        8 => Some(BN128Pair::<Byzantium>::run(input, target_gas, context)),
        _ => None,
    }
}

/// Matches the address given to Istanbul precompiles.
#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::run(input, target_gas, context)),
        2 => Some(SHA256::run(input, target_gas, context)),
        3 => Some(RIPEMD160::run(input, target_gas, context)),
        4 => Some(Identity::run(input, target_gas, context)),
        5 => Some(ModExp::<Byzantium>::run(input, target_gas, context)),
        6 => Some(BN128Add::<Istanbul>::run(input, target_gas, context)),
        7 => Some(BN128Mul::<Istanbul>::run(input, target_gas, context)),
        8 => Some(BN128Pair::<Istanbul>::run(input, target_gas, context)),
        9 => Some(Blake2F::run(input, target_gas, context)),
        // Not supported.
        _ => None,
    }
}

/// Matches the address given to Berlin precompiles.
#[allow(dead_code)]
pub fn berlin_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::run(input, target_gas, context)),
        2 => Some(SHA256::run(input, target_gas, context)),
        3 => Some(RIPEMD160::run(input, target_gas, context)),
        4 => Some(Identity::run(input, target_gas, context)),
        5 => Some(ModExp::<Berlin>::run(input, target_gas, context)), // TODO gas changes
        6 => Some(BN128Add::<Istanbul>::run(input, target_gas, context)),
        7 => Some(BN128Mul::<Istanbul>::run(input, target_gas, context)),
        8 => Some(BN128Pair::<Istanbul>::run(input, target_gas, context)),
        9 => Some(Blake2F::run(input, target_gas, context)),
        // Not supported.
        _ => None,
    }
}

/// Matches the address given to Near Connector precompiles.
#[allow(dead_code)]
pub fn near_connector_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
) -> Option<PrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(PrecompileResult::Err(ExitError::OutOfGas)),
    };

    match address.to_low_u64_be() {
        1 => Some(ECRecover::run(input, target_gas, context)),
        2 => Some(SHA256::run(input, target_gas, context)),
        3 => Some(RIPEMD160::run(input, target_gas, context)),
        4 => Some(Identity::run(input, target_gas, context)),
        5 => Some(ModExp::<Berlin>::run(input, target_gas, context)), // TODO gas changes
        6 => Some(BN128Add::<Istanbul>::run(input, target_gas, context)),
        7 => Some(BN128Mul::<Istanbul>::run(input, target_gas, context)),
        8 => Some(BN128Pair::<Istanbul>::run(input, target_gas, context)),
        9 => Some(Blake2F::run(input, target_gas, context)),
        // Near connector precompiles
        240 => Some(ExitToNear::run(input, target_gas, context)), // 0x0000..00F0
        241 => Some(ExitToEthereum::run(input, target_gas, context)), // 0x0000..00F1
        // Not supported.
        _ => None,
    }
}
