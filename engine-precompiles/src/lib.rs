#![allow(dead_code)]
#![feature(array_methods)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![cfg_attr(feature = "log", feature(panic_info_message))]

pub mod blake2;
pub mod bn128;
pub mod hash;
pub mod identity;
pub mod modexp;
pub mod native;
mod prelude;
pub mod random;
pub mod secp256k1;
#[cfg(test)]
mod utils;

use crate::blake2::Blake2F;
use crate::bn128::{Bn128Add, Bn128Mul, Bn128Pair};
use crate::hash::{RIPEMD160, SHA256};
use crate::identity::Identity;
use crate::modexp::ModExp;
use crate::native::{ExitToEthereum, ExitToNear};
use crate::prelude::types::EthGas;
use crate::prelude::{Vec, H160, H256};
use crate::random::RandomSeed;
use crate::secp256k1::ECRecover;
use aurora_engine_types::{account_id::AccountId, types_new::Address, vec, BTreeMap, Box};
use evm::backend::Log;
use evm::executor;
use evm::{Context, ExitError, ExitSucceed};

#[derive(Debug, Default)]
pub struct PrecompileOutput {
    pub cost: EthGas,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
}

impl PrecompileOutput {
    pub fn without_logs(cost: EthGas, output: Vec<u8>) -> Self {
        Self {
            cost,
            output,
            logs: Vec::new(),
        }
    }
}

impl From<PrecompileOutput> for evm::executor::PrecompileOutput {
    fn from(output: PrecompileOutput) -> Self {
        evm::executor::PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: output.cost.into_u64(),
            output: output.output,
            logs: output.logs,
        }
    }
}

type EvmPrecompileResult = Result<evm::executor::PrecompileOutput, ExitError>;

/// A precompiled function for use in the EVM.
pub trait Precompile {
    /// The required gas in order to run the precompile function.
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized;

    /// Runs the precompile function.
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult;
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

pub struct Precompiles(pub prelude::BTreeMap<Address, Box<dyn Precompile>>);

impl executor::PrecompileSet for Precompiles {
    fn execute(
        &self,
        address: prelude::H160,
        input: &[u8],
        gas_limit: Option<u64>,
        context: &Context,
        is_static: bool,
    ) -> Option<Result<executor::PrecompileOutput, executor::PrecompileFailure>> {
        self.0.get(&Address::new(address)).map(|p| {
            p.run(input, gas_limit.map(EthGas::new), context, is_static)
                .map_err(|exit_status| executor::PrecompileFailure::Error { exit_status })
        })
    }

    fn is_precompile(&self, address: prelude::H160) -> bool {
        self.0.contains_key(&Address::new(address))
    }
}

pub struct PrecompileConstructorContext {
    pub current_account_id: AccountId,
    pub random_seed: H256,
}

impl Precompiles {
    #[allow(dead_code)]
    pub fn new_homestead(ctx: PrecompileConstructorContext) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            ExitToNear::ADDRESS,
            ExitToEthereum::ADDRESS,
            RandomSeed::ADDRESS,
        ];
        let fun: prelude::Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(ExitToNear::new(ctx.current_account_id.clone())),
            Box::new(ExitToEthereum::new(ctx.current_account_id)),
            Box::new(RandomSeed::new(ctx.random_seed)),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Precompiles(map)
    }

    #[allow(dead_code)]
    pub fn new_byzantium(ctx: PrecompileConstructorContext) -> Self {
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
            RandomSeed::ADDRESS,
        ];
        let fun: prelude::Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Byzantium>::new()),
            Box::new(Bn128Add::<Byzantium>::new()),
            Box::new(Bn128Mul::<Byzantium>::new()),
            Box::new(Bn128Pair::<Byzantium>::new()),
            Box::new(ExitToNear::new(ctx.current_account_id.clone())),
            Box::new(ExitToEthereum::new(ctx.current_account_id)),
            Box::new(RandomSeed::new(ctx.random_seed)),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Precompiles(map)
    }

    pub fn new_istanbul(ctx: PrecompileConstructorContext) -> Self {
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
            RandomSeed::ADDRESS,
        ];
        let fun: prelude::Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Byzantium>::new()),
            Box::new(Bn128Add::<Istanbul>::new()),
            Box::new(Bn128Mul::<Istanbul>::new()),
            Box::new(Bn128Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(ExitToNear::new(ctx.current_account_id.clone())),
            Box::new(ExitToEthereum::new(ctx.current_account_id)),
            Box::new(RandomSeed::new(ctx.random_seed)),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Precompiles(map)
    }

    pub fn new_berlin(ctx: PrecompileConstructorContext) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Berlin>::ADDRESS,
            Bn128Add::<Istanbul>::ADDRESS,
            Bn128Mul::<Istanbul>::ADDRESS,
            Bn128Pair::<Istanbul>::ADDRESS,
            Blake2F::ADDRESS,
            ExitToNear::ADDRESS,
            ExitToEthereum::ADDRESS,
            RandomSeed::ADDRESS,
        ];
        let fun: prelude::Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Berlin>::new()),
            Box::new(Bn128Add::<Istanbul>::new()),
            Box::new(Bn128Mul::<Istanbul>::new()),
            Box::new(Bn128Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(ExitToNear::new(ctx.current_account_id.clone())),
            Box::new(ExitToEthereum::new(ctx.current_account_id)),
            Box::new(RandomSeed::new(ctx.random_seed)),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Precompiles(map)
    }

    pub fn new_london(ctx: PrecompileConstructorContext) -> Self {
        // no precompile changes in London HF
        Self::new_berlin(ctx)
    }
}

/// fn for making an address by concatenating the bytes from two given numbers,
/// Note that 32 + 128 = 160 = 20 bytes (the length of an address). This function is used
/// as a convenience for specifying the addresses of the various precompiles.
pub const fn make_address(x: u32, y: u128) -> prelude::types_new::Address {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    prelude::types_new::ADDRESS(H160([
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
    ]))
}

const fn make_h256(x: u128, y: u128) -> prelude::H256 {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    prelude::H256([
        x_bytes[0],
        x_bytes[1],
        x_bytes[2],
        x_bytes[3],
        x_bytes[4],
        x_bytes[5],
        x_bytes[6],
        x_bytes[7],
        x_bytes[8],
        x_bytes[9],
        x_bytes[10],
        x_bytes[11],
        x_bytes[12],
        x_bytes[13],
        x_bytes[14],
        x_bytes[15],
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
    use crate::prelude::H160;
    use crate::{prelude, Byzantium, Istanbul};
    use prelude::types_new::{Address, ADDRESS};
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
            let address = ADDRESS(H160(rng.gen()));
            let (x, y) = split_address(address);
            assert_eq!(address, super::make_address(x, y))
        }
    }

    fn u8_to_address(x: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = x;
        ADDRESS(H160(bytes))
    }

    // Inverse function of `super::make_address`.
    fn split_address(a: Address) -> (u32, u128) {
        let mut x_bytes = [0u8; 4];
        let mut y_bytes = [0u8; 16];

        x_bytes.copy_from_slice(&a.raw()[0..4]);
        y_bytes.copy_from_slice(&a.raw()[4..20]);

        (u32::from_be_bytes(x_bytes), u128::from_be_bytes(y_bytes))
    }
}
