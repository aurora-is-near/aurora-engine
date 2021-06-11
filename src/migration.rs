use crate::json;
use crate::prelude::*;
use crate::sdk;
use crate::types::{ExpectUtf8, ERR_FAILED_PARSE};

/// Basic actions for Migration
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
pub struct MigrationData {
    /// Field contains: 1. Current field (or new field), 2. Old field
    pub field: (String, Option<String>),
    /// Key prefix
    pub prefix: String,
    /// Value for migration
    pub value: Option<Vec<u8>>,
}

/// Migration data and/or fields
pub struct Migration {
    /// Migration action
    pub action: MigrationAction,
    /// Migration data
    pub data: Vec<MigrationData>,
}

/// Basic migdation data set
pub struct MigrationArgs(Vec<Migration>);

impl From<json::JsonValue> for MigrationData {
    fn from(v: json::JsonValue) -> Self {
        let new_field = v
            .string("new_field")
            .expect_utf8(&ERR_FAILED_PARSE.as_bytes());
        let old_field = v.string("old_field").ok();
        let prefix = v.string("prefix").expect_utf8(&ERR_FAILED_PARSE.as_bytes());
        let value = v
            .string("value")
            .map_or(None, |v| Some(v.as_bytes().to_vec()));
        Self {
            field: (new_field, old_field),
            prefix,
            value,
        }
    }
}

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
            action: MigrationAction::from(
                v.string("action").expect_utf8(&ERR_FAILED_PARSE.as_bytes()),
            ),
            data: v
                .array("data", MigrationData::from)
                .expect_utf8(&ERR_FAILED_PARSE.as_bytes()),
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
        sdk::log_utf8(format!("{:?}", data.len()).as_bytes());
        Self(data)
    }
}

/// Migrate key fields and/or data value
/// Can be executed only contract itself.
pub fn migrate(args: MigrationArgs) {
    args.0.iter().for_each(|m| match m.action {
        MigrationAction::Add => {
            m.data.iter().for_each(|data| {
                let prefix = data.prefix.clone();
                let new_field = data.field.0.clone();
                let key = format!("{}{}", prefix, new_field);
                if sdk::storage_has_key(key.as_bytes()) {
                    sdk::panic_utf8("AddAction: key already exists".as_bytes());
                }
                if data.value.is_none() {
                    sdk::panic_utf8("AddAction: value doesn't set".as_bytes());
                }
                sdk::write_storage(key.as_bytes(), &data.value.clone().unwrap())
            });
        }
        MigrationAction::RenameKey => {}
        MigrationAction::UpdateKeyValue => {}
        MigrationAction::UpdateValue => {}
        MigrationAction::Remove => {}
        MigrationAction::RemoveValue => {}
    });
    let _ = sdk::storage_has_key("".as_bytes());
}
