use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{Address, EthGas};
use evm::{Context, ExitError};

pub struct ECRecover;

impl ECRecover {
    pub const ADDRESS: Address = crate::secp256k1::ADDRESS;
}

impl Precompile for ECRecover {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(EthGas::new(
            crate::secp256k1::required_gas().map_err(Into::<ExitError>::into)?,
        ))
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let gas_limit = target_gas.unwrap_or(EthGas::new(u64::MAX));
        let (gas_used, output_data) =
            crate::secp256k1::run(input, gas_limit.as_u64()).map_err(Into::<ExitError>::into)?;
        Ok(PrecompileOutput::without_logs(
            EthGas::new(gas_used),
            output_data,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::new_context;
    use aurora_engine_types::{Vec, H256};

    fn ecverify(hash: H256, signature: &[u8], signer: Address) -> bool {
        matches!(  crate::secp256k1::ecrecover(hash, signature[0..crate::secp256k1::SIGNATURE_LENGTH].try_into().unwrap()), Ok(s) if s == signer)
    }

    #[test]
    fn test_ecverify() {
        let hash = H256::from_slice(
            &hex::decode("1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        );
        let signature =
            &hex::decode("b9f0bb08640d3c1c00761cdd0121209268f6fd3816bc98b9e6f3cc77bf82b69812ac7a61788a0fdc0e19180f14c945a8e1088a27d92a74dce81c0981fb6447441b")
                .unwrap();
        let signer = Address::try_from_slice(
            &hex::decode("1563915e194D8CfBA1943570603F7606A3115508").unwrap(),
        )
        .unwrap();
        assert!(ecverify(hash, signature, signer));
    }

    #[test]
    fn test_ecrecover() {
        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001b650acf9d3f5f0a2c799776a1254355d5f4061762a237396a99a0e0e3fc2bcd6729514a0dacb2e623ac4abd157cb18163ff942280db4d5caad66ddf941ba12e03").unwrap();
        let expected =
            hex::decode("000000000000000000000000c08b5542d177ac6686946920409741463a15dddb")
                .unwrap();

        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // out of gas
        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001b650acf9d3f5f0a2c799776a1254355d5f4061762a237396a99a0e0e3fc2bcd6729514a0dacb2e623ac4abd157cb18163ff942280db4d5caad66ddf941ba12e03").unwrap();

        let res = ECRecover.run(&input, Some(EthGas::new(2_999)), &new_context(), false);
        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // bad inputs
        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001a650acf9d3f5f0a2c799776a1254355d5f4061762a237396a99a0e0e3fc2bcd6729514a0dacb2e623ac4abd157cb18163ff942280db4d5caad66ddf941ba12e03").unwrap();
        let expected: Vec<u8> = Vec::new();

        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001b000000000000000000000000000000000000000000000000000000000000001b0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let expected: Vec<u8> = Vec::new();

        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001b0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001b").unwrap();
        let expected: Vec<u8> = Vec::new();

        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001bffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff000000000000000000000000000000000000000000000000000000000000001b").unwrap();
        let expected: Vec<u8> = Vec::new();

        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad000000000000000000000000000000000000000000000000000000000000001b000000000000000000000000000000000000000000000000000000000000001bffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();
        let expected: Vec<u8> = Vec::new();

        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }

    #[test]
    fn test_ecrecover_geth_tests() {
        let input = hex::decode("a8b53bdf3306a35a7103ab5504a0c9b492295564b6202b1942a84ef300107281000000000000000000000000000000000000000000000000000000000000001b307835653165303366353363653138623737326363623030393366663731663366353366356337356237346463623331613835616138623838393262346538621122334455667788991011121314151617181920212223242526272829303132").unwrap();
        let expected: Vec<u8> = Vec::new();
        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549").unwrap();
        let expected =
            hex::decode("000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b")
                .unwrap();
        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c100000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549").unwrap();
        let expected: Vec<u8> = Vec::new();
        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000001000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549").unwrap();
        let expected: Vec<u8> = Vec::new();
        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let input = hex::decode("18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000001000000000000000000000011c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549").unwrap();
        let expected: Vec<u8> = Vec::new();
        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }

    #[test]
    fn test_extra_input_length() {
        let input = hex::decode("18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549aabbccddeeff").unwrap();
        let expected =
            hex::decode("000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b")
                .unwrap();
        let res = ECRecover
            .run(&input, Some(EthGas::new(3_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }
}
