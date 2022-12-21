use crate::prelude::{BTreeMap, String, Vec};

use crate::errors;
use core::convert::From;
use rjson::{Array, Null, Object, Value};

#[derive(PartialEq)]
pub enum JsonValue {
    Null,
    F64(f64),
    I64(i64),
    U64(u64),
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
    ExpectedStringGotNumber,
    OutOfRange(JsonOutOfRangeError),
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum JsonOutOfRangeError {
    OutOfRangeU8,
    OutOfRangeU128,
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
                JsonValue::U64(n) => Ok(*n),
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
            JsonValue::U64(n) => match u8::try_from(*n) {
                Ok(v) => Ok(v),
                Err(_e) => Err(JsonError::OutOfRange(JsonOutOfRangeError::OutOfRangeU8)),
            },
            _ => Err(JsonError::InvalidU8),
        }
    }
}

impl AsRef<[u8]> for JsonError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::NotJsonType => errors::ERR_NOT_A_JSON_TYPE,
            Self::MissingValue => errors::ERR_JSON_MISSING_VALUE,
            Self::InvalidU8 => errors::ERR_FAILED_PARSE_U8,
            Self::InvalidU64 => errors::ERR_FAILED_PARSE_U64,
            Self::InvalidU128 => errors::ERR_FAILED_PARSE_U128,
            Self::InvalidBool => errors::ERR_FAILED_PARSE_BOOL,
            Self::InvalidString => errors::ERR_FAILED_PARSE_STRING,
            Self::ExpectedStringGotNumber => errors::ERR_EXPECTED_STRING_GOT_NUMBER,
            Self::OutOfRange(err) => err.as_ref(),
        }
    }
}

impl AsRef<[u8]> for JsonOutOfRangeError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::OutOfRangeU8 => errors::ERR_OUT_OF_RANGE_U8,
            Self::OutOfRangeU128 => errors::ERR_OUT_OF_RANGE_U128,
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
        JsonValue::F64(v)
    }
}

impl From<i64> for JsonValue {
    fn from(v: i64) -> Self {
        JsonValue::I64(v)
    }
}

impl From<u64> for JsonValue {
    fn from(v: u64) -> Self {
        JsonValue::U64(v)
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
                    Err(JsonError::OutOfRange(JsonOutOfRangeError::OutOfRangeU128))
                } else {
                    Err(JsonError::InvalidU128)
                }
            }
            JsonValue::F64(_) => Err(JsonError::ExpectedStringGotNumber),
            JsonValue::I64(_) => Err(JsonError::ExpectedStringGotNumber),
            JsonValue::U64(_) => Err(JsonError::ExpectedStringGotNumber),
            _ => Err(JsonError::InvalidU128),
        }
    }
}

impl core::fmt::Debug for JsonValue {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            JsonValue::Null => f.write_str("null"),
            JsonValue::String(v) => f.write_fmt(format_args!("\"{}\"", v)),
            JsonValue::F64(v) => f.write_fmt(format_args!("{}", v)),
            JsonValue::I64(v) => f.write_fmt(format_args!("{}", v)),
            JsonValue::U64(v) => f.write_fmt(format_args!("{}", v)),
            JsonValue::Bool(v) => f.write_fmt(format_args!("{}", v)),
            JsonValue::Array(arr) => {
                f.write_str("[")?;
                let mut items = arr.iter();
                if let Some(item) = items.next() {
                    f.write_fmt(format_args!("{:?}", item))?;
                }
                for item in items {
                    f.write_fmt(format_args!(", {:?}", item))?;
                }
                f.write_str("]")
            }
            JsonValue::Object(kvs) => {
                f.write_str("{")?;
                let mut pairs = kvs.iter();
                if let Some((key, value)) = pairs.next() {
                    f.write_fmt(format_args!("\"{}\": {:?}", key, value))?;
                }
                for (key, value) in pairs {
                    f.write_fmt(format_args!(", \"{}\": {:?}", key, value))?;
                }
                f.write_str("}")
            }
        }
    }
}

impl core::fmt::Display for JsonValue {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", *self))
    }
}

pub fn parse_json(data: &[u8]) -> Option<JsonValue> {
    let data_array: Vec<char> = data.iter().map(|b| char::from(*b)).collect::<Vec<_>>();
    let mut index = 0;
    rjson::parse::<JsonValue, JsonArray, JsonObject, JsonValue>(&data_array, &mut index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_all_types_fail_to_parse_missing_key() {
        let expected_err = std::str::from_utf8(errors::ERR_JSON_MISSING_VALUE).unwrap();
        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();

        let actual_err = json.string("missing_key").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let actual_err = json.bool("missing_key").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let actual_err = json.u64("missing_key").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let actual_err = json.u128("missing_key").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);
    }

    #[test]
    fn test_json_type_string() {
        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let string_data = json.string("foo").ok().unwrap();
        assert_eq!(string_data, "abcd");

        let expected_err = std::str::from_utf8(errors::ERR_FAILED_PARSE_STRING).unwrap();
        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let actual_err = json.string("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let actual_err = json.string("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": ["abcd"]}"#.as_bytes()).unwrap();
        let actual_err = json.string("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let actual_err = json.string("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let actual_err = json.string("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_NOT_A_JSON_TYPE).unwrap();
        let json = JsonValue::Null;
        let actual_err = json.string("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_json_type_u64_with_u128_value() {
        let _ = parse_json(format!(r#"{{"foo": {} }}"#, u128::MAX).as_bytes());
    }

    #[test]
    fn test_json_type_u64() {
        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let val = json.u64("foo").ok().unwrap();
        assert_eq!(val, 123);

        let json = parse_json(format!(r#"{{"foo": {} }}"#, u64::MAX).as_bytes()).unwrap();
        let val = json.u64("foo").ok().unwrap();
        assert_eq!(val, u64::MAX);

        let expected_err = std::str::from_utf8(errors::ERR_FAILED_PARSE_U64).unwrap();
        let json = parse_json(r#"{"foo": 12.99}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": -123}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": "123"}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": [123]}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_NOT_A_JSON_TYPE).unwrap();
        let json = JsonValue::Null;
        let actual_err = json.u64("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);
    }

    #[test]
    fn test_json_type_u128() {
        let json = parse_json(r#"{"foo": "123"}"#.as_bytes()).unwrap();
        let val = json.u128("foo").ok().unwrap();
        assert_eq!(val, 123);

        let expected_err =
            std::str::from_utf8(JsonOutOfRangeError::OutOfRangeU128.as_ref()).unwrap();
        let json = parse_json(r#"{"foo": "-123"}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_EXPECTED_STRING_GOT_NUMBER).unwrap();
        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": 12.3}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_FAILED_PARSE_U128).unwrap();
        let json = parse_json(r#"{"foo": "12.3"}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": ["123"]}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_NOT_A_JSON_TYPE).unwrap();
        let json = JsonValue::Null;
        let actual_err = json.u128("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);
    }

    #[test]
    fn test_json_type_bool() {
        let json = parse_json(r#"{"foo": true}"#.as_bytes()).unwrap();
        let val = json.bool("foo").ok().unwrap();
        assert!(val);

        let json = parse_json(r#"{"foo": false}"#.as_bytes()).unwrap();
        let val = json.bool("foo").ok().unwrap();
        assert!(!val);

        let expected_err = std::str::from_utf8(errors::ERR_FAILED_PARSE_BOOL).unwrap();
        let json = parse_json(r#"{"foo": "true"}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": "false"}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": [true]}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": 123}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": 12.3}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": "abcd"}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": {}}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let json = parse_json(r#"{"foo": null}"#.as_bytes()).unwrap();
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_NOT_A_JSON_TYPE).unwrap();
        let json = JsonValue::Null;
        let actual_err = json.bool("foo").unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);
    }

    #[test]
    fn test_json_type_u8() {
        let json = JsonValue::from(123_u64);
        let val = JsonValue::parse_u8(&json).ok().unwrap();
        assert_eq!(val, 123);

        let expected_err = std::str::from_utf8(errors::ERR_FAILED_PARSE_U8).unwrap();
        let json = JsonValue::from(-1_i64);
        let actual_err = JsonValue::parse_u8(&json).unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(JsonOutOfRangeError::OutOfRangeU8.as_ref()).unwrap();
        let json = JsonValue::from(256_u64);
        let actual_err = JsonValue::parse_u8(&json).unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);

        let expected_err = std::str::from_utf8(errors::ERR_FAILED_PARSE_U8).unwrap();
        let json = JsonValue::from("abcd".to_string());
        let actual_err = JsonValue::parse_u8(&json).unwrap_err();
        let actual_err = std::str::from_utf8(actual_err.as_ref()).unwrap();
        assert_eq!(actual_err, expected_err);
    }

    #[test]
    fn test_json_serialization() {
        // Test showing valid json (without trailing commas) is produced from the
        // `Display` impl on `JsonValue`.

        // empty object
        let object = JsonValue::Object(BTreeMap::new());
        assert_eq!(&format!("{}", object), "{}");

        // object with 1 field
        let object = JsonValue::Object(
            vec![("pi".to_string(), JsonValue::F64(std::f64::consts::PI))]
                .into_iter()
                .collect(),
        );
        assert_eq!(&format!("{}", object), "{\"pi\": 3.141592653589793}");

        // object with 2 fields
        let object = JsonValue::Object(
            vec![
                ("pi".to_string(), JsonValue::F64(std::f64::consts::PI)),
                ("Pie".to_string(), JsonValue::String("Apple".to_string())),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(
            &format!("{}", object),
            "{\"Pie\": \"Apple\", \"pi\": 3.141592653589793}"
        );

        // object with empty array
        let object = JsonValue::Object(
            vec![("empty".to_string(), JsonValue::Array(vec![]))]
                .into_iter()
                .collect(),
        );
        assert_eq!(&format!("{}", object), "{\"empty\": []}");

        // object with single element array
        let object = JsonValue::Object(
            vec![(
                "numbers".to_string(),
                JsonValue::Array(vec![JsonValue::U64(42)]),
            )]
            .into_iter()
            .collect(),
        );
        assert_eq!(&format!("{}", object), "{\"numbers\": [42]}");

        // object with two-element array
        let object = JsonValue::Object(
            vec![(
                "words".to_string(),
                JsonValue::Array(vec![
                    JsonValue::String("Hello".to_string()),
                    JsonValue::String("World".to_string()),
                ]),
            )]
            .into_iter()
            .collect(),
        );
        assert_eq!(
            &format!("{}", object),
            "{\"words\": [\"Hello\", \"World\"]}"
        );
    }
}
