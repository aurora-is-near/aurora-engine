#![allow(dead_code)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

pub mod account_ids;
pub mod blake2;
pub mod bn128;
pub mod hash;
pub mod identity;
pub mod modexp;
pub mod native;
mod prelude;
pub mod prepaid_gas;
pub mod random;
pub mod secp256k1;
#[cfg(test)]
mod utils;

use crate::account_ids::{predecessor_account, CurrentAccount, PredecessorAccount};
use crate::blake2::Blake2F;
use crate::bn128::{Bn128Add, Bn128Mul, Bn128Pair};
use crate::hash::{RIPEMD160, SHA256};
use crate::identity::Identity;
use crate::modexp::ModExp;
use crate::native::{exit_to_ethereum, exit_to_near, ExitToEthereum, ExitToNear};
use crate::prelude::types::EthGas;
use crate::prelude::{Vec, H160, H256};
use crate::prepaid_gas::PrepaidGas;
use crate::random::RandomSeed;
use crate::secp256k1::ECRecover;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_types::{account_id::AccountId, types::Address, vec, BTreeMap, Box};
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

impl From<PrecompileOutput> for executor::stack::PrecompileOutput {
    fn from(output: PrecompileOutput) -> Self {
        executor::stack::PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: output.cost.as_u64(),
            output: output.output,
            logs: output.logs,
        }
    }
}

type EvmPrecompileResult = Result<executor::stack::PrecompileOutput, ExitError>;

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

pub struct Precompiles<'a, I, E> {
    pub generic_precompiles: prelude::BTreeMap<Address, Box<dyn Precompile>>,
    // Cannot be part of the generic precompiles because the `dyn` type-erasure messes with
    // with the lifetime requirements on the type parameter `I`.
    pub near_exit: Option<ExitToNear<I>>,
    pub ethereum_exit: Option<ExitToEthereum<I>>,
    pub predecessor_account_id: Option<PredecessorAccount<'a, E>>,
    pub prepaid_gas: Option<PrepaidGas<'a, E>>,
}

impl<'a, I: IO + Copy, E: Env> executor::stack::PrecompileSet for Precompiles<'a, I, E> {
    fn execute(
        &self,
        address: prelude::H160,
        input: &[u8],
        gas_limit: Option<u64>,
        context: &Context,
        is_static: bool,
    ) -> Option<Result<executor::stack::PrecompileOutput, executor::stack::PrecompileFailure>> {
        self.precompile_action(Address::new(address), |p| {
            p.run(input, gas_limit.map(EthGas::new), context, is_static)
                .map_err(|exit_status| executor::stack::PrecompileFailure::Error { exit_status })
        })
    }

    fn is_precompile(&self, address: prelude::H160) -> bool {
        self.precompile_action(Address::new(address), |_| true)
            .unwrap_or(false)
    }
}

pub struct PrecompileConstructorContext<'a, I, E> {
    pub current_account_id: AccountId,
    pub random_seed: H256,
    pub io: I,
    pub env: &'a E,
}

impl<'a, I: IO + Copy, E: Env> Precompiles<'a, I, E> {
    #[allow(dead_code)]
    pub fn new_homestead(ctx: PrecompileConstructorContext<'a, I, E>) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: prelude::Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();
        Self::with_generic_precompiles(map, ctx)
    }

    #[allow(dead_code)]
    pub fn new_byzantium(ctx: PrecompileConstructorContext<'a, I, E>) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium>::ADDRESS,
            Bn128Add::<Byzantium>::ADDRESS,
            Bn128Mul::<Byzantium>::ADDRESS,
            Bn128Pair::<Byzantium>::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
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
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_istanbul(ctx: PrecompileConstructorContext<'a, I, E>) -> Self {
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
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
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
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_berlin(ctx: PrecompileConstructorContext<'a, I, E>) -> Self {
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
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
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
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_london(ctx: PrecompileConstructorContext<'a, I, E>) -> Self {
        // no precompile changes in London HF
        Self::new_berlin(ctx)
    }

    fn with_generic_precompiles(
        generic_precompiles: BTreeMap<Address, Box<dyn Precompile>>,
        ctx: PrecompileConstructorContext<'a, I, E>,
    ) -> Self {
        let near_exit = Some(ExitToNear::new(ctx.current_account_id.clone(), ctx.io));
        let ethereum_exit = Some(ExitToEthereum::new(ctx.current_account_id, ctx.io));
        let predecessor_account_id = Some(PredecessorAccount::new(ctx.env));
        let prepaid_gas = Some(PrepaidGas::new(ctx.env));

        Self {
            generic_precompiles,
            near_exit,
            ethereum_exit,
            predecessor_account_id,
            prepaid_gas,
        }
    }

    fn precompile_action<U, F: FnOnce(&dyn Precompile) -> U>(
        &self,
        address: Address,
        f: F,
    ) -> Option<U> {
        if address == exit_to_near::ADDRESS {
            return self.near_exit.as_ref().map(|v| Some(f(v))).unwrap_or(None);
        } else if address == exit_to_ethereum::ADDRESS {
            return self
                .ethereum_exit
                .as_ref()
                .map(|v| Some(f(v)))
                .unwrap_or(None);
        } else if address == predecessor_account::ADDRESS {
            return self
                .predecessor_account_id
                .as_ref()
                .map(|v| Some(f(v)))
                .unwrap_or(None);
        } else if address == prepaid_gas::ADDRESS {
            return self
                .prepaid_gas
                .as_ref()
                .map(|v| Some(f(v)))
                .unwrap_or(None);
        }
        self.generic_precompiles
            .get(&address)
            .map(|p| f(p.as_ref()))
    }
}

/// fn for making an address by concatenating the bytes from two given numbers,
/// Note that 32 + 128 = 160 = 20 bytes (the length of an address). This function is used
/// as a convenience for specifying the addresses of the various precompiles.
pub const fn make_address(x: u32, y: u128) -> prelude::types::Address {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    prelude::types::Address::new(H160([
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
    use prelude::types::Address;
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
            let address = Address::new(H160(rng.gen()));
            let (x, y) = split_address(address);
            assert_eq!(address, super::make_address(x, y))
        }
    }

    fn u8_to_address(x: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = x;
        Address::new(H160(bytes))
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
