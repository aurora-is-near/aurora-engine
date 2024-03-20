use revm::precompile::{Bytes, Precompile, PrecompileResult, PrecompileWithAddress};

pub const FUN: PrecompileWithAddress =
    PrecompileWithAddress(crate::identity::ADDRESS, Precompile::Standard(identity_run));

/// Takes the input bytes, copies them, and returns it as the output.
///
/// See: <https://ethereum.github.io/yellowpaper/paper.pdf>
/// See: <https://etherscan.io/address/0000000000000000000000000000000000000004>
fn identity_run(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let gas_used = crate::identity::run(input, gas_limit)?;
    Ok((gas_used, input.clone()))
}
