mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
#[cfg_attr(not(feature = "contract"), allow(dead_code))]
mod native;
mod secp256k1;
use evm::{Context, ExitError};

pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::prelude::Vec;
use crate::AuroraState;
use crate::{
    precompiles::blake2::Blake2F,
    precompiles::bn128::{BN128Add, BN128Mul, BN128Pair},
    precompiles::hash::{RIPEMD160, SHA256},
    precompiles::identity::Identity,
    precompiles::modexp::ModExp,
    precompiles::native::{ExitToEthereum, ExitToNear},
    precompiles::secp256k1::ECRecover,
    prelude::Address,
    state::AuroraStackState,
};
use evm::backend::Log;
use evm::ExitSucceed;

#[derive(Debug)]
pub struct PrecompileOutput {
    pub cost: u64,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
}

impl PrecompileOutput {
    pub fn without_logs(cost: u64, output: Vec<u8>) -> Self {
        Self {
            cost,
            output,
            logs: Vec::new(),
        }
    }
}

impl Default for PrecompileOutput {
    fn default() -> Self {
        PrecompileOutput {
            cost: 0,
            output: Vec::new(),
            logs: Vec::new(),
        }
    }
}

impl From<PrecompileOutput> for evm::executor::PrecompileOutput {
    fn from(output: PrecompileOutput) -> Self {
        evm::executor::PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: output.cost,
            output: output.output,
            logs: output.logs,
        }
    }
}

/// A precompile operation result.
type PrecompileResult = Result<PrecompileOutput, ExitError>;

type EvmPrecompileResult = Result<evm::executor::PrecompileOutput, ExitError>;

/// A precompiled function for use in the EVM.
pub trait Precompile<S: AuroraState> {
    /// The required gas in order to run the precompile function.
    fn required_gas(input: &[u8]) -> Result<u64, ExitError>;

    /// Runs the precompile function.
    fn run(
        input: &[u8],
        target_gas: u64,
        context: &Context,
        state: &mut S,
        is_static: bool,
    ) -> PrecompileResult;
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
    _is_static: bool,
) -> Option<EvmPrecompileResult> {
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
    is_static: bool,
) -> Option<EvmPrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
    };

    let output = match address.0 {
        ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToEthereum::<AuroraStackState>::ADDRESS => Some(
            ExitToEthereum::<AuroraStackState>::run(input, target_gas, context, state, is_static),
        ),
        _ => None,
    };
    output.map(|res| res.map(Into::into))
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
    is_static: bool,
) -> Option<EvmPrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
    };

    let output = match address.0 {
        ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        Identity::<AuroraStackState>::ADDRESS => Some(Identity::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        modexp::ADDRESS => Some(ModExp::<Byzantium, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::ADD => Some(BN128Add::<Byzantium, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::MUL => Some(BN128Mul::<Byzantium, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::PAIR => Some(BN128Pair::<Byzantium, _>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToEthereum::<AuroraStackState>::ADDRESS => Some(
            ExitToEthereum::<AuroraStackState>::run(input, target_gas, context, state, is_static),
        ),
        _ => None,
    };
    output.map(|res| res.map(Into::into))
}

/// Matches the address given to Istanbul precompiles.
#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    context: &Context,
    state: &mut AuroraStackState,
    is_static: bool,
) -> Option<EvmPrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
    };

    let output = match address.0 {
        ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        Identity::<AuroraStackState>::ADDRESS => Some(Identity::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        modexp::ADDRESS => Some(ModExp::<Byzantium, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::ADD => Some(BN128Add::<Istanbul, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::MUL => Some(BN128Mul::<Istanbul, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::PAIR => Some(BN128Pair::<Istanbul, _>::run(
            input, target_gas, context, state, is_static,
        )),
        Blake2F::<AuroraStackState>::ADDRESS => Some(Blake2F::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToEthereum::<AuroraStackState>::ADDRESS => Some(
            ExitToEthereum::<AuroraStackState>::run(input, target_gas, context, state, is_static),
        ),
        _ => None,
    };
    output.map(|res| res.map(Into::into))
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
    is_static: bool,
) -> Option<EvmPrecompileResult> {
    let target_gas = match target_gas {
        Some(t) => t,
        None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
    };

    let output = match address.0 {
        ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        Identity::<AuroraStackState>::ADDRESS => Some(Identity::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        modexp::ADDRESS => Some(ModExp::<Berlin, _>::run(
            input, target_gas, context, state, is_static,
        )), // TODO gas changes
        bn128::addresses::ADD => Some(BN128Add::<Istanbul, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::MUL => Some(BN128Mul::<Istanbul, _>::run(
            input, target_gas, context, state, is_static,
        )),
        bn128::addresses::PAIR => Some(BN128Pair::<Istanbul, _>::run(
            input, target_gas, context, state, is_static,
        )),
        Blake2F::<AuroraStackState>::ADDRESS => Some(Blake2F::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
            input, target_gas, context, state, is_static,
        )),
        ExitToEthereum::<AuroraStackState>::ADDRESS => Some(
            ExitToEthereum::<AuroraStackState>::run(input, target_gas, context, state, is_static),
        ),
        _ => None,
    };
    output.map(|res| res.map(Into::into))
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
    use crate::test_utils::MockState;
    use rand::Rng;

    #[test]
    fn test_precompile_addresses() {
        assert_eq!(
            super::secp256k1::ECRecover::<MockState>::ADDRESS,
            u8_to_address(1)
        );
        assert_eq!(super::hash::SHA256::<MockState>::ADDRESS, u8_to_address(2));
        assert_eq!(
            super::hash::RIPEMD160::<MockState>::ADDRESS,
            u8_to_address(3)
        );
        assert_eq!(
            super::identity::Identity::<MockState>::ADDRESS,
            u8_to_address(4)
        );
        assert_eq!(super::modexp::ADDRESS, u8_to_address(5));
        assert_eq!(super::bn128::addresses::ADD, u8_to_address(6));
        assert_eq!(super::bn128::addresses::MUL, u8_to_address(7));
        assert_eq!(super::bn128::addresses::PAIR, u8_to_address(8));
        assert_eq!(
            super::blake2::Blake2F::<MockState>::ADDRESS,
            u8_to_address(9)
        );
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
