use crate::json;
use crate::json::parse_json;
use crate::parameters::*;
use crate::prelude::*;
use crate::sdk;
use crate::types::*;
use borsh::{BorshDeserialize, BorshSerialize};

/// Basic actions for Migration
#[derive(BorshSerialize, BorshDeserialize)]
pub enum MigrationAction {
    // Add field with Value
    Add,
    // Rename key field
    RenameKey,
    // Update value for key field
    UpdateValue,
    // Rename key and update value
    UpdateKeyValue,
    // Remove key field
    Remove,
    // Remove only value for key field
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
    pub data: Vec<MigrationData>,
}

/// Migration function
// TODO: will be changed
impl From<json::JsonValue> for Migration {
    fn from(_v: json::JsonValue) -> Self {
        Self {
            action: MigrationAction::Add,
            data: vec![],
        }
    }
}

/// Migrate key fields and/or data value
/// Can be executed only contract itself.
pub fn migrate() {
    sdk::assert_private_call();
    let _args =
        Migration::from(parse_json(&sdk::read_input()).expect_utf8(ERR_FAILED_PARSE.as_bytes()));
    sdk::return_output(&"done".as_bytes());
}
