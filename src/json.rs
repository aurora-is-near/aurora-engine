use super::prelude::*;

use core::convert::From;
use rjson::{Array, Null, Object, Value};

pub enum JsonValue {
    Null,
    Number(f64),
    Bool(bool),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

#[derive(Debug)]
pub enum JsonError {
    NotJsonType,
    MissingValue,
    InvalidU8,
    InvalidU64,
    InvalidU128,
    InvalidBool,
    InvalidString,
    InvalidArray,
    ExpectedStringGotNumber,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidAccountId,
}

pub struct JsonArray(Vec<JsonValue>);
pub struct JsonObject(BTreeMap<String, JsonValue>);

impl JsonValue {
    #[allow(dead_code)]
    pub fn string(&self, key: &str) -> Result<String, JsonError> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(JsonError::MissingValue)? {
                JsonValue::String(s) => Ok(s.into()),
                _ => Err(JsonError::InvalidString),
            },
            _ => Err(JsonError::NotJsonType),
        }
    }

    #[allow(dead_code)]
    pub fn u64(&self, key: &str) -> Result<u64, JsonError> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(JsonError::MissingValue)? {
                JsonValue::Number(n) => Ok(*n as u64),
                _ => Err(JsonError::InvalidU64),
            },
            _ => Err(JsonError::NotJsonType),
        }
    }

    #[allow(dead_code)]
    pub fn u128(&self, key: &str) -> Result<u128, JsonError> {
        match self {
            JsonValue::Object(o) => o.get(key).ok_or(JsonError::MissingValue)?.try_into(),
            _ => Err(JsonError::NotJsonType),
        }
    }

    #[allow(dead_code)]
    pub fn bool(&self, key: &str) -> Result<bool, JsonError> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(JsonError::MissingValue)? {
                JsonValue::Bool(n) => Ok(*n),
                _ => Err(JsonError::InvalidBool),
            },
            _ => Err(JsonError::NotJsonType),
        }
    }

    #[allow(dead_code)]
    pub fn parse_u8(v: &JsonValue) -> Result<u8, JsonError> {
        match v {
            JsonValue::Number(n) => Ok(*n as u8),
            _ => Err(JsonError::InvalidU8),
        }
    }

    #[allow(dead_code)]
    pub fn array<T, F>(&self, key: &str, call: F) -> Result<Vec<T>, JsonError>
    where
        F: FnMut(&JsonValue) -> T,
    {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(JsonError::MissingValue)? {
                JsonValue::Array(arr) => Ok(arr.iter().map(call).collect()),
                _ => Err(JsonError::InvalidArray),
            },
            _ => Err(JsonError::NotJsonType),
        }
    }
}

impl AsRef<[u8]> for JsonError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::NotJsonType => b"ERR_NOT_A_JSON_TYPE",
            Self::MissingValue => b"ERR_JSON_MISSING_VALUE",
            Self::InvalidU8 => b"ERR_FAILED_PARSE_U8",
            Self::InvalidU64 => b"ERR_FAILED_PARSE_U64",
            Self::InvalidU128 => b"ERR_FAILED_PARSE_U128",
            Self::InvalidBool => b"ERR_FAILED_PARSE_BOOL",
            Self::InvalidString => b"ERR_FAILED_PARSE_STRING",
            Self::InvalidArray => b"ERR_FAILED_PARSE_ARRAY",
            Self::ExpectedStringGotNumber => b"ERR_EXPECTED_STRING_GOT_NUMBER",
        }
    }
}

impl Array<JsonValue, JsonObject, JsonValue> for JsonArray {
    fn new() -> Self {
        JsonArray(Vec::new())
    }
    fn push(&mut self, v: JsonValue) {
        self.0.push(v)
    }
}

impl Object<JsonValue, JsonArray, JsonValue> for JsonObject {
    fn new<'b>() -> Self {
        JsonObject(BTreeMap::new())
    }
    fn insert(&mut self, k: String, v: JsonValue) {
        self.0.insert(k, v);
    }
}

impl Null<JsonValue, JsonArray, JsonObject> for JsonValue {
    fn new() -> Self {
        JsonValue::Null
    }
}

impl Value<JsonArray, JsonObject, JsonValue> for JsonValue {}

impl From<f64> for JsonValue {
    fn from(v: f64) -> Self {
        JsonValue::Number(v)
    }
}

impl From<bool> for JsonValue {
    fn from(v: bool) -> Self {
        JsonValue::Bool(v)
    }
}

impl From<String> for JsonValue {
    fn from(v: String) -> Self {
        JsonValue::String(v)
    }
}

impl From<JsonArray> for JsonValue {
    fn from(v: JsonArray) -> Self {
        JsonValue::Array(v.0)
    }
}

impl From<JsonObject> for JsonValue {
    fn from(v: JsonObject) -> Self {
        JsonValue::Object(v.0)
    }
}

impl TryFrom<&JsonValue> for u128 {
    type Error = JsonError;

    fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
        match value {
            JsonValue::String(n) => Ok(n.parse::<u128>().map_err(|_| JsonError::InvalidU128)?),
            JsonValue::Number(_) => Err(JsonError::ExpectedStringGotNumber),
            _ => Err(JsonError::InvalidU128),
        }
    }
}

impl core::fmt::Debug for JsonValue {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self {
            JsonValue::Null => f.write_str("null"),
            JsonValue::String(ref v) => f.write_fmt(format_args!("\"{}\"", v)),
            JsonValue::Number(ref v) => f.write_fmt(format_args!("{}", v)),
            JsonValue::Bool(ref v) => f.write_fmt(format_args!("{}", v)),
            JsonValue::Array(ref v) => f.write_fmt(format_args!("{:?}", v)),
            JsonValue::Object(ref v) => f.write_fmt(format_args!("{:#?}", v)),
        }
    }
}

impl core::fmt::Display for JsonValue {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", *self))
    }
}

#[allow(dead_code)]
pub fn parse_json(data: &[u8]) -> Option<JsonValue> {
    let data_array: Vec<char> = data.iter().map(|b| *b as char).collect::<Vec<_>>();
    let mut index = 0;
    rjson::parse::<JsonValue, JsonArray, JsonObject, JsonValue>(&*data_array, &mut index)
}
