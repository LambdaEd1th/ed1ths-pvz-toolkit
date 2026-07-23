use super::types::RtonTagInfo;

pub fn rton_tag_info(byte: u8) -> Option<RtonTagInfo> {
    Some(match byte {
        0x00 => RtonTagInfo {
            name: "BoolFalse",
            category: "bool",
            payload_kind: "none",
        },
        0x01 => RtonTagInfo {
            name: "BoolTrue",
            category: "bool",
            payload_kind: "none",
        },
        0x02 => RtonTagInfo {
            name: "StrNull (*)",
            category: "string",
            payload_kind: "literal *",
        },
        0x08 => RtonTagInfo {
            name: "Int8",
            category: "number",
            payload_kind: "i8",
        },
        0x09 => RtonTagInfo {
            name: "Int8Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x0a => RtonTagInfo {
            name: "UInt8",
            category: "number",
            payload_kind: "u8",
        },
        0x0b => RtonTagInfo {
            name: "UIntZero",
            category: "number",
            payload_kind: "zero",
        },
        0x10 => RtonTagInfo {
            name: "Int16",
            category: "number",
            payload_kind: "i16le",
        },
        0x11 => RtonTagInfo {
            name: "Int16Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x12 => RtonTagInfo {
            name: "UInt16",
            category: "number",
            payload_kind: "u16le",
        },
        0x13 => RtonTagInfo {
            name: "UInt16Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x20 => RtonTagInfo {
            name: "Int32",
            category: "number",
            payload_kind: "i32le",
        },
        0x21 => RtonTagInfo {
            name: "Int32Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x22 => RtonTagInfo {
            name: "Float",
            category: "number",
            payload_kind: "f32le",
        },
        0x23 => RtonTagInfo {
            name: "FloatZero",
            category: "number",
            payload_kind: "zero",
        },
        0x24 => RtonTagInfo {
            name: "VarIntU32",
            category: "varint",
            payload_kind: "varint u32",
        },
        0x25 => RtonTagInfo {
            name: "VarIntI32",
            category: "varint",
            payload_kind: "zigzag i32",
        },
        0x26 => RtonTagInfo {
            name: "UInt32",
            category: "number",
            payload_kind: "u32le",
        },
        0x27 => RtonTagInfo {
            name: "UInt32Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x28 => RtonTagInfo {
            name: "VarIntU32Alt",
            category: "varint",
            payload_kind: "varint u32",
        },
        0x29 => RtonTagInfo {
            name: "VarIntI32Alt",
            category: "varint",
            payload_kind: "zigzag i32",
        },
        0x40 => RtonTagInfo {
            name: "Int64",
            category: "number",
            payload_kind: "i64le",
        },
        0x41 => RtonTagInfo {
            name: "Int64Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x42 => RtonTagInfo {
            name: "Double",
            category: "number",
            payload_kind: "f64le",
        },
        0x43 => RtonTagInfo {
            name: "DoubleZero",
            category: "number",
            payload_kind: "zero",
        },
        0x44 => RtonTagInfo {
            name: "VarIntU64",
            category: "varint",
            payload_kind: "varint u64",
        },
        0x45 => RtonTagInfo {
            name: "VarIntI64",
            category: "varint",
            payload_kind: "zigzag i64",
        },
        0x46 => RtonTagInfo {
            name: "UInt64",
            category: "number",
            payload_kind: "u64le",
        },
        0x47 => RtonTagInfo {
            name: "UInt64Zero",
            category: "number",
            payload_kind: "zero",
        },
        0x48 => RtonTagInfo {
            name: "VarIntU64Alt",
            category: "varint",
            payload_kind: "varint u64",
        },
        0x49 => RtonTagInfo {
            name: "VarIntI64Alt",
            category: "varint",
            payload_kind: "zigzag i64",
        },
        0x81 => RtonTagInfo {
            name: "StrAsciiDirect",
            category: "string",
            payload_kind: "ascii direct",
        },
        0x82 => RtonTagInfo {
            name: "StrUtf8Direct",
            category: "string",
            payload_kind: "utf8 direct",
        },
        0x83 => RtonTagInfo {
            name: "Rtid",
            category: "rtid",
            payload_kind: "rtid payload",
        },
        0x84 => RtonTagInfo {
            name: "RtidZero",
            category: "rtid",
            payload_kind: "none",
        },
        0x85 => RtonTagInfo {
            name: "ObjectStart",
            category: "container",
            payload_kind: "object entries",
        },
        0x86 => RtonTagInfo {
            name: "ArrayStart",
            category: "container",
            payload_kind: "array capacity",
        },
        0x87 => RtonTagInfo {
            name: "BinaryBlob",
            category: "binary",
            payload_kind: "binary blob",
        },
        0x90 => RtonTagInfo {
            name: "StrAsciiDef",
            category: "string table",
            payload_kind: "ascii definition",
        },
        0x91 => RtonTagInfo {
            name: "StrAsciiRef",
            category: "string table",
            payload_kind: "ascii reference",
        },
        0x92 => RtonTagInfo {
            name: "StrUtf8Def",
            category: "string table",
            payload_kind: "utf8 definition",
        },
        0x93 => RtonTagInfo {
            name: "StrUtf8Ref",
            category: "string table",
            payload_kind: "utf8 reference",
        },
        0xb0 => RtonTagInfo {
            name: "StrCompactAsciiDef",
            category: "compact string table",
            payload_kind: "ascii definition",
        },
        0xb1 => RtonTagInfo {
            name: "StrCompactAsciiRef",
            category: "compact string table",
            payload_kind: "ascii reference",
        },
        0xb2 => RtonTagInfo {
            name: "StrCompactUtf8Def",
            category: "compact string table",
            payload_kind: "utf8 definition",
        },
        0xb3 => RtonTagInfo {
            name: "StrCompactUtf8Ref",
            category: "compact string table",
            payload_kind: "utf8 reference",
        },
        0xb4 => RtonTagInfo {
            name: "StrCompactPair1",
            category: "compact string table",
            payload_kind: "ascii definition",
        },
        0xb5 => RtonTagInfo {
            name: "StrCompactPair2",
            category: "compact string table",
            payload_kind: "ascii reference",
        },
        0xb6 => RtonTagInfo {
            name: "StrCompactPair3",
            category: "compact string table",
            payload_kind: "utf8 definition",
        },
        0xb7 => RtonTagInfo {
            name: "StrCompactPair4",
            category: "compact string table",
            payload_kind: "utf8 reference",
        },
        0xb8 => RtonTagInfo {
            name: "ObjectStartCompact",
            category: "container",
            payload_kind: "object entries",
        },
        0xb9 => RtonTagInfo {
            name: "ArrayStartCompact",
            category: "container",
            payload_kind: "array capacity",
        },
        0xba => RtonTagInfo {
            name: "RtidCompact",
            category: "rtid",
            payload_kind: "rtid payload",
        },
        0xbb => RtonTagInfo {
            name: "BinaryBlobCompact",
            category: "binary",
            payload_kind: "binary blob",
        },
        0xbc => RtonTagInfo {
            name: "BoolCompact",
            category: "bool",
            payload_kind: "u8 bool",
        },
        0xfd => RtonTagInfo {
            name: "ArrayCapacity",
            category: "container",
            payload_kind: "varint capacity",
        },
        0xfe => RtonTagInfo {
            name: "ArrayEnd",
            category: "container",
            payload_kind: "none",
        },
        0xff => RtonTagInfo {
            name: "ObjectEnd",
            category: "container",
            payload_kind: "none",
        },
        _ => return None,
    })
}
