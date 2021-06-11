use crate::json;
use crate::prelude::*;
use crate::sdk;
use crate::types::{ExpectUtf8, ERR_FAILED_PARSE};
use borsh::{BorshDeserialize, BorshSerialize};

/// Basic actions for Migration
#[derive(BorshSerialize, BorshDeserialize)]
pub enum MigrationAction {
    /// Add field with Value
    Add,
    /// Rename key field
    RenameKey,
    /// Update value for key field
    UpdateValue,
    /// Rename key and update value
    UpdateKeyValue,
    /// Remove key field
    Remove,
    /// Remove only value for key field
    RemoveValue,
}

/// Migration data/rules
#[derive(BorshSerialize, BorshDeserialize)]
pub struct MigrationData {
    /// Field contains: 1. Current field (ot new field), 2. Old field
    pub field: (String, Option<String>),
    /// Key prefix
    pub prefix: String,
    /// Value for migration
    pub value: Option<Vec<u8>>,
}

/// Migration data and/or fields
// TODO: will be changed
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Migration {
    /// Migration action
    pub action: MigrationAction,
    /// Migration data
    pub data: Vec<MigrationData>,
}

/// Basic migdation data set
pub struct MigrationArgs(Vec<Migration>);

impl From<String> for MigrationAction {
    fn from(v: String) -> Self {
        match v.as_str() {
            "Add" => MigrationAction::Add,
            "UpdateKeyValue" => MigrationAction::UpdateKeyValue,
            "UpdateValue" => MigrationAction::UpdateValue,
            "Remove" => MigrationAction::Remove,
            "RemoveValue" => MigrationAction::RemoveValue,
            "RenameKey" => MigrationAction::RenameKey,
            _ => sdk::panic_utf8(ERR_FAILED_PARSE.as_bytes()),
        }
    }
}

impl From<json::JsonValue> for Migration {
    fn from(v: json::JsonValue) -> Self {
        Self {
            action: {
                let s = v.string("action").expect_utf8(&ERR_FAILED_PARSE.as_bytes());
                MigrationAction::from(s)
            },
            data: vec![],
        }
    }
}

// TODO: will be changed
impl From<json::JsonValue> for MigrationArgs {
    fn from(v: json::JsonValue) -> Self {
        let data: Vec<Migration> = v
            .array_objects()
            .expect_utf8(ERR_FAILED_PARSE.as_bytes())
            .iter()
            .map(|val| Migration::from(val.clone()))
            .collect();
        //sdk::log_utf8(format!("{:?}", x.len()).as_bytes());
        Self(data)
    }
}

/// Migrate key fields and/or data value
/// Can be executed only contract itself.
pub fn migrate(_args: MigrationArgs) {
    let _ = sdk::storage_has_key("".as_bytes());
}
