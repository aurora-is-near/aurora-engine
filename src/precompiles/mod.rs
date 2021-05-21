mod blake2;
mod bn128;
#[cfg(feature = "contract")]
mod entry;
mod hash;
mod identity;
mod modexp;
#[cfg(feature = "contract")]
mod native;
mod secp256k1;

#[cfg(feature = "contract")]
pub use entry::{berlin_precompiles, byzantium_precompiles, istanbul_precompiles};
pub use entry::{
    exit_to_near_address, Berlin, Byzantium, HardFork, Istanbul, Precompile, PrecompileResult,
};

pub use self::secp256k1::ecrecover;
