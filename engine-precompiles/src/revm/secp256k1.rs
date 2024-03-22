use revm::precompile::{Bytes, Precompile, PrecompileResult, PrecompileWithAddress};

pub const ECRECOVER: PrecompileWithAddress = PrecompileWithAddress(
    super::to_address(crate::secp256k1::ADDRESS),
    Precompile::Standard(secp256k1_run),
);

fn secp256k1_run(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let (gas_used, output_data) = crate::secp256k1::run(input, gas_limit)?;
    Ok((gas_used, output_data.into()))
}
