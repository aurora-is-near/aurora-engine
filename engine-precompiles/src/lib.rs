#![allow(dead_code)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![deny(clippy::as_conversions)]

pub mod account_ids;
pub mod alt_bn256;
pub mod blake2;
pub mod hash;
pub mod identity;
pub mod modexp;
pub mod native;
mod prelude;
pub mod prepaid_gas;
pub mod promise_result;
pub mod random;
pub mod secp256k1;
mod utils;

use crate::account_ids::{predecessor_account, CurrentAccount, PredecessorAccount};
use crate::alt_bn256::{Bn256Add, Bn256Mul, Bn256Pair};
use crate::blake2::Blake2F;
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
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
use aurora_engine_types::{account_id::AccountId, types::Address, vec, BTreeMap, Box};
use evm::backend::Log;
use evm::executor::{self, stack::PrecompileHandle};
use evm::{Context, ExitError, ExitSucceed};
use promise_result::PromiseResult;

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

type EvmPrecompileResult = Result<PrecompileOutput, ExitError>;

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

pub struct Precompiles<'a, I, E, H> {
    pub generic_precompiles: prelude::BTreeMap<Address, Box<dyn Precompile>>,
    // Cannot be part of the generic precompiles because the `dyn` type-erasure messes with
    // with the lifetime requirements on the type parameter `I`.
    pub near_exit: ExitToNear<I>,
    pub ethereum_exit: ExitToEthereum<I>,
    pub predecessor_account_id: PredecessorAccount<'a, E>,
    pub prepaid_gas: PrepaidGas<'a, E>,
    pub promise_results: PromiseResult<H>,
}

impl<'a, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> executor::stack::PrecompileSet
    for Precompiles<'a, I, E, H>
{
    fn execute(
        &self,
        handle: &mut impl PrecompileHandle,
    ) -> Option<Result<executor::stack::PrecompileOutput, executor::stack::PrecompileFailure>> {
        let address = handle.code_address();

        self.precompile_action(Address::new(address), |p| {
            let input = handle.input();
            let gas_limit = handle.gas_limit();
            let context = handle.context();
            let is_static = handle.is_static();

            match p.run(input, gas_limit.map(EthGas::new), context, is_static) {
                Ok(output) => {
                    handle.record_cost(output.cost.as_u64())?;
                    for log in output.logs {
                        handle.log(log.address, log.topics, log.data)?;
                    }
                    Ok(executor::stack::PrecompileOutput {
                        exit_status: ExitSucceed::Returned,
                        output: output.output,
                    })
                }
                Err(exit_status) => Err(executor::stack::PrecompileFailure::Error { exit_status }),
            }
        })
    }

    fn is_precompile(&self, address: prelude::H160) -> bool {
        self.precompile_action(Address::new(address), |_| true)
            .unwrap_or(false)
    }
}

pub struct PrecompileConstructorContext<'a, I, E, H> {
    pub current_account_id: AccountId,
    pub random_seed: H256,
    pub io: I,
    pub env: &'a E,
    pub promise_handler: H,
}

impl<'a, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> Precompiles<'a, I, E, H> {
    #[allow(dead_code)]
    pub fn new_homestead(ctx: PrecompileConstructorContext<'a, I, E, H>) -> Self {
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
    pub fn new_byzantium(ctx: PrecompileConstructorContext<'a, I, E, H>) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium>::ADDRESS,
            Bn256Add::<Byzantium>::ADDRESS,
            Bn256Mul::<Byzantium>::ADDRESS,
            Bn256Pair::<Byzantium>::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: prelude::Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Byzantium>::new()),
            Box::new(Bn256Add::<Byzantium>::new()),
            Box::new(Bn256Mul::<Byzantium>::new()),
            Box::new(Bn256Pair::<Byzantium>::new()),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_istanbul(ctx: PrecompileConstructorContext<'a, I, E, H>) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium>::ADDRESS,
            Bn256Add::<Istanbul>::ADDRESS,
            Bn256Mul::<Istanbul>::ADDRESS,
            Bn256Pair::<Istanbul>::ADDRESS,
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
            Box::new(Bn256Add::<Istanbul>::new()),
            Box::new(Bn256Mul::<Istanbul>::new()),
            Box::new(Bn256Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_berlin(ctx: PrecompileConstructorContext<'a, I, E, H>) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Berlin>::ADDRESS,
            Bn256Add::<Istanbul>::ADDRESS,
            Bn256Mul::<Istanbul>::ADDRESS,
            Bn256Pair::<Istanbul>::ADDRESS,
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
            Box::new(Bn256Add::<Istanbul>::new()),
            Box::new(Bn256Mul::<Istanbul>::new()),
            Box::new(Bn256Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map: BTreeMap<Address, Box<dyn Precompile>> = addresses.into_iter().zip(fun).collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_london(ctx: PrecompileConstructorContext<'a, I, E, H>) -> Self {
        // no precompile changes in London HF
        Self::new_berlin(ctx)
    }

    fn with_generic_precompiles(
        generic_precompiles: BTreeMap<Address, Box<dyn Precompile>>,
        ctx: PrecompileConstructorContext<'a, I, E, H>,
    ) -> Self {
        let near_exit = ExitToNear::new(ctx.current_account_id.clone(), ctx.io);
        let ethereum_exit = ExitToEthereum::new(ctx.current_account_id, ctx.io);
        let predecessor_account_id = PredecessorAccount::new(ctx.env);
        let prepaid_gas = PrepaidGas::new(ctx.env);
        let promise_results = PromiseResult::new(ctx.promise_handler);

        Self {
            generic_precompiles,
            near_exit,
            ethereum_exit,
            predecessor_account_id,
            prepaid_gas,
            promise_results,
        }
    }

    fn precompile_action<U, F: FnOnce(&dyn Precompile) -> U>(
        &self,
        address: Address,
        f: F,
    ) -> Option<U> {
        if address == exit_to_near::ADDRESS {
            return Some(f(&self.near_exit));
        } else if address == exit_to_ethereum::ADDRESS {
            return Some(f(&self.ethereum_exit));
        } else if address == predecessor_account::ADDRESS {
            return Some(f(&self.predecessor_account_id));
        } else if address == prepaid_gas::ADDRESS {
            return Some(f(&self.prepaid_gas));
        } else if address == promise_result::ADDRESS {
            return Some(f(&self.promise_results));
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
        assert_eq!(super::Bn256Add::<Istanbul>::ADDRESS, u8_to_address(6));
        assert_eq!(super::Bn256Mul::<Istanbul>::ADDRESS, u8_to_address(7));
        assert_eq!(super::Bn256Pair::<Istanbul>::ADDRESS, u8_to_address(8));
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
