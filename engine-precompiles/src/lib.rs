#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

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
pub mod xcc;

use crate::account_ids::{predecessor_account, CurrentAccount, PredecessorAccount};
use crate::alt_bn256::{Bn256Add, Bn256Mul, Bn256Pair};
use crate::blake2::Blake2F;
use crate::hash::{RIPEMD160, SHA256};
use crate::identity::Identity;
use crate::modexp::ModExp;
use crate::native::{exit_to_ethereum, exit_to_near, ExitToEthereum, ExitToNear};
use crate::prelude::types::EthGas;
use crate::prelude::{Vec, H256};
use crate::prepaid_gas::PrepaidGas;
use crate::random::RandomSeed;
use crate::secp256k1::ECRecover;
use crate::xcc::CrossContractCall;
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
use aurora_engine_types::{account_id::AccountId, types::Address, vec, BTreeMap, BTreeSet, Box};
use aurora_evm::backend::Log;
use aurora_evm::executor::{
    self,
    stack::{PrecompileFailure, PrecompileHandle},
};
use aurora_evm::{Context, ExitError, ExitFatal, ExitSucceed};
use promise_result::PromiseResult;
use xcc::cross_contract_call;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PrecompileOutput {
    pub cost: EthGas,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
}

impl PrecompileOutput {
    #[must_use]
    pub const fn without_logs(cost: EthGas, output: Vec<u8>) -> Self {
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

pub trait HandleBasedPrecompile {
    fn run_with_handle(
        &self,
        handle: &mut impl PrecompileHandle,
    ) -> Result<PrecompileOutput, PrecompileFailure>;
}

/// Hard fork marker.
pub trait HardFork {}

/// Homestead hard fork marker.
pub struct Homestead;

/// Byzantium hard fork marker.
pub struct Byzantium;

/// Istanbul hard fork marker.
pub struct Istanbul;

/// Berlin hard fork marker.
pub struct Berlin;

/// Osaka hard fork marker.
pub struct Osaka;

impl HardFork for Homestead {}

impl HardFork for Byzantium {}

impl HardFork for Istanbul {}

impl HardFork for Berlin {}

impl HardFork for Osaka {}

pub struct Precompiles<'a, I, E, H> {
    pub all_precompiles: BTreeMap<Address, AllPrecompiles<'a, I, E, H>>,
    pub paused_precompiles: BTreeSet<Address>,
}

impl<I, E, H> Precompiles<'_, I, E, H> {
    fn is_paused(&self, address: &Address) -> bool {
        self.paused_precompiles.contains(address)
    }
}

impl<I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> executor::stack::PrecompileSet
    for Precompiles<'_, I, E, H>
{
    fn execute(
        &self,
        handle: &mut impl PrecompileHandle,
    ) -> Option<Result<executor::stack::PrecompileOutput, PrecompileFailure>> {
        let address = Address::new(handle.code_address());

        if self.is_paused(&address) {
            return Some(Err(PrecompileFailure::Fatal {
                exit_status: ExitFatal::Other(prelude::Cow::Borrowed("ERR_PAUSED")),
            }));
        }

        let result = match self.all_precompiles.get(&address)? {
            AllPrecompiles::ExitToNear(p) => process_precompile(p, handle),
            AllPrecompiles::ExitToEthereum(p) => process_precompile(p, handle),
            AllPrecompiles::PredecessorAccount(p) => process_precompile(p, handle),
            AllPrecompiles::PrepaidGas(p) => process_precompile(p, handle),
            AllPrecompiles::PromiseResult(p) => process_precompile(p, handle),
            AllPrecompiles::CrossContractCall(p) => process_handle_based_precompile(p, handle),
            AllPrecompiles::Generic(p) => process_precompile(p.as_ref(), handle),
        };

        Some(result.and_then(|output| post_process(output, handle)))
    }

    fn is_precompile(&self, address: prelude::H160) -> bool {
        self.all_precompiles.contains_key(&Address::new(address))
    }
}

fn process_precompile(
    p: &dyn Precompile,
    handle: &impl PrecompileHandle,
) -> Result<PrecompileOutput, PrecompileFailure> {
    let input = handle.input();
    let gas_limit = handle.gas_limit();
    let context = handle.context();
    let is_static = handle.is_static();

    p.run(input, gas_limit.map(EthGas::new), context, is_static)
        .map_err(|exit_status| PrecompileFailure::Error { exit_status })
}

fn process_handle_based_precompile(
    p: &impl HandleBasedPrecompile,
    handle: &mut impl PrecompileHandle,
) -> Result<PrecompileOutput, PrecompileFailure> {
    p.run_with_handle(handle)
}

fn post_process(
    output: PrecompileOutput,
    handle: &mut impl PrecompileHandle,
) -> Result<executor::stack::PrecompileOutput, PrecompileFailure> {
    handle.record_cost(output.cost.as_u64())?;
    for log in output.logs {
        handle.log(log.address, log.topics, log.data)?;
    }
    Ok(executor::stack::PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: output.output,
    })
}

pub struct PrecompileConstructorContext<'a, I, E, H, M> {
    pub current_account_id: AccountId,
    pub random_seed: H256,
    pub io: I,
    pub env: &'a E,
    pub promise_handler: H,
    pub mod_exp_algorithm: prelude::PhantomData<M>,
}

impl<'a, I: IO + Copy, E: Env, H: ReadOnlyPromiseHandler> Precompiles<'a, I, E, H> {
    #[allow(dead_code)]
    pub fn new_homestead<M: ModExpAlgorithm + 'static>(
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map = addresses
            .into_iter()
            .zip(fun)
            .map(|(a, f)| (a, AllPrecompiles::Generic(f)))
            .collect();
        Self::with_generic_precompiles(map, ctx)
    }

    #[allow(dead_code)]
    pub fn new_byzantium<M: ModExpAlgorithm + 'static>(
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium, M>::ADDRESS,
            Bn256Add::<Byzantium>::ADDRESS,
            Bn256Mul::<Byzantium>::ADDRESS,
            Bn256Pair::<Byzantium>::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Byzantium, M>::new()),
            Box::new(Bn256Add::<Byzantium>::new()),
            Box::new(Bn256Mul::<Byzantium>::new()),
            Box::new(Bn256Pair::<Byzantium>::new()),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map = addresses
            .into_iter()
            .zip(fun)
            .map(|(a, f)| (a, AllPrecompiles::Generic(f)))
            .collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_istanbul<M: ModExpAlgorithm + 'static>(
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Byzantium, M>::ADDRESS,
            Bn256Add::<Istanbul>::ADDRESS,
            Bn256Mul::<Istanbul>::ADDRESS,
            Bn256Pair::<Istanbul>::ADDRESS,
            Blake2F::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Byzantium, M>::new()),
            Box::new(Bn256Add::<Istanbul>::new()),
            Box::new(Bn256Mul::<Istanbul>::new()),
            Box::new(Bn256Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map = addresses
            .into_iter()
            .zip(fun)
            .map(|(a, f)| (a, AllPrecompiles::Generic(f)))
            .collect();

        Self::with_generic_precompiles(map, ctx)
    }

    pub fn new_berlin<M: ModExpAlgorithm + 'static>(
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Berlin, M>::ADDRESS,
            Bn256Add::<Istanbul>::ADDRESS,
            Bn256Mul::<Istanbul>::ADDRESS,
            Bn256Pair::<Istanbul>::ADDRESS,
            Blake2F::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Berlin, M>::new()),
            Box::new(Bn256Add::<Istanbul>::new()),
            Box::new(Bn256Mul::<Istanbul>::new()),
            Box::new(Bn256Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map = addresses
            .into_iter()
            .zip(fun)
            .map(|(a, f)| (a, AllPrecompiles::Generic(f)))
            .collect();

        Self::with_generic_precompiles(map, ctx)
    }

    /// Builds a Precompiles set configured for the London hard fork.
    ///
    /// Returns a Precompiles instance populated with the precompiles active in the London hard fork.
    ///
    /// # Examples
    ///
    /// ```
    /// let ctx = /* PrecompileConstructorContext::new(...) */ ;
    /// let precompiles = Precompiles::new_london(ctx);
    /// ```
    pub fn new_london<M: ModExpAlgorithm + 'static>(
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        // no precompile changes in London HF
        Self::new_berlin(ctx)
    }

    /// Constructs a Precompiles set configured for the Osaka hard fork.
    ///
    /// The returned Precompiles contains the standard precompiles enabled for Osaka:
    /// ECRecover, SHA256, RIPEMD160, Identity, the Osaka ModExp implementation, BN256 ops
    /// (Istanbul variants), Blake2F, RandomSeed, and CurrentAccount, wired into the
    /// generic precompile map alongside the engine-provided cross-contract helpers.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a prepared `ctx: PrecompileConstructorContext<_, _, _, _, _>`,
    /// // construct the Osaka precompile set:
    /// // let precompiles = Precompiles::new_osaka(ctx);
    /// ```
    pub fn new_osaka<M: ModExpAlgorithm + 'static>(
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        let addresses = vec![
            ECRecover::ADDRESS,
            SHA256::ADDRESS,
            RIPEMD160::ADDRESS,
            Identity::ADDRESS,
            ModExp::<Osaka, M>::ADDRESS,
            Bn256Add::<Istanbul>::ADDRESS,
            Bn256Mul::<Istanbul>::ADDRESS,
            Bn256Pair::<Istanbul>::ADDRESS,
            Blake2F::ADDRESS,
            RandomSeed::ADDRESS,
            CurrentAccount::ADDRESS,
        ];
        let fun: Vec<Box<dyn Precompile>> = vec![
            Box::new(ECRecover),
            Box::new(SHA256),
            Box::new(RIPEMD160),
            Box::new(Identity),
            Box::new(ModExp::<Osaka, M>::new()),
            Box::new(Bn256Add::<Istanbul>::new()),
            Box::new(Bn256Mul::<Istanbul>::new()),
            Box::new(Bn256Pair::<Istanbul>::new()),
            Box::new(Blake2F),
            Box::new(RandomSeed::new(ctx.random_seed)),
            Box::new(CurrentAccount::new(ctx.current_account_id.clone())),
        ];
        let map = addresses
            .into_iter()
            .zip(fun)
            .map(|(a, f)| (a, AllPrecompiles::Generic(f)))
            .collect();

        Self::with_generic_precompiles(map, ctx)
    }

    /// Inserts the standard built-in precompiles into a provided map and returns a `Precompiles`
    /// instance with an empty paused set.
    ///
    /// This function augments `generic_precompiles` with the following precompiles keyed by their
    /// canonical addresses: exit-to-near, exit-to-ethereum, cross-contract call, predecessor account,
    /// prepaid gas, and promise result. The resulting `Precompiles` has `all_precompiles` set to the
    /// augmented map and `paused_precompiles` initialized empty.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::collections::BTreeMap;
    ///
    /// // `ctx` must be a properly constructed `PrecompileConstructorContext`.
    /// let generic: BTreeMap<_, _> = BTreeMap::new();
    /// let ctx = /* construct context */ unimplemented!();
    ///
    /// let precompiles = with_generic_precompiles(generic, ctx);
    /// ```
    fn with_generic_precompiles<M: ModExpAlgorithm + 'static>(
        mut generic_precompiles: BTreeMap<Address, AllPrecompiles<'a, I, E, H>>,
        ctx: PrecompileConstructorContext<'a, I, E, H, M>,
    ) -> Self {
        let near_exit = ExitToNear::new(ctx.current_account_id.clone(), ctx.io);
        let ethereum_exit = ExitToEthereum::new(ctx.io);
        let cross_contract_call = CrossContractCall::new(ctx.current_account_id, ctx.io);
        let predecessor_account_id = PredecessorAccount::new(ctx.env);
        let prepaid_gas = PrepaidGas::new(ctx.env);
        let promise_results = PromiseResult::new(ctx.promise_handler);

        generic_precompiles.insert(exit_to_near::ADDRESS, AllPrecompiles::ExitToNear(near_exit));
        generic_precompiles.insert(
            exit_to_ethereum::ADDRESS,
            AllPrecompiles::ExitToEthereum(ethereum_exit),
        );
        generic_precompiles.insert(
            cross_contract_call::ADDRESS,
            AllPrecompiles::CrossContractCall(cross_contract_call),
        );
        generic_precompiles.insert(
            predecessor_account::ADDRESS,
            AllPrecompiles::PredecessorAccount(predecessor_account_id),
        );
        generic_precompiles.insert(
            prepaid_gas::ADDRESS,
            AllPrecompiles::PrepaidGas(prepaid_gas),
        );
        generic_precompiles.insert(
            promise_result::ADDRESS,
            AllPrecompiles::PromiseResult(promise_results),
        );

        Self {
            all_precompiles: generic_precompiles,
            paused_precompiles: BTreeSet::new(),
        }
    }
}

pub enum AllPrecompiles<'a, I, E, H> {
    ExitToNear(ExitToNear<I>),
    ExitToEthereum(ExitToEthereum<I>),
    CrossContractCall(CrossContractCall<I>),
    PredecessorAccount(PredecessorAccount<'a, E>),
    PrepaidGas(PrepaidGas<'a, E>),
    PromiseResult(PromiseResult<H>),
    Generic(Box<dyn Precompile>),
}

const fn make_h256(x: u128, y: u128) -> H256 {
    let x_bytes = x.to_be_bytes();
    let y_bytes = y.to_be_bytes();
    H256([
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
    #[allow(clippy::too_many_lines)]
    fn test_paused_precompiles_throws_error() {
        use crate::{
            AllPrecompiles, Context, EvmPrecompileResult, ExitError, Precompile, PrecompileOutput,
            Precompiles,
        };
        use aurora_engine_sdk::env::Fixed;
        use aurora_engine_sdk::promise::Noop;
        use aurora_engine_test_doubles::io::StoragePointer;
        use aurora_engine_types::types::EthGas;
        use aurora_evm::executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileSet};
        use aurora_evm::{ExitFatal, ExitReason, Transfer};

        struct MockPrecompile;

        impl Precompile for MockPrecompile {
            fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError>
            where
                Self: Sized,
            {
                Ok(EthGas::new(0))
            }

            fn run(
                &self,
                _input: &[u8],
                _target_gas: Option<EthGas>,
                _context: &Context,
                _is_static: bool,
            ) -> EvmPrecompileResult {
                Ok(PrecompileOutput::default())
            }
        }

        struct MockPrecompileHandle {
            code_address: H160,
        }

        impl MockPrecompileHandle {
            pub const fn new(code_address: H160) -> Self {
                Self { code_address }
            }
        }

        impl PrecompileHandle for MockPrecompileHandle {
            fn call(
                &mut self,
                _to: H160,
                _transfer: Option<Transfer>,
                _input: Vec<u8>,
                _gas_limit: Option<u64>,
                _is_static: bool,
                _context: &Context,
            ) -> (ExitReason, Vec<u8>) {
                unimplemented!()
            }

            fn record_cost(&mut self, _cost: u64) -> Result<(), ExitError> {
                unimplemented!()
            }

            fn record_external_cost(
                &mut self,
                _ref_time: Option<u64>,
                _proof_size: Option<u64>,
                _storage_growth: Option<u64>,
            ) -> Result<(), ExitError> {
                unimplemented!()
            }

            fn refund_external_cost(&mut self, _ref_time: Option<u64>, _proof_size: Option<u64>) {
                unimplemented!()
            }

            fn remaining_gas(&self) -> u64 {
                unimplemented!()
            }

            fn log(
                &mut self,
                _address: H160,
                _topics: Vec<aurora_engine_types::H256>,
                _data: Vec<u8>,
            ) -> Result<(), ExitError> {
                unimplemented!()
            }

            fn code_address(&self) -> H160 {
                self.code_address
            }

            fn input(&self) -> &[u8] {
                unimplemented!()
            }

            fn context(&self) -> &Context {
                unimplemented!()
            }

            fn is_static(&self) -> bool {
                unimplemented!()
            }

            fn gas_limit(&self) -> Option<u64> {
                unimplemented!()
            }
        }

        let precompile_address = Address::default();
        let precompile: AllPrecompiles<StoragePointer, Fixed, Noop> =
            AllPrecompiles::Generic(Box::new(MockPrecompile));

        let precompiles: Precompiles<StoragePointer, Fixed, Noop> = Precompiles {
            all_precompiles: {
                let mut map = prelude::BTreeMap::new();
                map.insert(precompile_address, precompile);
                map
            },
            paused_precompiles: {
                let mut set = prelude::BTreeSet::new();
                set.insert(precompile_address);
                set
            },
        };
        let mut precompile_handle = MockPrecompileHandle::new(precompile_address.raw());

        let result = precompiles
            .execute(&mut precompile_handle)
            .expect("result must contain error but is empty");
        let actual_failure = result.expect_err("result must contain failure but is successful");
        let expected_failure = PrecompileFailure::Fatal {
            exit_status: ExitFatal::Other(prelude::Cow::Borrowed("ERR_PAUSED")),
        };

        assert_eq!(expected_failure, actual_failure);
    }

    const fn u8_to_address(x: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = x;
        Address::new(H160(bytes))
    }
}