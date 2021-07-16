mod blake2;
mod bn128;
mod hash;
mod identity;
mod modexp;
#[cfg_attr(not(feature = "contract"), allow(dead_code))]
mod native;
mod secp256k1;
use evm::{executor, Context, ExitError};

use crate::parameters::PromiseCreateArgs;
pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::prelude::{vec, BTreeMap, PhantomData, Vec};
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
use std::ops::Index;

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

struct Precompiles {
    addresses: Vec<Address>,
    fun: Vec<PrecompileFn>,
}

impl Precompiles {
    #[allow(dead_code)]
    fn new_homestead() -> Self {
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

    fn new_byzantium() -> Self {
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
        ];

        Precompiles { addresses, fun }
    }

    fn get_fun(&self, address: &Address) -> Option<PrecompileFn> {
        self.addresses
            .iter()
            .position(|e| e == address)
            .and_then(|i| self.fun.get(i))
            .map(|f| *f)
    }

    fn addresses(&self) -> &[Address] {
        &self.addresses
    }
}

pub(crate) struct PrecompileAddresses<HF> {
    precompiles: Precompiles,
    hark_fork: PhantomData<HF>,
}

/// Matches the address given to Homestead precompiles.
impl<'backend, 'config> executor::Precompiles<AuroraStackState<'backend, 'config>>
    for PrecompileAddresses<Homestead>
{
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

        let output = self.precompiles.get_fun(&address).and_then(|p| {
            let mut res = (p)(input, target_gas, context, is_static);
            if let Ok(mut o) = &mut res {
                if let Some(p) = o.promise.take() {
                    state.add_promise(p)
                }
            }
            Some(res)
        });

        output.map(|res| res.map(Into::into))
    }

    fn addresses(&self) -> &[Address] {
        &self.precompiles.addresses()
    }
}

impl PrecompileAddresses<Homestead> {
    #[allow(dead_code)]
    pub fn new_homestead() -> Self {
        Self {
            precompiles: Precompiles::new_homestead(),
            hark_fork: Default::default(),
        }
    }
}

//
// /// Matches the address given to Byzantium precompiles.
// impl<'backend, 'config> executor::Precompiles<AuroraStackState<'backend, 'config>>
//     for PrecompileAddresses<Byzantium>
// {
//     fn run(
//         &self,
//         address: Address,
//         input: &[u8],
//         target_gas: Option<u64>,
//         context: &Context,
//         state: &mut AuroraStackState,
//         is_static: bool,
//     ) -> Option<EvmPrecompileResult> {
//         let target_gas = match target_gas {
//             Some(t) => t,
//             None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
//         };
//
//         let output = match Address(address.0) {
//             ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             Identity::<AuroraStackState>::ADDRESS => Some(Identity::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             modexp::ADDRESS => Some(ModExp::<Byzantium, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::ADD => Some(Bn128Add::<Byzantium, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::MUL => Some(Bn128Mul::<Byzantium, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::PAIR => Some(Bn128Pair::<Byzantium, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             ExitToEthereum::<AuroraStackState>::ADDRESS => {
//                 Some(ExitToEthereum::<AuroraStackState>::run(
//                     input, target_gas, context, state, is_static,
//                 ))
//             }
//             _ => None,
//         };
//         output.map(|res| res.map(Into::into))
//     }
//
//     fn addresses(&self) -> &[Address] {
//         &self.addresses
//     }
// }
//
// impl PrecompileAddresses<Byzantium> {
//     #[allow(dead_code)]
//     pub fn byzantium() -> Self {
//         Self {
//             precompiles: Precompiles {},
//             hark_fork: Default::default(),
//         }
//     }
// }
//
// /// Matches the address given to Istanbul precompiles.
// impl<'backend, 'config> executor::Precompiles<AuroraStackState<'backend, 'config>>
//     for PrecompileAddresses<Istanbul>
// {
//     fn run(
//         &self,
//         address: Address,
//         input: &[u8],
//         target_gas: Option<u64>,
//         context: &Context,
//         state: &mut AuroraStackState,
//         is_static: bool,
//     ) -> Option<EvmPrecompileResult> {
//         let target_gas = match target_gas {
//             Some(t) => t,
//             None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
//         };
//
//         let output = match Address(address.0) {
//             ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             Identity::<AuroraStackState>::ADDRESS => Some(Identity::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             modexp::ADDRESS => Some(ModExp::<Byzantium, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::ADD => Some(Bn128Add::<Istanbul, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::MUL => Some(Bn128Mul::<Istanbul, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::PAIR => Some(Bn128Pair::<Istanbul, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             Blake2F::<AuroraStackState>::ADDRESS => Some(Blake2F::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             ExitToEthereum::<AuroraStackState>::ADDRESS => {
//                 Some(ExitToEthereum::<AuroraStackState>::run(
//                     input, target_gas, context, state, is_static,
//                 ))
//             }
//             _ => None,
//         };
//         output.map(|res| res.map(Into::into))
//     }
//
//     fn addresses(&self) -> &[Address] {
//         // &self.addresses.0.keys()
//     }
// }
//
// impl PrecompileAddresses<Istanbul> {
//     pub fn istanbul() -> Self {
//         Self {
//             addresses: vec![
//                 ECRecover::<AuroraStackState>::ADDRESS,
//                 SHA256::<AuroraStackState>::ADDRESS,
//                 RIPEMD160::<AuroraStackState>::ADDRESS,
//                 Identity::<AuroraStackState>::ADDRESS,
//                 modexp::ADDRESS,
//                 bn128::addresses::ADD,
//                 bn128::addresses::MUL,
//                 bn128::addresses::PAIR,
//                 Blake2F::<AuroraStackState>::ADDRESS,
//                 ExitToNear::<AuroraStackState>::ADDRESS,
//                 ExitToEthereum::<AuroraStackState>::ADDRESS,
//             ]
//             .into_iter()
//             .map(Address)
//             .collect(),
//             hark_fork: Default::default(),
//         }
//     }
// }
//
// /// Matches the address given to Berlin precompiles.
// impl<'backend, 'config> executor::Precompiles<AuroraStackState<'backend, 'config>>
//     for PrecompileAddresses<Berlin>
// {
//     fn run(
//         &self,
//         address: Address,
//         input: &[u8],
//         target_gas: Option<u64>,
//         context: &Context,
//         state: &mut AuroraStackState,
//         is_static: bool,
//     ) -> Option<EvmPrecompileResult> {
//         let target_gas = match target_gas {
//             Some(t) => t,
//             None => return Some(EvmPrecompileResult::Err(ExitError::OutOfGas)),
//         };
//
//         let output = match Address(address.0) {
//             ECRecover::<AuroraStackState>::ADDRESS => Some(ECRecover::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             SHA256::<AuroraStackState>::ADDRESS => Some(SHA256::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             RIPEMD160::<AuroraStackState>::ADDRESS => Some(RIPEMD160::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             Identity::<AuroraStackState>::ADDRESS => Some(Identity::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             modexp::ADDRESS => Some(ModExp::<Berlin, _>::run(
//                 input, target_gas, context, state, is_static,
//             )), // TODO gas changes
//             bn128::addresses::ADD => Some(Bn128Add::<Istanbul, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::MUL => Some(Bn128Mul::<Istanbul, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             bn128::addresses::PAIR => Some(Bn128Pair::<Istanbul, _>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             Blake2F::<AuroraStackState>::ADDRESS => Some(Blake2F::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             ExitToNear::<AuroraStackState>::ADDRESS => Some(ExitToNear::<AuroraStackState>::run(
//                 input, target_gas, context, state, is_static,
//             )),
//             ExitToEthereum::<AuroraStackState>::ADDRESS => {
//                 Some(ExitToEthereum::<AuroraStackState>::run(
//                     input, target_gas, context, state, is_static,
//                 ))
//             }
//             _ => None,
//         };
//         output.map(|res| res.map(Into::into))
//     }
//
//     fn addresses(&self) -> &[Address] {
//         // &self.addresses.0.keys()
//     }
// }
//
// impl PrecompileAddresses<Berlin> {
//     #[allow(dead_code)]
//     pub fn berlin() -> Self {
//         Self {
//             addresses: vec![
//                 ECRecover::<AuroraStackState>::ADDRESS,
//                 SHA256::<AuroraStackState>::ADDRESS,
//                 RIPEMD160::<AuroraStackState>::ADDRESS,
//                 Identity::<AuroraStackState>::ADDRESS,
//                 modexp::ADDRESS,
//                 bn128::addresses::ADD,
//                 bn128::addresses::MUL,
//                 bn128::addresses::PAIR,
//                 Blake2F::<AuroraStackState>::ADDRESS,
//                 ExitToNear::<AuroraStackState>::ADDRESS,
//                 ExitToEthereum::<AuroraStackState>::ADDRESS,
//             ]
//             .into_iter()
//             .map(Address)
//             .collect(),
//             hark_fork: Default::default(),
//         }
//     }
// }

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
        assert_eq!(super::ModExp::ADDRESS, u8_to_address(5));
        assert_eq!(super::Bn128Add::ADDRESS, u8_to_address(6));
        assert_eq!(super::Bn128Mul::ADDRESS, u8_to_address(7));
        assert_eq!(super::Bn128Pair::ADDRESS, u8_to_address(8));
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
