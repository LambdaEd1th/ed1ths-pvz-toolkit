use serde_rton::{BinaryBlob, Rtid, Value};
use std::io::{self, Write};
use std::str::FromStr;

use crate::{CoreError, DecodedDocument, Result, TextFormat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRender {
    pub text: String,
    pub truncated: bool,
}

pub fn parse_text(text: &str, format: TextFormat) -> Result<DecodedDocument> {
    let mut value = match format {
        TextFormat::Json => parse_editor_json(text)?,
        TextFormat::Yaml if is_empty_yaml_document(text) => Value::Object(Vec::new()),
        TextFormat::Yaml => serde_yaml::from_str::<Value>(text)?,
        TextFormat::Toml => toml::from_str::<Value>(text)?,
    };
    normalize_editor_value(&mut value);
    Ok(DecodedDocument::new(value, false, None))
}

pub fn value_to_text(value: &Value, format: TextFormat) -> Result<String> {
    if let Some(text) = empty_object_text(value, format) {
        return Ok(text.to_string());
    }

    match format {
        TextFormat::Json => {
            ensure_json_safe_value(value)?;
            serde_json::to_string_pretty(value).map_err(CoreError::Json)
        }
        TextFormat::Yaml => serde_yaml::to_string(value).map_err(CoreError::Yaml),
        TextFormat::Toml => toml::to_string_pretty(value).map_err(CoreError::TomlSer),
    }
}

pub fn value_to_text_limited(
    value: &Value,
    format: TextFormat,
    max_bytes: usize,
) -> Result<TextRender> {
    if let Some(text) = empty_object_text(value, format) {
        return Ok(limit_text(text.to_string(), max_bytes));
    }

    match format {
        TextFormat::Json => value_to_json_limited(value, max_bytes),
        TextFormat::Yaml => {
            let text = serde_yaml::to_string(value).map_err(CoreError::Yaml)?;
            Ok(limit_text(text, max_bytes))
        }
        TextFormat::Toml => {
            let text = toml::to_string_pretty(value).map_err(CoreError::TomlSer)?;
            Ok(limit_text(text, max_bytes))
        }
    }
}

fn is_empty_yaml_document(text: &str) -> bool {
    matches!(text.trim(), "" | "---" | "...")
}

fn empty_object_text(value: &Value, format: TextFormat) -> Option<&'static str> {
    let Value::Object(entries) = value else {
        return None;
    };
    if !entries.is_empty() {
        return None;
    }

    Some(match format {
        TextFormat::Json => "{}",
        TextFormat::Yaml => "---\n",
        TextFormat::Toml => "\n",
    })
}

fn value_to_json_limited(value: &Value, max_bytes: usize) -> Result<TextRender> {
    ensure_json_safe_value(value)?;
    let mut writer = LimitedStringWriter::new(max_bytes);
    match serde_json::to_writer_pretty(&mut writer, value) {
        Ok(()) => Ok(writer.finish(false)),
        Err(_error) if writer.truncated => Ok(writer.finish(true)),
        Err(error) => Err(CoreError::Json(error)),
    }
}

fn limit_text(text: String, max_bytes: usize) -> TextRender {
    if text.len() <= max_bytes {
        return TextRender {
            text,
            truncated: false,
        };
    }

    let end = text
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    TextRender {
        text: text[..end].to_string(),
        truncated: true,
    }
}

struct LimitedStringWriter {
    output: String,
    max_bytes: usize,
    truncated: bool,
}

impl LimitedStringWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            output: String::new(),
            max_bytes,
            truncated: false,
        }
    }

    fn finish(self, truncated: bool) -> TextRender {
        TextRender {
            text: self.output,
            truncated: truncated || self.truncated,
        }
    }
}

impl Write for LimitedStringWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.output.len() >= self.max_bytes {
            self.truncated = true;
            return Err(io::Error::other("text render limit reached"));
        }

        let remaining = self.max_bytes - self.output.len();
        if bytes.len() <= remaining {
            self.output.push_str(&String::from_utf8_lossy(bytes));
            return Ok(bytes.len());
        }

        let take = bytes[..remaining]
            .iter()
            .rposition(|byte| (*byte & 0b1100_0000) != 0b1000_0000)
            .unwrap_or(0);
        self.output
            .push_str(&String::from_utf8_lossy(&bytes[..take]));
        self.truncated = true;
        Err(io::Error::other("text render limit reached"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn parse_editor_json(json: &str) -> Result<Value> {
    match serde_json::from_str::<Value>(json) {
        Ok(mut value) => {
            normalize_editor_value(&mut value);
            Ok(value)
        }
        Err(primary_error) => {
            let json_value = serde_json::from_str::<serde_json::Value>(json)
                .map_err(|_| CoreError::Json(primary_error))?;
            json_value_to_rton(json_value).map_err(CoreError::Rton)
        }
    }
}

fn json_value_to_rton(value: serde_json::Value) -> std::result::Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Value::Rtid(Rtid::Null)),
        serde_json::Value::Bool(value) => Ok(Value::Bool(value)),
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(Value::new_int(value))
            } else if let Some(value) = number.as_u64() {
                Ok(Value::new_uint(value))
            } else if let Some(value) = number.as_f64() {
                Ok(Value::Double(value))
            } else {
                Err(format!("Unsupported JSON number: {number}"))
            }
        }
        serde_json::Value::String(text) => {
            if text.starts_with("RTID(") {
                Rtid::from_str(&text)
                    .map(Value::Rtid)
                    .map_err(|error| error.to_string())
            } else if text.starts_with("$BINARY(") {
                BinaryBlob::from_str(&text)
                    .map(Value::Binary)
                    .map_err(|error| error.to_string())
            } else {
                Ok(Value::String(text))
            }
        }
        serde_json::Value::Array(items) => items
            .into_iter()
            .map(json_value_to_rton)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map(Value::Array),
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| json_value_to_rton(value).map(|value| (key, value)))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map(Value::Object),
    }
}

fn normalize_editor_value(value: &mut Value) {
    match value {
        Value::String(text) if text.starts_with("$BINARY(") => {
            if let Ok(blob) = BinaryBlob::from_str(text) {
                *value = Value::Binary(blob);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_editor_value(item);
            }
        }
        Value::Object(entries) => {
            for (_, item) in entries {
                normalize_editor_value(item);
            }
        }
        _ => {}
    }
}

fn ensure_json_safe_value(value: &Value) -> Result<()> {
    match value {
        Value::Float(value) if !value.is_finite() => {
            Err(CoreError::NonFiniteJson(describe_non_finite(*value as f64)))
        }
        Value::Double(value) if !value.is_finite() => {
            Err(CoreError::NonFiniteJson(describe_non_finite(*value)))
        }
        Value::Array(items) => {
            for item in items {
                ensure_json_safe_value(item)?;
            }
            Ok(())
        }
        Value::Object(entries) => {
            for (_, item) in entries {
                ensure_json_safe_value(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn describe_non_finite(value: f64) -> &'static str {
    if value.is_nan() {
        "NaN"
    } else if value.is_sign_negative() {
        "-Infinity"
    } else {
        "Infinity"
    }
}
