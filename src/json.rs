use super::prelude::*;
use crate::sdk;

use crate::types::ERR_FAILED_PARSE;
use alloc::collections::BTreeMap;
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

pub struct JsonArray(Vec<JsonValue>);
pub struct JsonObject(BTreeMap<String, JsonValue>);

impl JsonValue {
    #[allow(dead_code)]
    pub fn string(&self, key: &str) -> Result<String, ()> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(())? {
                JsonValue::String(s) => Ok(s.into()),
                _ => Err(()),
            },
            _ => Err(()),
        }
    }

    #[allow(dead_code)]
    pub fn u64(&self, key: &str) -> Result<u64, ()> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(())? {
                JsonValue::Number(n) => Ok(*n as u64),
                _ => Err(()),
            },
            _ => Err(()),
        }
    }

    #[allow(dead_code)]
    pub fn u128(&self, key: &str) -> Result<u128, ()> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(())? {
                JsonValue::Number(n) => Ok(*n as u128),
                _ => Err(()),
            },
            _ => Err(()),
        }
    }

    #[allow(dead_code)]
    pub fn u128_string(&self, key: &str) -> Result<u128, ()> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(())? {
                JsonValue::String(s) => s.parse::<u128>().map_err(|_| ()),
                _ => Err(()),
            },
            _ => Err(()),
        }
    }

    #[allow(dead_code)]
    pub fn bool(&self, key: &str) -> Result<bool, ()> {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(())? {
                JsonValue::Bool(n) => Ok(*n),
                _ => Err(()),
            },
            _ => Err(()),
        }
    }

    #[allow(dead_code)]
    pub fn parse_u8(v: &JsonValue) -> u8 {
        match v {
            JsonValue::Number(n) => *n as u8,
            _ => sdk::panic_utf8(ERR_FAILED_PARSE.as_bytes()),
        }
    }

    #[allow(dead_code)]
    pub fn array<T, F>(&self, key: &str, call: F) -> Result<Vec<T>, ()>
    where
        F: FnMut(&JsonValue) -> T,
    {
        match self {
            JsonValue::Object(o) => match o.get(key).ok_or(())? {
                JsonValue::Array(arr) => Ok(arr.iter().map(call).collect()),
                _ => Err(()),
            },
            _ => Err(()),
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
