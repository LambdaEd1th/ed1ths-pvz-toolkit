use serde::{Deserialize, Serialize};
use serde_rton::Value;

use crate::BinaryEncoding;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecodedDocument {
    pub value: Value,
    pub encrypted_source: bool,
    #[serde(default)]
    pub encoding_source: BinaryEncoding,
    pub byte_len: Option<usize>,
    pub stats: ValueStats,
}

impl DecodedDocument {
    pub fn new(value: Value, encrypted_source: bool, byte_len: Option<usize>) -> Self {
        Self::new_with_source_encoding(value, encrypted_source, BinaryEncoding::Standard, byte_len)
    }

    pub fn new_with_source_encoding(
        value: Value,
        encrypted_source: bool,
        encoding_source: BinaryEncoding,
        byte_len: Option<usize>,
    ) -> Self {
        let stats = ValueStats::from_value(&value);
        Self {
            value,
            encrypted_source,
            encoding_source,
            byte_len,
            stats,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueStats {
    pub nodes: usize,
    pub objects: usize,
    pub arrays: usize,
    pub strings: usize,
    pub numbers: usize,
    pub bools: usize,
    pub nulls: usize,
    pub binaries: usize,
    pub rtids: usize,
    pub max_depth: usize,
}

impl ValueStats {
    pub fn from_value(value: &Value) -> Self {
        let mut stats = ValueStats::default();
        stats.visit(value, 0);
        stats
    }

    fn visit(&mut self, value: &Value, depth: usize) {
        self.nodes += 1;
        self.max_depth = self.max_depth.max(depth);

        match value {
            Value::Null => self.nulls += 1,
            Value::Bool(_) => self.bools += 1,
            Value::Int8(_)
            | Value::UInt8(_)
            | Value::Int16(_)
            | Value::UInt16(_)
            | Value::Int32(_)
            | Value::UInt32(_)
            | Value::Int64(_)
            | Value::UInt64(_)
            | Value::VarIntI32(_)
            | Value::VarIntU32(_)
            | Value::VarIntI64(_)
            | Value::VarIntU64(_)
            | Value::Float(_)
            | Value::Double(_) => self.numbers += 1,
            Value::String(_) => self.strings += 1,
            Value::Binary(_) => self.binaries += 1,
            Value::Rtid(_) => self.rtids += 1,
            Value::Array(items) => {
                self.arrays += 1;
                for item in items {
                    self.visit(item, depth + 1);
                }
            }
            Value::Object(entries) => {
                self.objects += 1;
                for (_, item) in entries {
                    self.visit(item, depth + 1);
                }
            }
        }
    }
}
