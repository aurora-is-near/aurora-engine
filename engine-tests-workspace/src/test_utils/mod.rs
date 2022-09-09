pub mod solidity;

use crate::prelude::U256;
use crate::runner::EvmContract;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::types::{Address, Wei};

pub(crate) fn str_to_account_id(account_id: &str) -> AccountId {
    use aurora_engine_types::str::FromStr;
    AccountId::from_str(account_id).unwrap()
}

pub(crate) async fn validate_address_balance_and_nonce(
    evm_contract: &EvmContract,
    address: Address,
    expected_balance: Wei,
    expected_nonce: U256,
) {
    assert_eq!(
        evm_contract.get_balance(address).await,
        expected_balance,
        "balance"
    );
    assert_eq!(
        evm_contract.get_nonce(address).await,
        expected_nonce,
        "nonce"
    );
}
