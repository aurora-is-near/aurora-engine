use serde::Deserialize;
use std::collections::HashMap;
use crate::test_utils::read_file;
use std::path::Path;



#[derive(Debug, Clone, Deserialize)]
pub struct AddressLessThan20 {
    pub info: TransactonTestInfo,
    pub result: HashMap<String, Result>,
    pub txbytes: String
}

impl AddressLessThan20 {
    pub fn new() -> Self {
        let json_str = read_file("TransactionTests/ttAddress/AddressLessThan20.json".to_string());
        let tt: TransactionTestJson = serde_json::from_str(&json_str).unwrap();
        let input = tt.AddressLessThan20.get("AddressLessThan20").unwrap();
        AddressLessThan20 {
            info: input.clone().info,
            result: input.clone().result,
            txbytes: input.clone().txbytes
        }
    }

    pub fn info(&self) -> &TransactonTestInfo {
        &self.info
    }

    pub fn result(&self, network: String) -> &Result {
        &self.result.get(&network).unwrap()
    }

    pub fn txbytes(&self) -> &String {
        &self.txbytes
    }
}

//// JSON parsing type

#[derive(Debug, Deserialize)]
pub struct TransactionTestJson {
    #[serde(flatten)]
    pub AddressLessThan20: HashMap<String, TransactionTest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionTest {
    /// General information for the test
    #[serde(alias = "_info")]
    pub info: TransactonTestInfo,
    /// Result of the transaction
    pub result: HashMap<String, Result>,
    /// Encoded TX bytes to feed to Aurora VM
    pub txbytes: String
}

// TODO: set result for London hard fork only
#[derive(Debug, Clone, Deserialize)]
pub struct Result {
    /// Exception on expected error
    #[serde(alias = "exception")]
    pub exception: String,
    /// Consumed Gas in hexadecimal notation
    #[serde(alias = "intrinsicGas")]
    pub intrinsic_gas: String
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactonTestInfo {
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


