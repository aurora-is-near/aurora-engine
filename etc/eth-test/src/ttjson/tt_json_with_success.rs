use serde::Deserialize;
use std::collections::HashMap;
use crate::ttjson::read_file;

pub type TTOk = HashMap<String, TransactionTestOk>;

#[derive(Debug, Deserialize)]
pub struct TransactionTestJsonOk {
    #[serde(flatten)]
    pub json: TTOk,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionTestOk {
    /// General information for the test
    #[serde(alias = "_info")]
    pub info: TransactonTestInfo,
    /// Result of the transaction
    pub result: HashMap<String, TtResultOk>,
    /// Encoded TX bytes to feed to Aurora VM
    pub txbytes: String
}

impl TransactionTestOk{
    pub fn new(path: String, test_name: String) -> Self {
        let json_str = read_file(path);
        let tt: TransactionTestJsonOk = serde_json::from_str(&json_str).unwrap();
        let input = tt.json.get(&test_name).unwrap();
        TransactionTestOk {
            info: input.clone().info,
            result: input.clone().result,
            txbytes: input.clone().txbytes
        }
    }

    pub fn info(&self) -> &TransactonTestInfo {
        &self.info
    }

    pub fn result(&self, network: String) -> &TtResultOk {
        &self.result.get(&network).unwrap()
    }

    pub fn txbytes(&self) -> &String {
        &self.txbytes
    }
}


// TODO: set result for London hard fork only
#[derive(Debug, Clone, Deserialize)]
pub struct TtResultOk {
    /// Exception on expected error
    #[serde(alias = "hash")]
    pub hash: String,
    /// Consumed Gas in hexadecimal notation
    #[serde(alias = "intrinsicGas")]
    pub intrinsic_gas: String,
    #[serde(alias = "sender")]
    pub sender: String
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
