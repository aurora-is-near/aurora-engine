use super::prelude::*;

use core::convert::From;
use rjson::{Array, Null, Object, Value};
#[cfg(test)]
use std::collections::BTreeMap;

pub enum JsonValue {
    Null,
    Number(f64),
    Bool(bool),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
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
    OutOfRange(JsonOutOfRangeError),
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum JsonOutOfRangeError {
    U8,
    U64,
    U128,
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
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
                JsonValue::Number(n) => {
                    if n.is_sign_negative() || n.is_infinite() || n > &(u64::MAX as f64) {
                        Err(JsonError::OutOfRange(JsonOutOfRangeError::U64))
                    } else if n.is_nan() {
                        Err(JsonError::InvalidU64)
                    } else {
                        Ok(*n as u64)
                    }
                }
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
            JsonValue::Number(n) => {
                if n.is_sign_negative() || n > &(u8::MAX as f64) {
                    Err(JsonError::OutOfRange(JsonOutOfRangeError::U8))
                } else {
                    Ok(*n as u8)
                }
            }
            _ => Err(JsonError::InvalidU8),
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
            Self::OutOfRange(err) => err.as_ref(),
        }
    }
}

impl AsRef<[u8]> for JsonOutOfRangeError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::U8 => b"ERR_OUT_OF_RANGE_U8",
            Self::U64 => b"ERR_OUT_OF_RANGE_U64",
            Self::U128 => b"ERR_OUT_OF_RANGE_U128",
        }
    }
}

#[cfg(test)]
impl std::fmt::Debug for JsonError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "{}",
            std::str::from_utf8(self.as_ref()).unwrap()
        ))
    }
}

#[cfg(test)]
impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", *self))
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
            JsonValue::String(n) => {
                if let Ok(x) = n.parse::<u128>() {
                    Ok(x)
                } else if n.parse::<i128>().is_ok() {
                    Err(JsonError::OutOfRange(JsonOutOfRangeError::U128))
                } else {
                    Err(JsonError::InvalidU128)
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_type_string() {
        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let string_data = json.string("foo").ok().unwrap();
        assert_eq!(string_data, "abcd");

        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let err = json.string("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidString);

        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let err = json.string("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidString);

        let json = parse_json(r#"{"foo": ["abcd"]}"#.as_bytes()).unwrap();
        let err = json.string("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidString);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let err = json.string("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidString);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let err = json.string("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidString);

        let json = JsonValue::Null;
        let err = json.string("foo").unwrap_err();
        assert_eq!(err, JsonError::NotJsonType);
    }

    #[test]
    fn test_json_type_u64() {
        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let val = json.u64("foo").ok().unwrap();
        assert_eq!(val, 123);

        let json = parse_json(format!(r#"{{"foo": {} }}"#, u64::MAX).as_bytes()).unwrap();
        let val = json.u64("foo").ok().unwrap();
        assert_eq!(val, u64::MAX);

        let json = parse_json(r#"{"foo": 12.99}"#.as_bytes()).unwrap();
        // TODO [#176]: should fail since it is not a `u64`
        let val = json.u64("foo").ok().unwrap();
        assert_eq!(val, 12);

        let json = parse_json(format!(r#"{{"foo": {} }}"#, u128::MAX).as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::OutOfRange(JsonOutOfRangeError::U64));

        let json = parse_json(r#"{"foo": -123}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::OutOfRange(JsonOutOfRangeError::U64));

        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU64);

        let json = parse_json(r#"{"foo": "123"}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU64);

        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU64);

        let json = parse_json(r#"{"foo": [123]}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU64);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU64);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU64);

        let json = JsonValue::Null;
        let err = json.u64("foo").unwrap_err();
        assert_eq!(err, JsonError::NotJsonType);
    }

    #[test]
    fn test_json_type_u128() {
        let json = parse_json(r#"{"foo": "123"}"#.as_bytes()).unwrap();
        let val = json.u128("foo").ok().unwrap();
        assert_eq!(val, 123);

        let json = parse_json(r#"{"foo": "-123"}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::OutOfRange(JsonOutOfRangeError::U128));

        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::ExpectedStringGotNumber);

        let json = parse_json(r#"{"foo": 12.3}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::ExpectedStringGotNumber);

        let json = parse_json(r#"{"foo": "12.3"}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU128);

        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU128);

        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU128);

        let json = parse_json(r#"{"foo": ["123"]}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU128);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU128);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidU128);

        let json = JsonValue::Null;
        let err = json.u128("foo").unwrap_err();
        assert_eq!(err, JsonError::NotJsonType);
    }

    #[test]
    fn test_json_type_bool() {
        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let val = json.bool("foo").ok().unwrap();
        assert_eq!(val, true);

        let json = parse_json(r#"{"foo": false}"#.as_bytes()).unwrap();
        let val = json.bool("foo").ok().unwrap();
        assert_eq!(val, false);

        let json = parse_json(r#"{"foo": "true"}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": "false"}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": [true]}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": 12.3}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::InvalidBool);

        let json = JsonValue::Null;
        let err = json.bool("foo").unwrap_err();
        assert_eq!(err, JsonError::NotJsonType);
    }

    #[test]
    fn test_json_type_u8() {
        let json = JsonValue::from(123f64);
        let val = JsonValue::parse_u8(&json).ok().unwrap();
        assert_eq!(val, 123);

        let json = JsonValue::from(-1f64);
        let err = JsonValue::parse_u8(&json).unwrap_err();
        assert_eq!(err, JsonError::OutOfRange(JsonOutOfRangeError::U8));

        let json = JsonValue::from(256f64);
        let err = JsonValue::parse_u8(&json).unwrap_err();
        assert_eq!(err, JsonError::OutOfRange(JsonOutOfRangeError::U8));

        let json = JsonValue::from("abcd".to_string());
        let err = JsonValue::parse_u8(&json).unwrap_err();
        assert_eq!(err, JsonError::InvalidU8);
    }
}
