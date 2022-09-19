use serde::Deserialize;
use std::collections::HashMap;

// TODO: change the parsing data structure to be more efficient with struct methods

// TODO: make TransactionTestJSON to AddressLessThan20 for identifying each test json file

#[derive(Debug, Deserialize)]
pub struct TransactionTestJson {
    #[serde(flatten)]
    AddressLessThan20: HashMap<String, TransactionTest>,
}

#[derive(Debug, Deserialize)]
struct TransactionTest {
    /// General information for the test
    #[serde(alias = "_info")]
    pub info: TransactonTestInfo,
    /// Result of the transaction
    pub result: HashMap<String, Result>,
    /// Encoded TX bytes to feed to Aurora VM
    pub txbytes: String
}

// TODO: set result for London hard fork only
#[derive(Debug, Deserialize)]
struct Result {
    /// Exception on expected error
    #[serde(alias = "exception")]
    pub exception: String,
    /// Consumed Gas in hexadecimal notation
    #[serde(alias = "intrinsicGas")]
    pub intrinsic_gas: String
}

#[derive(Debug, Deserialize)]
struct TransactonTestInfo {
    /// Comment for explanation
    pub comment : String,
    /// Filling RPC server specification
    #[serde(alias = "filling-rpc-server")]
    pub filling_rpc_server : String,
    /// Filling Tool Version
    #[serde(alias = "filling-tool-version")]
    pub filling_tool_version : String,
    /// Generated Test Hash, hash from test object
    #[serde(alias = "generatedTestHash")]
    pub generated_test_hash : String,
    /// lllc version
    pub lllcversion : String,
    /// Source within the test repository of ethereum/tests
    pub source : String,
    /// Source hash from the test repository of ethereum/tests
    #[serde(alias = "sourceHash")]
    pub source_hash : String
}


