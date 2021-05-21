mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
#[cfg(feature = "contract")]
mod native;
mod secp256k1;

use crate::precompiles::blake2::Blake2F;
use crate::precompiles::bn128::{BN128Add, BN128Mul, BN128Pair};
use crate::precompiles::hash::{RIPEMD160, SHA256};
use crate::precompiles::identity::Identity;
use crate::precompiles::modexp::ModExp;
#[cfg(feature = "contract")]
use crate::precompiles::native::{ExitToEthereum, ExitToNear};
pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::precompiles::secp256k1::ECRecover;
use crate::prelude::{Address, Vec};
use evm::executor::PrecompileOutput;
use evm::{Context, ExitError};

/// Exit to Ethereum precompile address (truncated to 8 bytes)
///
/// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
/// This address is computed as: `&keccak("exitToEthereum")[12..]`
const EXIT_TO_ETHEREUM_ID: u64 = 17176159495920586411;

fn exit_to_ethereum_address() -> Address {
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

fn exit_to_near_address() -> Address {
    Address::from_slice(
        hex::decode("e9217bc70b7ed1f598ddd3199e80b093fa71124f")
            .unwrap()
            .as_slice(),
    )
}

/// A precompile operation result.
type PrecompileResult = Result<PrecompileOutput, ExitError>;

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
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::run(input, target_gas, context)),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::run(input, target_gas, context)),
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
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::run(input, target_gas, context)),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::run(input, target_gas, context)),
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
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::run(input, target_gas, context)),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::run(input, target_gas, context)),
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
        #[cfg(feature = "contract")]
        EXIT_TO_NEAR_ID => Some(ExitToNear::run(input, target_gas, context)),
        #[cfg(feature = "contract")]
        EXIT_TO_ETHEREUM_ID => Some(ExitToEthereum::run(input, target_gas, context)),
        _ => None,
    }
}
