use super::types::RtonVarintInfo;
use crate::ByteRead;

pub fn format_inspector_offset(offset: usize) -> String {
    format!("0x{offset:08X}")
}

pub(crate) fn byte_to_hex(byte: u8) -> String {
    format!("{byte:02X}")
}

pub fn read_rton_varint(bytes: &impl ByteRead, offset: usize) -> Option<RtonVarintInfo> {
    if offset >= bytes.len() {
        return None;
    }

    let mut value = 0_u128;
    let mut shift = 0_u32;
    let mut cursor = offset;
    while cursor < bytes.len() && cursor - offset < 10 {
        let byte = bytes.byte_at(cursor)? as u128;
        value |= (byte & 0x7f) << shift;
        cursor += 1;
        if byte & 0x80 == 0 {
            return Some(RtonVarintInfo {
                value: value.to_string(),
                zigzag: decode_zigzag(value).to_string(),
                length: cursor - offset,
                bytes: bytes_to_spaced_hex(bytes, offset, cursor),
                next_offset: cursor,
                raw_value: value,
            });
        }
        shift += 7;
    }
    None
}

pub(crate) fn decode_zigzag(value: u128) -> i128 {
    ((value >> 1) as i128) ^ -((value & 1) as i128)
}

pub(crate) fn read_i8(bytes: &impl ByteRead, offset: usize) -> Option<i8> {
    bytes.byte_at(offset).map(|value| value as i8)
}

pub(crate) fn read_u16(bytes: &impl ByteRead, offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.read_array(offset)?))
}

pub(crate) fn read_i16(bytes: &impl ByteRead, offset: usize) -> Option<i16> {
    read_u16(bytes, offset).map(|value| value as i16)
}

pub(crate) fn read_u32(bytes: &impl ByteRead, offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.read_array(offset)?))
}

pub(crate) fn read_i32(bytes: &impl ByteRead, offset: usize) -> Option<i32> {
    read_u32(bytes, offset).map(|value| value as i32)
}

pub(crate) fn read_u64(bytes: &impl ByteRead, offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.read_array(offset)?))
}

pub(crate) fn read_i64(bytes: &impl ByteRead, offset: usize) -> Option<i64> {
    read_u64(bytes, offset).map(|value| value as i64)
}

pub(crate) fn read_f32(bytes: &impl ByteRead, offset: usize) -> Option<f32> {
    Some(f32::from_le_bytes(bytes.read_array(offset)?))
}

pub(crate) fn read_f64(bytes: &impl ByteRead, offset: usize) -> Option<f64> {
    Some(f64::from_le_bytes(bytes.read_array(offset)?))
}

pub(crate) fn has_rton_header(bytes: &impl ByteRead) -> bool {
    bytes.len() >= 8 && bytes.starts_with_bytes(b"RTON")
}

pub(crate) fn is_footer_at(bytes: &impl ByteRead, offset: usize) -> bool {
    bytes.matches_bytes_at(offset, b"DONE")
}

pub(crate) fn safe_length(value: u128) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| "RTON length is too large".to_string())
}

pub(crate) fn offset_range(start: usize, end: usize) -> String {
    format!("0x{start:X}..0x{end:X}")
}

pub(crate) fn bytes_to_spaced_hex(bytes: &impl ByteRead, start: usize, end: usize) -> String {
    bytes
        .range_to_vec(start, end)
        .unwrap_or_default()
        .iter()
        .map(|byte| byte_to_hex(*byte))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn is_printable_ascii(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte)
}

pub(crate) fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| *byte as char).collect()
}

pub(crate) fn decode_utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

pub(crate) fn trim_preview(text: &str) -> String {
    const LIMIT: usize = 96;
    let mut chars = text.chars();
    let preview = chars.by_ref().take(LIMIT).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}
