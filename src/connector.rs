use crate::fungible_token::*;
use crate::types::*;

#[allow(dead_code)]
pub const CONTRACT_NAME_KEY: &str = "EthConnector";
pub const CONTRACT_FT_KEY: &str = "EthConnector.ft";
pub const NO_DEPOSIT: Balance = 0;
#[allow(dead_code)]
const GAS_FOR_FINISH_DEPOSIT: Gas = 10_000_000_000_000;
#[allow(dead_code)]
const GAS_FOR_VERIFY_LOG_ENTRY: Gas = 40_000_000_000_000;

#[allow(dead_code)]
pub struct EthConnectorContract {
    contract: EthConnector,
    ft: FungibleToken,
}
