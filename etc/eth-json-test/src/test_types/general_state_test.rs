use std::fs::read_to_string;

// helper function to read file
pub fn read_file(path: String) -> String {
    read_to_string(path).unwrap()
}

use serde::Deserialize;
use std::collections::HashMap;

pub type GST = HashMap<String, GeneralStateTest>;

#[derive(Debug, Deserialize)]
pub struct GeneralStateTestJson {
    #[serde(flatten)]
    pub json: GST,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralStateTest {
    /// General information for the test
    #[serde(alias = "_info")]
    pub info: GeneralStateTestInfo,
    /// Result of the transaction
    pub env: GeneralStateTestEnv,
    /// Encoded TX bytes to feed to Aurora VM
    pub post: HashMap<String, serde_json::Value>,
    pub pre: HashMap<String, PresetAccount>,
    pub transaction: Transaction,
}

impl GeneralStateTest {
    pub fn new(path: String, test_name: String) -> Self {
        let json_str = read_file(path);
        let gst: GeneralStateTestJson = serde_json::from_str(&json_str).unwrap();
        let input = gst.json.get(&test_name).unwrap();
        GeneralStateTest {
            info: input.clone().info,
            env: input.clone().env,
            post: input.clone().post,
            pre: input.clone().pre,
            transaction: input.clone().transaction,
        }
    }

    pub fn info(&self) -> &GeneralStateTestInfo {
        &self.info
    }

    pub fn env(&self) -> &GeneralStateTestEnv {
        &self.env
    }

    pub fn post(&self, version: String) -> Vec<Post> {
        let value = self.post.get(&version).unwrap();
        let parse_post_result =
            |value: &serde_json::Value| -> Result<Vec<Post>, serde_json::Error> {
                return serde_json::from_value::<Vec<Post>>(value.clone());
            };
        let parsed = parse_post_result(value).unwrap();
        return parsed;
    }

    pub fn pre_account(&self, preset_account: String) -> &PresetAccount {
        &self.pre.get(&preset_account).unwrap()
    }

    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralStateTestInfo {
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

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralStateTestEnv {
    #[serde(alias = "currentBaseFee")]
    pub current_base_fee: String,
    #[serde(alias = "currentCoinbase")]
    pub current_coinbase: String,
    #[serde(alias = "currentDifficulty")]
    pub current_difficulty: String,
    #[serde(alias = "currentGasLimit")]
    pub current_gas_limit: String,
    #[serde(alias = "currentNumber")]
    pub current_number: String,
    #[serde(alias = "currentRandom")]
    pub current_random: String,
    #[serde(alias = "currentTimestamp")]
    pub current_timestamp: String,
    #[serde(alias = "previousHash")]
    pub previous_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PresetAccount {
    pub balance: String,
    pub code: String,
    pub nonce: String,
    pub storage: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Indexes {
    pub data: u64,
    pub gas: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Post {
    pub hash: String,
    pub indexes: Indexes,
    pub logs: String,
    pub txbytes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    pub data: Vec<String>,
    #[serde(alias = "gasLimit")]
    pub gas_limit: Vec<String>,
    #[serde(alias = "gasPrice")]
    pub gas_price: String,
    pub nonce: String,
    #[serde(alias = "secretKey")]
    pub secret_key: String,
    pub sender: String,
    pub to: String,
    pub value: Vec<String>,
}
