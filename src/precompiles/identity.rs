use crate::precompiles::PrecompileResult;
use evm::ExitSucceed;

/// Identity precompile costs.
mod costs {
    /// The base cost of the operation.
    pub(super) const IDENTITY_BASE: u64 = 15;

    /// The cost per word.
    pub(super) const IDENTITY_PER_WORD: u64 = 3;
}

/// Takes the input bytes, copies them, and returns it as the output.
///
/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://etherscan.io/address/0000000000000000000000000000000000000004
pub(super) fn identity(input: &[u8], target_gas: Option<u64>) -> PrecompileResult {
    let cost = (input.len() + 31) as u64 / 32 * costs::IDENTITY_PER_WORD + costs::IDENTITY_BASE;
    super::check_gas(target_gas, cost)?;

    Ok((ExitSucceed::Returned, input.to_vec(), 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use evm::ExitError;

    #[test]
    fn test_identity() {
        let input = [0u8, 1, 2, 3];

        let expected = input[0..2].to_vec();
        let res = identity(&input[0..2], Some(18)).unwrap().1;
        assert_eq!(res, expected);

        let expected = input.to_vec();
        let res = identity(&input, Some(18)).unwrap().1;
        assert_eq!(res, expected);

        // gas fail
        let res = identity(&input[0..2], Some(17));
        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // larger input
        let input = [
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let res = identity(&input, Some(21)).unwrap().1;
        assert_eq!(res, input.to_vec());
    }
}
