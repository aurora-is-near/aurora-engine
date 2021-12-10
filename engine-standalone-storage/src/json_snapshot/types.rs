use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonSnapshot {
    pub id: String,
    pub jsonrpc: String,
    pub result: JsonSnapshotResult,
}

impl JsonSnapshot {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let snapshot = serde_json::from_reader(reader)?;
        Ok(snapshot)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonSnapshotResult {
    /// Base 64 encoding of the block hash
    pub block_hash: String,
    pub block_height: u64,
    /// See https://github.com/near/nearcore/blob/2bc63c60afe202e7c78a67176a4e267b8c0fb48f/core/primitives/src/views.rs#L201-L202.
    pub proof: Vec<String>,
    pub values: Vec<JsonSnapshotValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonSnapshotValue {
    /// Base 64 encoding of the key
    pub key: String,
    /// See https://github.com/near/nearcore/blob/2bc63c60afe202e7c78a67176a4e267b8c0fb48f/core/primitives/src/views.rs#L201-L202.
    pub proof: Vec<String>,
    /// Base 64 encoding of the value
    pub value: String,
}
