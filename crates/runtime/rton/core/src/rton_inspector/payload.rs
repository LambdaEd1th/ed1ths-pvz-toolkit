use super::cursor::RtonCursor;
use super::primitives::{
    byte_to_hex, bytes_to_spaced_hex, decode_latin1, decode_utf8, offset_range, read_f32, read_f64,
    read_i8, read_i16, read_i32, read_i64, read_rton_varint, read_u16, read_u32, read_u64,
    safe_length, trim_preview,
};
use super::types::{ParsedStringPayload, RtonPayloadInfo, RtonTagInfo};
use crate::ByteRead;

pub fn inspect_special_region(bytes: &impl ByteRead, offset: usize) -> Option<String> {
    const FILE_HEADER: &[u8] = b"RTON";
    const FILE_FOOTER: &[u8] = b"DONE";
    if bytes.starts_with_bytes(FILE_HEADER) && offset < 4 {
        return Some("RTON header magic".to_string());
    }
    if bytes.starts_with_bytes(FILE_HEADER) && (4..8).contains(&offset) && bytes.len() >= 8 {
        let version = u32::from_le_bytes([
            bytes.byte_at(4)?,
            bytes.byte_at(5)?,
            bytes.byte_at(6)?,
            bytes.byte_at(7)?,
        ]);
        return Some(format!("RTON version {version}"));
    }
    if bytes.ends_with_bytes(FILE_FOOTER) && offset >= bytes.len().saturating_sub(4) {
        return Some("RTON footer DONE".to_string());
    }
    None
}

pub fn inspect_rton_payload(
    bytes: &impl ByteRead,
    offset: usize,
    tag_byte: u8,
    tag: Option<RtonTagInfo>,
) -> Option<RtonPayloadInfo> {
    tag?;
    let payload_offset = offset.checked_add(1)?;
    match tag_byte {
        0x00 => Some(scalar_payload("bool", "false")),
        0x01 => Some(scalar_payload("bool", "true")),
        0x02 => Some(scalar_payload("literal", "*")),
        0x09 | 0x0b | 0x11 | 0x13 | 0x21 | 0x23 | 0x27 | 0x41 | 0x43 | 0x47 => {
            Some(scalar_payload("value", "0"))
        }
        0x08 => fixed_payload(
            bytes,
            payload_offset,
            1,
            "i8",
            read_i8(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x0a | 0xbc => fixed_payload(
            bytes,
            payload_offset,
            1,
            if tag_byte == 0xbc { "bool byte" } else { "u8" },
            bytes
                .byte_at(payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x10 => fixed_payload(
            bytes,
            payload_offset,
            2,
            "i16le",
            read_i16(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x12 => fixed_payload(
            bytes,
            payload_offset,
            2,
            "u16le",
            read_u16(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x20 => fixed_payload(
            bytes,
            payload_offset,
            4,
            "i32le",
            read_i32(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x22 => fixed_payload(
            bytes,
            payload_offset,
            4,
            "f32le",
            read_f32(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x26 => fixed_payload(
            bytes,
            payload_offset,
            4,
            "u32le",
            read_u32(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x40 => fixed_payload(
            bytes,
            payload_offset,
            8,
            "i64le",
            read_i64(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x42 => fixed_payload(
            bytes,
            payload_offset,
            8,
            "f64le",
            read_f64(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x46 => fixed_payload(
            bytes,
            payload_offset,
            8,
            "u64le",
            read_u64(bytes, payload_offset)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ),
        0x24 | 0x28 | 0x44 | 0x48 | 0xfd => varint_payload(bytes, payload_offset, false, "varint"),
        0x25 | 0x29 | 0x45 | 0x49 => varint_payload(bytes, payload_offset, true, "varint"),
        0x81 | 0x90 | 0xb0 | 0xb4 => string_payload(bytes, payload_offset, false),
        0x82 | 0x92 | 0xb2 | 0xb6 => string_payload(bytes, payload_offset, true),
        0x91 | 0x93 | 0xb1 | 0xb3 | 0xb5 | 0xb7 => {
            varint_payload(bytes, payload_offset, false, "table index")
        }
        0x86 | 0xb9 => inspect_array_capacity_payload(bytes, payload_offset),
        0x83 | 0xba => inspect_rtid_payload(bytes, payload_offset),
        0x84 => Some(scalar_payload("rtid", "RTID(0)")),
        0xbb => read_rton_varint(bytes, payload_offset).map(|length| RtonPayloadInfo {
            label: "binary length".to_string(),
            value: length.value,
            bytes: Some(length.bytes),
            range: Some(offset_range(payload_offset, length.next_offset)),
        }),
        _ => None,
    }
}

fn inspect_array_capacity_payload(bytes: &impl ByteRead, offset: usize) -> Option<RtonPayloadInfo> {
    if bytes.byte_at(offset)? != 0xfd {
        return None;
    }
    let capacity = read_rton_varint(bytes, offset + 1)?;
    Some(RtonPayloadInfo {
        label: "capacity".to_string(),
        value: capacity.value.clone(),
        bytes: Some(format!("FD {}", capacity.bytes)),
        range: Some(offset_range(offset, capacity.next_offset)),
    })
}

fn inspect_rtid_payload(bytes: &impl ByteRead, offset: usize) -> Option<RtonPayloadInfo> {
    let sub_tag = bytes.byte_at(offset)?;
    let name = match sub_tag {
        0x00 => "Zero",
        0x01 => "UidNoString",
        0x02 => "Uid",
        0x03 => "String",
        _ => "Unknown",
    };
    Some(RtonPayloadInfo {
        label: "rtid subtag".to_string(),
        value: format!("{name} (0x{})", byte_to_hex(sub_tag)),
        bytes: Some(byte_to_hex(sub_tag)),
        range: Some(offset_range(offset, offset + 1)),
    })
}

fn string_payload(bytes: &impl ByteRead, offset: usize, utf8: bool) -> Option<RtonPayloadInfo> {
    let parsed = read_string_payload(bytes, offset, utf8).ok()?;
    Some(RtonPayloadInfo {
        label: if utf8 { "utf8 string" } else { "ascii string" }.to_string(),
        value: trim_preview(&parsed.text),
        bytes: None,
        range: Some(offset_range(offset, parsed.end_offset)),
    })
}

fn varint_payload(
    bytes: &impl ByteRead,
    offset: usize,
    signed: bool,
    label: &str,
) -> Option<RtonPayloadInfo> {
    let varint = read_rton_varint(bytes, offset)?;
    Some(RtonPayloadInfo {
        label: label.to_string(),
        value: if signed {
            varint.zigzag.clone()
        } else {
            varint.value.clone()
        },
        bytes: Some(varint.bytes),
        range: Some(offset_range(offset, varint.next_offset)),
    })
}

fn scalar_payload(label: &str, value: &str) -> RtonPayloadInfo {
    RtonPayloadInfo {
        label: label.to_string(),
        value: value.to_string(),
        bytes: None,
        range: None,
    }
}

fn fixed_payload(
    bytes: &impl ByteRead,
    offset: usize,
    length: usize,
    label: &str,
    value: String,
) -> Option<RtonPayloadInfo> {
    let end = offset.checked_add(length)?;
    if end > bytes.len() {
        return None;
    }
    Some(RtonPayloadInfo {
        label: label.to_string(),
        value,
        bytes: Some(bytes_to_spaced_hex(bytes, offset, end)),
        range: Some(offset_range(offset, end)),
    })
}

pub(crate) fn read_string_payload(
    bytes: &impl ByteRead,
    offset: usize,
    utf8: bool,
) -> Result<ParsedStringPayload, String> {
    let mut cursor = RtonCursor::new(bytes, offset);
    if !utf8 {
        let length = cursor.read_varint("Unexpected end of string length")?;
        let byte_count = safe_length(length.raw_value)?;
        let text_bytes = cursor.read_vec(byte_count)?;
        return Ok(ParsedStringPayload {
            text: decode_latin1(&text_bytes),
            length: Some(length.raw_value),
            byte_length: length.raw_value,
            end_offset: cursor.offset(),
        });
    }

    let char_count = cursor.read_varint("Unexpected end of UTF-8 char count")?;
    let byte_length = cursor.read_varint("Unexpected end of UTF-8 byte length")?;
    let byte_count = safe_length(byte_length.raw_value)?;
    let text_bytes = cursor.read_vec(byte_count)?;
    Ok(ParsedStringPayload {
        text: decode_utf8(&text_bytes),
        length: Some(char_count.raw_value),
        byte_length: byte_length.raw_value,
        end_offset: cursor.offset(),
    })
}
