use std::fs::read_to_string;

// helper function to read file
pub fn read_file(path: String) -> String {
    return read_to_string(path).unwrap();
}

use serde::Deserialize;
use std::collections::HashMap;

pub type TT = HashMap<String, TransactionTest>;

#[derive(Debug, Deserialize)]
pub struct TransactionTestJson {
    #[serde(flatten)]
    pub json: TT,
}

#[derive(Debug, Clone, Deserialize)]
pub enum TtResult {
    TtResultErr {
        /// Exception on expected error
        #[serde(alias = "exception")]
        exception: String,
        /// Consumed Gas in hexadecimal notation
        #[serde(alias = "intrinsicGas")]
        intrinsic_gas: String,
    },
    TtResultOk {
        /// Exception on expected error
        #[serde(alias = "hash")]
        hash: String,
        /// Consumed Gas in hexadecimal notation
        #[serde(alias = "intrinsicGas")]
        intrinsic_gas: String,
        #[serde(alias = "sender")]
        sender: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionTest {
    /// General information for the test
    #[serde(alias = "_info")]
    pub info: TransactonTestInfo,
    /// Result of the transaction
    pub result: HashMap<String, serde_json::Value>,
    /// Encoded TX bytes to feed to Aurora VM
    pub txbytes: String,
}

impl TransactionTest {
    pub fn new(path: String, test_name: String) -> Self {
        let json_str = read_file(path);
        let tt: TransactionTestJson = serde_json::from_str(&json_str).unwrap();
        let input = tt.json.get(&test_name).unwrap();
        TransactionTest {
            info: input.clone().info,
            result: input.clone().result,
            txbytes: input.clone().txbytes,
        }
    }

    pub fn info(&self) -> &TransactonTestInfo {
        &self.info
    }

    pub fn result(&self, network: String) -> TtResult {
        let value = self.result.get(&network).unwrap();
        let parse_ok_result = |value: &serde_json::Value| -> Result<TtResultOk, serde_json::Error> {
            return serde_json::from_value::<TtResultOk>(value.clone());
        };
        let parsed = match parse_ok_result(value) {
            Ok(result) => TtResult::TtResultOk {
                hash: result.hash,
                intrinsic_gas: result.intrinsic_gas,
                sender: result.sender,
            },
            Err(_) => {
                let result: TtResultErr = serde_json::from_value(value.clone()).unwrap();
                TtResult::TtResultErr {
                    exception: result.exception,
                    intrinsic_gas: result.intrinsic_gas,
                }
            }
        };
        return parsed;
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
    pub sender: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct TtResultErr {
    /// Exception on expected error
    #[serde(alias = "exception")]
    pub exception: String,
    /// Consumed Gas in hexadecimal notation
    #[serde(alias = "intrinsicGas")]
    pub intrinsic_gas: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactonTestInfo {
    /// Comment for explanation
    pub comment: String,
    /// Filling RPC server specification
    #[serde(alias = "filling-rpc-server")]
    pub filling_rpc_server: String,
    /// Filling Tool Version
    #[serde(alias = "filling-tool-version")]
    pub filling_tool_version: String,
    /// Generated Test Hash, hash from test object
    #[serde(alias = "generatedTestHash")]
    pub generated_test_hash: String,
    /// lllc version
    pub lllcversion: String,
    /// Source within the test repository of ethereum/tests
    pub source: String,
    /// Source hash from the test repository of ethereum/tests
    #[serde(alias = "sourceHash")]
    pub source_hash: String,
}
