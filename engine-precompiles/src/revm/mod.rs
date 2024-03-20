mod identity;

use aurora_engine_types::{Box, PhantomData};
use once_cell::race::OnceBox;
use revm::precompile::PrecompileError;

pub struct Precompiles<'a, I, E, H> {
    _marker: PhantomData<&'a (I, E, H)>,
}

impl From<crate::PrecompileError> for PrecompileError {
    fn from(value: crate::PrecompileError) -> Self {
        match value {
            crate::PrecompileError::OutOfGas => Self::OutOfGas,
            crate::PrecompileError::Blake2WrongLength => Self::Blake2WrongLength,
            crate::PrecompileError::Blake2WrongFinalIndicatorFlag => {
                Self::Blake2WrongFinalIndicatorFlag
            }
            crate::PrecompileError::ModexpExpOverflow => Self::ModexpExpOverflow,
            crate::PrecompileError::ModexpBaseOverflow => Self::ModexpBaseOverflow,
            crate::PrecompileError::ModexpModOverflow => Self::ModexpModOverflow,
            crate::PrecompileError::Bn128FieldPointNotAMember => Self::Bn128FieldPointNotAMember,
            crate::PrecompileError::Bn128AffineGFailedToCreate => Self::Bn128AffineGFailedToCreate,
            crate::PrecompileError::Bn128PairLength => Self::Bn128PairLength,
            crate::PrecompileError::BlobInvalidInputLength => Self::BlobInvalidInputLength,
            crate::PrecompileError::BlobMismatchedVersion => Self::BlobMismatchedVersion,
            crate::PrecompileError::BlobVerifyKzgProofFailed => Self::BlobVerifyKzgProofFailed,
            crate::PrecompileError::Other(err) => Self::Other(err.into_owned()),
        }
    }
}

/// Returns precompiles for Homestead spec.
pub fn homestead() -> &'static revm::precompile::Precompiles {
    static INSTANCE: OnceBox<revm::precompile::Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = revm::precompile::Precompiles::default();
        precompiles.extend([identity::FUN]);
        Box::new(precompiles)
    })
}

/// Returns precompiles for Byzantium spec.
pub fn byzantium() -> &'static revm::precompile::Precompiles {
    static INSTANCE: OnceBox<revm::precompile::Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let precompiles = homestead().clone();
        Box::new(precompiles)
    })
}

/// Returns precompiles for Istanbul spec.
pub fn istanbul() -> &'static revm::precompile::Precompiles {
    static INSTANCE: OnceBox<revm::precompile::Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let precompiles = byzantium().clone();
        Box::new(precompiles)
    })
}

/// Returns precompiles for Berlin spec.
pub fn berlin() -> &'static revm::precompile::Precompiles {
    static INSTANCE: OnceBox<revm::precompile::Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let precompiles = istanbul().clone();
        Box::new(precompiles)
    })
}

/// Returns precompiles for Cancun spec.
///
/// If the `c-kzg` feature is not enabled KZG Point Evaluation precompile will not be included,
/// effectively making this the same as Berlin.
pub fn cancun() -> &'static revm::precompile::Precompiles {
    static INSTANCE: OnceBox<revm::precompile::Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let precompiles = berlin().clone();
        Box::new(precompiles)
    })
}

/// Returns the precompiles for the latest spec.
pub fn latest() -> &'static revm::precompile::Precompiles {
    cancun()
}
