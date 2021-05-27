mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
mod native;
mod secp256k1;

use crate::precompiles::blake2::Blake2F;
use crate::precompiles::bn128::{BN128Add, BN128Mul, BN128Pair};
use crate::precompiles::hash::{RIPEMD160, SHA256};
use crate::precompiles::identity::Identity;
use crate::precompiles::modexp::ModExp;
use crate::precompiles::native::{ExitToEthereum, ExitToNear};
pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::precompiles::secp256k1::ECRecover;
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

    match address.0 {
        ECRecover::ADDRESS => Some(ECRecover::run(input, target_gas, context)),
        SHA256::ADDRESS => Some(SHA256::run(input, target_gas, context)),
        RIPEMD160::ADDRESS => Some(RIPEMD160::run(input, target_gas, context)),
        ExitToNear::ADDRESS => Some(ExitToNear::run(input, target_gas, context)),
        ExitToEthereum::ADDRESS => Some(ExitToEthereum::run(input, target_gas, context)),
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

    match address.0 {
        ECRecover::ADDRESS => Some(ECRecover::run(input, target_gas, context)),
        SHA256::ADDRESS => Some(SHA256::run(input, target_gas, context)),
        RIPEMD160::ADDRESS => Some(RIPEMD160::run(input, target_gas, context)),
        Identity::ADDRESS => Some(Identity::run(input, target_gas, context)),
        modexp::ADDRESS => Some(ModExp::<Byzantium>::run(input, target_gas, context)),
        bn128::addresses::ADD => Some(BN128Add::<Byzantium>::run(input, target_gas, context)),
        bn128::addresses::MUL => Some(BN128Mul::<Byzantium>::run(input, target_gas, context)),
        bn128::addresses::PAIR => Some(BN128Pair::<Byzantium>::run(input, target_gas, context)),
        ExitToNear::ADDRESS => Some(ExitToNear::run(input, target_gas, context)),
        ExitToEthereum::ADDRESS => Some(ExitToEthereum::run(input, target_gas, context)),
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

    match address.0 {
        ECRecover::ADDRESS => Some(ECRecover::run(input, target_gas, context)),
        SHA256::ADDRESS => Some(SHA256::run(input, target_gas, context)),
        RIPEMD160::ADDRESS => Some(RIPEMD160::run(input, target_gas, context)),
        Identity::ADDRESS => Some(Identity::run(input, target_gas, context)),
        modexp::ADDRESS => Some(ModExp::<Byzantium>::run(input, target_gas, context)),
        bn128::addresses::ADD => Some(BN128Add::<Istanbul>::run(input, target_gas, context)),
        bn128::addresses::MUL => Some(BN128Mul::<Istanbul>::run(input, target_gas, context)),
        bn128::addresses::PAIR => Some(BN128Pair::<Istanbul>::run(input, target_gas, context)),
        Blake2F::ADDRESS => Some(Blake2F::run(input, target_gas, context)),
        ExitToNear::ADDRESS => Some(ExitToNear::run(input, target_gas, context)),
        ExitToEthereum::ADDRESS => Some(ExitToEthereum::run(input, target_gas, context)),
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

    match address.0 {
        ECRecover::ADDRESS => Some(ECRecover::run(input, target_gas, context)),
        SHA256::ADDRESS => Some(SHA256::run(input, target_gas, context)),
        RIPEMD160::ADDRESS => Some(RIPEMD160::run(input, target_gas, context)),
        Identity::ADDRESS => Some(Identity::run(input, target_gas, context)),
        modexp::ADDRESS => Some(ModExp::<Berlin>::run(input, target_gas, context)), // TODO gas changes
        bn128::addresses::ADD => Some(BN128Add::<Istanbul>::run(input, target_gas, context)),
        bn128::addresses::MUL => Some(BN128Mul::<Istanbul>::run(input, target_gas, context)),
        bn128::addresses::PAIR => Some(BN128Pair::<Istanbul>::run(input, target_gas, context)),
        Blake2F::ADDRESS => Some(Blake2F::run(input, target_gas, context)),
        #[cfg(feature = "contract")]
        ExitToNear::ADDRESS => Some(ExitToNear::run(input, target_gas, context)),
        #[cfg(feature = "contract")]
        ExitToEthereum::ADDRESS => Some(ExitToEthereum::run(input, target_gas, context)),
        _ => None,
    }
}

/// const fn for making an address by concatenating the bytes from two given numbers,
/// Note that 32 + 128 = 160 = 20 bytes (the length of an address). This function is used
/// as a convenience for specifying the addresses of the various precompiles.
const fn make_address(x: u32, y: u128) -> [u8; 20] {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    [
        x_bytes[0],
        x_bytes[1],
        x_bytes[2],
        x_bytes[3],
        y_bytes[0],
        y_bytes[1],
        y_bytes[2],
        y_bytes[3],
        y_bytes[4],
        y_bytes[5],
        y_bytes[6],
        y_bytes[7],
        y_bytes[8],
        y_bytes[9],
        y_bytes[10],
        y_bytes[11],
        y_bytes[12],
        y_bytes[13],
        y_bytes[14],
        y_bytes[15],
    ]
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    #[test]
    fn test_precompile_addresses() {
        assert_eq!(super::secp256k1::ECRecover::ADDRESS, u8_to_address(1));
        assert_eq!(super::hash::SHA256::ADDRESS, u8_to_address(2));
        assert_eq!(super::hash::RIPEMD160::ADDRESS, u8_to_address(3));
        assert_eq!(super::identity::Identity::ADDRESS, u8_to_address(4));
        assert_eq!(super::modexp::ADDRESS, u8_to_address(5));
        assert_eq!(super::bn128::addresses::ADD, u8_to_address(6));
        assert_eq!(super::bn128::addresses::MUL, u8_to_address(7));
        assert_eq!(super::bn128::addresses::PAIR, u8_to_address(8));
        assert_eq!(super::blake2::Blake2F::ADDRESS, u8_to_address(9));
    }

    #[test]
    fn test_make_address() {
        for i in 0..u8::MAX {
            assert_eq!(super::make_address(0, i as u128), u8_to_address(i));
        }

        let mut rng = rand::thread_rng();
        for _ in 0..u8::MAX {
            let address: [u8; 20] = rng.gen();
            let (x, y) = split_address(address);
            assert_eq!(address, super::make_address(x, y))
        }
    }

    fn u8_to_address(x: u8) -> [u8; 20] {
        let mut bytes = [0u8; 20];
        bytes[19] = x;
        bytes
    }

    // Inverse function of `super::make_address`.
    fn split_address(a: [u8; 20]) -> (u32, u128) {
        let mut x_bytes = [0u8; 4];
        let mut y_bytes = [0u8; 16];

        x_bytes.copy_from_slice(&a[0..4]);
        y_bytes.copy_from_slice(&a[4..20]);

        (u32::from_be_bytes(x_bytes), u128::from_be_bytes(y_bytes))
    }
}
