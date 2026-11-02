use aurora_engine_types::{account_id::AccountId, storage::EthConnectorStorageId};
use aurora_evm::ExitError;
use core::ops::RangeInclusive;

use crate::prelude::{
    Cow, U256, Vec,
    sdk::io::{IO, StorageIntermediate},
    storage::{KeyPrefix, bytes_to_key},
};

pub mod events;
pub mod exit_to_ethereum;
pub mod exit_to_near;

pub use exit_to_ethereum::ExitToEthereum;
pub use exit_to_near::ExitToNear;

const ERR_TARGET_TOKEN_NOT_FOUND: &str = "Target token not found";

fn validate_input_size(input: &[u8], allowed: RangeInclusive<usize>) -> Result<(), ExitError> {
    if !allowed.contains(&input.len()) {
        return Err(ExitError::Other(Cow::from("ERR_INVALID_INPUT")));
    }

    Ok(())
}

fn get_nep141_from_erc20<I: IO>(erc20_token: &[u8], io: &I) -> Result<AccountId, ExitError> {
    AccountId::try_from(
        io.read_storage(bytes_to_key(KeyPrefix::Erc20Nep141Map, erc20_token).as_slice())
            .map(|s| s.to_vec())
            .ok_or(ExitError::Other(Cow::Borrowed(ERR_TARGET_TOKEN_NOT_FOUND)))?,
    )
    .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_INVALID_NEP141_ACCOUNT")))
}

fn get_eth_connector_contract_account<I: IO>(io: &I) -> Result<AccountId, ExitError> {
    io.read_storage(&construct_contract_key(
        EthConnectorStorageId::EthConnectorAccount,
    ))
    .ok_or(ExitError::Other(Cow::Borrowed("ERR_KEY_NOT_FOUND")))
    .and_then(|x| {
        x.to_value()
            .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_DESERIALIZE")))
    })
}

fn construct_contract_key(suffix: EthConnectorStorageId) -> Vec<u8> {
    bytes_to_key(KeyPrefix::EthConnector, &[u8::from(suffix)])
}

fn parse_amount(input: &[u8]) -> Result<U256, ExitError> {
    let amount = U256::from_big_endian(input);

    if amount > U256::from(u128::MAX) {
        return Err(ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")));
    }

    Ok(amount)
}

fn to_u128(amount: U256) -> Result<u128, ExitError> {
    if amount > U256::from(u128::MAX) {
        return Err(ExitError::Other(Cow::from("ERR_INVALID_AMOUNT")));
    }

    Ok(amount.as_u128())
}

#[cfg(test)]
mod tests {
    use super::{exit_to_ethereum, exit_to_near, parse_amount, validate_input_size};
    use crate::prelude::sdk::types::near_account_to_evm_address;
    use aurora_engine_types::U256;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            exit_to_ethereum::ADDRESS,
            near_account_to_evm_address(b"exitToEthereum")
        );
        assert_eq!(
            exit_to_near::ADDRESS,
            near_account_to_evm_address(b"exitToNear")
        );
    }

    #[test]
    fn test_exit_signatures() {
        let exit_to_near = super::events::exit_to_near_schema();
        let exit_to_eth = super::events::exit_to_eth_schema();

        assert_eq!(
            exit_to_near.signature().0,
            super::events::EXIT_TO_NEAR_SIGNATURE.0
        );
        assert_eq!(
            exit_to_eth.signature().0,
            super::events::EXIT_TO_ETH_SIGNATURE.0
        );
    }

    #[test]
    fn test_check_invalid_input_lt_min() {
        let input = [0u8; 4];
        assert!(validate_input_size(&input, 10..=20).is_err());
    }

    #[test]
    fn test_check_invalid_input_gt_max() {
        let input = [1u8; 55];
        assert!(validate_input_size(&input, 10..=54).is_err());
    }

    #[test]
    fn test_check_valid_input() {
        let input = [1u8; 55];
        validate_input_size(&input, 10..=input.len()).unwrap();
        validate_input_size(&input, 0..=input.len()).unwrap();
    }

    #[test]
    #[should_panic(expected = "ERR_INVALID_AMOUNT")]
    fn test_exit_with_invalid_amount() {
        let input = (U256::from(u128::MAX) + 1).to_big_endian();
        parse_amount(input.as_slice()).unwrap();
    }

    #[test]
    fn test_exit_with_valid_amount() {
        let input = U256::from(u128::MAX).to_big_endian();
        assert_eq!(
            parse_amount(input.as_slice()).unwrap(),
            U256::from(u128::MAX)
        );
    }
}
