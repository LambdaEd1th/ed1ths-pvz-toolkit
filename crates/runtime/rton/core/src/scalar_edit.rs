use serde_rton::{BinaryBlob, Rtid, Value, VarInt};
use std::fmt::Display;
use std::str::FromStr;

use crate::value_path::{replace_value_at_path, value_at_path};
use crate::{CoreError, Result};

pub fn scalar_edit_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("null".to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Int8(value) => Some(value.to_string()),
        Value::UInt8(value) => Some(value.to_string()),
        Value::Int16(value) => Some(value.to_string()),
        Value::UInt16(value) => Some(value.to_string()),
        Value::Int32(value) => Some(value.to_string()),
        Value::UInt32(value) => Some(value.to_string()),
        Value::Int64(value) => Some(value.to_string()),
        Value::UInt64(value) => Some(value.to_string()),
        Value::VarIntI32(value) => Some(value.0.to_string()),
        Value::VarIntU32(value) => Some(value.0.to_string()),
        Value::VarIntI64(value) => Some(value.0.to_string()),
        Value::VarIntU64(value) => Some(value.0.to_string()),
        Value::Float(value) => Some(value.to_string()),
        Value::Double(value) => Some(value.to_string()),
        Value::String(value) => Some(value.clone()),
        Value::Binary(value) => Some(value.to_string()),
        Value::Rtid(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

pub fn parse_scalar_edit(existing: &Value, text: &str) -> Result<Value> {
    let trimmed = text.trim();
    match existing {
        Value::Null => {
            if trimmed.eq_ignore_ascii_case("null") {
                Ok(Value::Null)
            } else {
                Err(invalid_scalar("null", "expected null"))
            }
        }
        Value::Bool(_) if trimmed.eq_ignore_ascii_case("true") => Ok(Value::Bool(true)),
        Value::Bool(_) if trimmed.eq_ignore_ascii_case("false") => Ok(Value::Bool(false)),
        Value::Bool(_) => Err(invalid_scalar("bool", "expected true or false")),
        Value::Int8(_) => parse_typed_scalar::<i8>(trimmed, "i8").map(Value::Int8),
        Value::UInt8(_) => parse_typed_scalar::<u8>(trimmed, "u8").map(Value::UInt8),
        Value::Int16(_) => parse_typed_scalar::<i16>(trimmed, "i16").map(Value::Int16),
        Value::UInt16(_) => parse_typed_scalar::<u16>(trimmed, "u16").map(Value::UInt16),
        Value::Int32(_) => parse_typed_scalar::<i32>(trimmed, "i32").map(Value::Int32),
        Value::UInt32(_) => parse_typed_scalar::<u32>(trimmed, "u32").map(Value::UInt32),
        Value::Int64(_) => parse_typed_scalar::<i64>(trimmed, "i64").map(Value::Int64),
        Value::UInt64(_) => parse_typed_scalar::<u64>(trimmed, "u64").map(Value::UInt64),
        Value::VarIntI32(_) => parse_typed_scalar::<i32>(trimmed, "varint i32")
            .map(|value| Value::VarIntI32(VarInt(value))),
        Value::VarIntU32(_) => parse_typed_scalar::<u32>(trimmed, "varint u32")
            .map(|value| Value::VarIntU32(VarInt(value))),
        Value::VarIntI64(_) => parse_typed_scalar::<i64>(trimmed, "varint i64")
            .map(|value| Value::VarIntI64(VarInt(value))),
        Value::VarIntU64(_) => parse_typed_scalar::<u64>(trimmed, "varint u64")
            .map(|value| Value::VarIntU64(VarInt(value))),
        Value::Float(_) => parse_typed_scalar::<f32>(trimmed, "f32").map(Value::Float),
        Value::Double(_) => parse_typed_scalar::<f64>(trimmed, "f64").map(Value::Double),
        Value::String(_) => Ok(Value::String(text.to_string())),
        Value::Binary(_) => BinaryBlob::from_str(trimmed)
            .map(Value::Binary)
            .map_err(|error| invalid_scalar("binary", error)),
        Value::Rtid(_) => Rtid::from_str(trimmed)
            .map(Value::Rtid)
            .map_err(|error| invalid_scalar("rtid", error)),
        Value::Array(_) => Err(CoreError::NonEditableValue("array")),
        Value::Object(_) => Err(CoreError::NonEditableValue("object")),
    }
}

pub fn edit_value_at_path(root: &mut Value, path: &str, text: &str) -> Result<()> {
    let replacement = {
        let current = value_at_path(root, path)?;
        parse_scalar_edit(current, text)?
    };
    replace_value_at_path(root, path, replacement)
}

fn parse_typed_scalar<T>(text: &str, kind: &'static str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    text.parse::<T>()
        .map_err(|error| invalid_scalar(kind, error))
}

fn invalid_scalar(kind: &'static str, message: impl Display) -> CoreError {
    CoreError::InvalidScalarValue {
        kind,
        message: message.to_string(),
    }
}
