use crate::parameters::PromiseCreateArgs;
pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::prelude::{vec, Vec};
use crate::AuroraState;
use crate::{
    precompiles::blake2::Blake2F,
    precompiles::bn128::{Bn128Add, Bn128Mul, Bn128Pair},
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
use evm::{executor, Context, ExitError};

mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
#[cfg_attr(not(feature = "contract"), allow(dead_code))]
mod native;
mod secp256k1;

#[derive(Debug)]
pub struct PrecompileOutput {
    pub cost: u64,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
    pub promise: Option<PromiseCreateArgs>,
}

impl PrecompileOutput {
    pub fn without_logs(cost: u64, output: Vec<u8>) -> Self {
        Self {
            cost,
            output,
            logs: Vec::new(),
            promise: None,
        }
    }
}

impl Default for PrecompileOutput {
    fn default() -> Self {
        PrecompileOutput {
            cost: 0,
            output: Vec::new(),
            logs: Vec::new(),
            promise: None,
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
pub trait Precompile {
    /// The required gas in order to run the precompile function.
    fn required_gas(input: &[u8]) -> Result<u64, ExitError>;

    /// Runs the precompile function.
    fn run(input: &[u8], target_gas: u64, context: &Context, is_static: bool) -> PrecompileResult;
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

type PrecompileFn = fn(&[u8], u64, &Context, bool) -> PrecompileResult;

pub(crate) struct Precompiles {
    addresses: Vec<Address>,
    fun: Vec<PrecompileFn>,
}

impl Precompiles {
    #[allow(dead_code)]
    pub fn new_homestead() -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            ExitToNear::ADDRESS,
            ExitToEthereum::ADDRESS,
        ];
        let fun: Vec<PrecompileFn> = vec![
            ECRecover::run,
            SHA256::run,
            RIPEMD160::run,
            ExitToNear::run,
            ExitToEthereum::run,
        ];

        Precompiles { addresses, fun }
    }

    #[allow(dead_code)]
    pub fn new_byzantium() -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium>::ADDRESS,
            Bn128Add::<Byzantium>::ADDRESS,
            Bn128Mul::<Byzantium>::ADDRESS,
            Bn128Pair::<Byzantium>::ADDRESS,
            ExitToNear::ADDRESS,
            ExitToEthereum::ADDRESS,
        ];
        let fun: Vec<PrecompileFn> = vec![
            ECRecover::run,
            SHA256::run,
            RIPEMD160::run,
            Identity::run,
            ModExp::<Byzantium>::run,
            Bn128Add::<Byzantium>::run,
            Bn128Mul::<Byzantium>::run,
            Bn128Pair::<Byzantium>::run,
            ExitToNear::run,
            ExitToEthereum::run,
        ];

        Precompiles { addresses, fun }
    }

    pub fn new_istanbul() -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium>::ADDRESS,
            Bn128Add::<Istanbul>::ADDRESS,
            Bn128Mul::<Istanbul>::ADDRESS,
            Bn128Pair::<Istanbul>::ADDRESS,
            Blake2F::ADDRESS,
            ExitToNear::ADDRESS,
            ExitToEthereum::ADDRESS,
        ];
        let fun: Vec<PrecompileFn> = vec![
            ECRecover::run,
            SHA256::run,
            RIPEMD160::run,
            Identity::run,
            ModExp::<Byzantium>::run,
            Bn128Add::<Istanbul>::run,
            Bn128Mul::<Istanbul>::run,
            Bn128Pair::<Istanbul>::run,
            Blake2F::run,
            ExitToNear::run,
            ExitToEthereum::run,
        ];

        Precompiles { addresses, fun }
    }

    #[allow(dead_code)]
    fn new_berlin() -> Self {
        Self::new_istanbul()
    }

    fn get_fun(&self, address: &Address) -> Option<PrecompileFn> {
        self.addresses
            .iter()
            .position(|e| e == address)
            .and_then(|i| self.fun.get(i))
            .copied()
    }
}

/// Matches the address given to Homestead precompiles.
impl<'backend, 'config> executor::Precompiles<AuroraStackState<'backend, 'config>> for Precompiles {
    fn run(
        &self,
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

        let output = self.get_fun(&address).map(|fun| {
            let mut res = (fun)(input, target_gas, context, is_static);
            if let Ok(output) = &mut res {
                if let Some(promise) = output.promise.take() {
                    state.add_promise(promise)
                }
            }
            res
        });

        output.map(|res| res.map(Into::into))
    }

    fn addresses(&self) -> &[Address] {
        &self.addresses
    }
}

/// const fn for making an address by concatenating the bytes from two given numbers,
/// Note that 32 + 128 = 160 = 20 bytes (the length of an address). This function is used
/// as a convenience for specifying the addresses of the various precompiles.
const fn make_address(x: u32, y: u128) -> Address {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    Address([
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
    ])
}

#[cfg(test)]
mod tests {
    use crate::precompiles::{Byzantium, Istanbul};
    use crate::prelude::Address;
    use rand::Rng;

    #[test]
    fn test_precompile_addresses() {
        assert_eq!(super::secp256k1::ECRecover::ADDRESS, u8_to_address(1));
        assert_eq!(super::hash::SHA256::ADDRESS, u8_to_address(2));
        assert_eq!(super::hash::RIPEMD160::ADDRESS, u8_to_address(3));
        assert_eq!(super::identity::Identity::ADDRESS, u8_to_address(4));
        assert_eq!(super::ModExp::<Byzantium>::ADDRESS, u8_to_address(5));
        assert_eq!(super::Bn128Add::<Istanbul>::ADDRESS, u8_to_address(6));
        assert_eq!(super::Bn128Mul::<Istanbul>::ADDRESS, u8_to_address(7));
        assert_eq!(super::Bn128Pair::<Istanbul>::ADDRESS, u8_to_address(8));
        assert_eq!(super::blake2::Blake2F::ADDRESS, u8_to_address(9));
    }

    #[test]
    fn test_make_address() {
        for i in 0..u8::MAX {
            assert_eq!(super::make_address(0, i as u128), u8_to_address(i));
        }

        let mut rng = rand::thread_rng();
        for _ in 0..u8::MAX {
            let address: Address = Address(rng.gen());
            let (x, y) = split_address(address);
            assert_eq!(address, super::make_address(x, y))
        }
    }

    fn u8_to_address(x: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = x;
        Address(bytes)
    }

    // Inverse function of `super::make_address`.
    fn split_address(a: Address) -> (u32, u128) {
        let mut x_bytes = [0u8; 4];
        let mut y_bytes = [0u8; 16];

        x_bytes.copy_from_slice(&a[0..4]);
        y_bytes.copy_from_slice(&a[4..20]);

        (u32::from_be_bytes(x_bytes), u128::from_be_bytes(y_bytes))
    }
}
