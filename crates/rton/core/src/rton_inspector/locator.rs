use super::primitives::{
    byte_to_hex, format_inspector_offset, has_rton_header, is_footer_at, read_rton_varint,
    read_u32, safe_length,
};
use super::types::RtonVarintInfo;
use crate::{ByteRead, ValuePathSegment, parse_value_path_segments};

pub fn locate_rton_value_offset(bytes: &impl ByteRead, path: &str) -> Option<usize> {
    let segments = parse_value_path_segments(path)?;
    RtonOffsetLocator { bytes }.locate(&segments).ok().flatten()
}

struct RtonOffsetLocator<'a, B: ByteRead> {
    bytes: &'a B,
}

impl<B: ByteRead> RtonOffsetLocator<'_, B> {
    fn locate(&self, path: &[ValuePathSegment]) -> Result<Option<usize>, String> {
        let root_offset = if has_rton_header(self.bytes) { 8 } else { 0 };
        let Some(root_tag) = self.bytes.byte_at(root_offset) else {
            return Ok(None);
        };
        if path.is_empty() {
            return Ok(Some(root_offset));
        }

        if matches!(root_tag, 0x85 | 0xb8) {
            self.locate_in_object(root_offset + 1, path)
        } else {
            self.locate_in_object(root_offset, path)
        }
    }

    fn locate_value(
        &self,
        offset: usize,
        path: &[ValuePathSegment],
    ) -> Result<Option<usize>, String> {
        if path.is_empty() {
            return Ok(Some(offset));
        }

        match self.byte_at(offset)? {
            0x85 | 0xb8 => self.locate_in_object(offset + 1, path),
            0x86 => self.locate_in_standard_array(offset + 1, path),
            0xb9 => self.locate_in_compact_array(offset + 1, path),
            _ => Ok(None),
        }
    }

    fn locate_in_object(
        &self,
        offset: usize,
        path: &[ValuePathSegment],
    ) -> Result<Option<usize>, String> {
        let Some((ValuePathSegment::Object(target_index), rest)) = path.split_first() else {
            return Ok(None);
        };

        let mut cursor = offset;
        let mut entry_index = 0usize;
        while cursor < self.bytes.len() {
            let tag = self.byte_at(cursor)?;
            if tag == 0xff || is_footer_at(self.bytes, cursor) {
                return Ok(None);
            }

            cursor = self.skip_value(cursor)?;
            let value_offset = cursor;
            if entry_index == *target_index {
                return self.locate_value(value_offset, rest);
            }

            cursor = self.skip_value(value_offset)?;
            entry_index += 1;
        }

        Ok(None)
    }

    fn locate_in_standard_array(
        &self,
        offset: usize,
        path: &[ValuePathSegment],
    ) -> Result<Option<usize>, String> {
        let Some((ValuePathSegment::Array(target_index), rest)) = path.split_first() else {
            return Ok(None);
        };

        let mut cursor = offset;
        if self.byte_at(cursor)? != 0xfd {
            return Ok(None);
        }
        cursor += 1;
        let capacity = self.read_varint(cursor)?;
        cursor = capacity.next_offset;
        let count = safe_length(capacity.raw_value)?;

        for index in 0..count {
            if cursor >= self.bytes.len() || self.byte_at(cursor)? == 0xfe {
                return Ok(None);
            }
            if index == *target_index {
                return self.locate_value(cursor, rest);
            }
            cursor = self.skip_value(cursor)?;
        }

        Ok(None)
    }

    fn locate_in_compact_array(
        &self,
        offset: usize,
        path: &[ValuePathSegment],
    ) -> Result<Option<usize>, String> {
        let Some((ValuePathSegment::Array(target_index), rest)) = path.split_first() else {
            return Ok(None);
        };

        let mut cursor = offset;
        if self.byte_at(cursor)? != 0xfd {
            return Ok(None);
        }
        cursor += 1;
        let count = self.read_u32(cursor)? as usize;
        cursor += 4;
        if *target_index >= count {
            return Ok(None);
        }

        let table_offset = cursor;
        let target_offset = self.read_u32(table_offset + target_index * 4)? as usize;
        if target_offset != 0 {
            return self.locate_value(target_offset, rest);
        }

        cursor = cursor
            .checked_add((count + 1).saturating_mul(4))
            .ok_or_else(|| "RTON range is too large".to_string())?;
        for index in 0..count {
            if index == *target_index {
                return self.locate_value(cursor, rest);
            }
            cursor = self.skip_value(cursor)?;
        }

        Ok(None)
    }

    fn skip_value(&self, offset: usize) -> Result<usize, String> {
        let tag = self.byte_at(offset)?;
        let cursor = offset
            .checked_add(1)
            .ok_or_else(|| "RTON range is too large".to_string())?;

        match tag {
            0x00 | 0x01 | 0x02 | 0x09 | 0x0b | 0x11 | 0x13 | 0x21 | 0x23 | 0x27 | 0x41 | 0x43
            | 0x47 | 0x84 | 0xfe | 0xff => Ok(cursor),
            0x08 | 0x0a | 0xbc => self.advance_fixed(cursor, 1),
            0x10 | 0x12 => self.advance_fixed(cursor, 2),
            0x20 | 0x22 | 0x26 => self.advance_fixed(cursor, 4),
            0x40 | 0x42 | 0x46 => self.advance_fixed(cursor, 8),
            0x24 | 0x25 | 0x28 | 0x44 | 0x45 | 0x48 | 0x91 | 0x93 => {
                Ok(self.read_varint(cursor)?.next_offset)
            }
            0x81 | 0x90 => self.skip_latin1_string_payload(cursor),
            0x82 | 0x92 => self.skip_utf8_string_payload(cursor),
            0x83 | 0xba => self.skip_rtid_payload(cursor),
            0x85 | 0xb8 => self.skip_object(cursor),
            0x86 => self.skip_standard_array(cursor),
            0xb9 => self.skip_compact_array(cursor),
            0x87 => {
                let cursor = self.advance_fixed(cursor, 1)?;
                let cursor = self.skip_latin1_string_payload(cursor)?;
                let length = self.read_varint(cursor)?;
                self.advance_fixed(length.next_offset, safe_length(length.raw_value)?)
            }
            0xbb => {
                let cursor = self.skip_compact_binary_blob_string(cursor)?;
                let length = self.read_u32(cursor)? as usize;
                self.advance_fixed(cursor + 4, length)
            }
            0xb0 => self.skip_compact_latin1_definition(cursor, false),
            0xb1 => self.advance_fixed(cursor, 4),
            0xb2 => self.skip_compact_utf32_definition(cursor, false),
            0xb3 => self.advance_fixed(cursor, 4),
            0xb4 => self.skip_compact_latin1_definition(cursor, true),
            0xb5 => self.advance_fixed(cursor, 8),
            0xb6 => self.skip_compact_utf32_definition(cursor, true),
            0xb7 => self.advance_fixed(cursor, 8),
            _ => Err(format!(
                "Unknown RTON tag 0x{} at offset {}",
                byte_to_hex(tag),
                format_inspector_offset(offset)
            )),
        }
    }

    fn skip_object(&self, offset: usize) -> Result<usize, String> {
        let mut cursor = offset;
        while cursor < self.bytes.len() {
            if self.byte_at(cursor)? == 0xff {
                return Ok(cursor + 1);
            }
            cursor = self.skip_value(cursor)?;
            cursor = self.skip_value(cursor)?;
        }
        Err("Unterminated RTON object".to_string())
    }

    fn skip_standard_array(&self, offset: usize) -> Result<usize, String> {
        let mut cursor = offset;
        if self.byte_at(cursor)? != 0xfd {
            return Err("Missing RTON array capacity tag".to_string());
        }
        cursor += 1;
        let capacity = self.read_varint(cursor)?;
        cursor = capacity.next_offset;
        let count = safe_length(capacity.raw_value)?;

        for _ in 0..count {
            if cursor >= self.bytes.len() || self.byte_at(cursor)? == 0xfe {
                return Ok(cursor + 1);
            }
            cursor = self.skip_value(cursor)?;
        }
        if cursor < self.bytes.len() && self.byte_at(cursor)? == 0xfe {
            Ok(cursor + 1)
        } else {
            Ok(cursor)
        }
    }

    fn skip_compact_array(&self, offset: usize) -> Result<usize, String> {
        let mut cursor = offset;
        if self.byte_at(cursor)? != 0xfd {
            return Err("Missing compact RTON array capacity tag".to_string());
        }
        cursor += 1;
        let count = self.read_u32(cursor)? as usize;
        cursor = cursor
            .checked_add(4 + (count + 1).saturating_mul(4))
            .ok_or_else(|| "RTON range is too large".to_string())?;
        for _ in 0..count {
            cursor = self.skip_value(cursor)?;
        }
        Ok(cursor)
    }

    fn skip_latin1_string_payload(&self, offset: usize) -> Result<usize, String> {
        let length = self.read_varint(offset)?;
        self.advance_fixed(length.next_offset, safe_length(length.raw_value)?)
    }

    fn skip_utf8_string_payload(&self, offset: usize) -> Result<usize, String> {
        let char_count = self.read_varint(offset)?;
        let byte_length = self.read_varint(char_count.next_offset)?;
        self.advance_fixed(byte_length.next_offset, safe_length(byte_length.raw_value)?)
    }

    fn skip_rtid_payload(&self, offset: usize) -> Result<usize, String> {
        let sub_tag = self.byte_at(offset)?;
        let mut cursor = offset + 1;
        match sub_tag {
            0x00 => Ok(cursor),
            0x01 => {
                cursor = self.read_varint(cursor)?.next_offset;
                cursor = self.read_varint(cursor)?.next_offset;
                self.advance_fixed(cursor, 4)
            }
            0x02 => {
                cursor = self.skip_utf8_string_payload(cursor)?;
                cursor = self.read_varint(cursor)?.next_offset;
                cursor = self.read_varint(cursor)?.next_offset;
                self.advance_fixed(cursor, 4)
            }
            0x03 => {
                cursor = self.skip_utf8_string_payload(cursor)?;
                self.skip_utf8_string_payload(cursor)
            }
            _ => Err(format!("Unknown RTID subtag 0x{}", byte_to_hex(sub_tag))),
        }
    }

    fn skip_compact_binary_blob_string(&self, offset: usize) -> Result<usize, String> {
        let tag = self.byte_at(offset)?;
        let cursor = offset + 1;
        match tag {
            0xb0 => self.skip_compact_latin1_definition(cursor, false),
            0xb1 => self.advance_fixed(cursor, 4),
            _ => Ok(cursor),
        }
    }

    fn skip_compact_latin1_definition(&self, offset: usize, paired: bool) -> Result<usize, String> {
        let length = self.read_u32(offset)? as usize;
        self.advance_fixed(offset + 4, length + if paired { 4 } else { 0 })
    }

    fn skip_compact_utf32_definition(&self, offset: usize, paired: bool) -> Result<usize, String> {
        let length = self.read_u32(offset)? as usize;
        self.advance_fixed(offset + 4, length + if paired { 4 } else { 0 })
    }

    fn read_varint(&self, offset: usize) -> Result<RtonVarintInfo, String> {
        read_rton_varint(self.bytes, offset)
            .ok_or_else(|| "Unexpected end of RTON varint".to_string())
    }

    fn read_u32(&self, offset: usize) -> Result<u32, String> {
        read_u32(self.bytes, offset).ok_or_else(|| "Unexpected end of RTON bytes".to_string())
    }

    fn advance_fixed(&self, offset: usize, length: usize) -> Result<usize, String> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| "RTON range is too large".to_string())?;
        if end > self.bytes.len() {
            Err("Unexpected end of RTON bytes".to_string())
        } else {
            Ok(end)
        }
    }

    fn byte_at(&self, offset: usize) -> Result<u8, String> {
        self.bytes
            .byte_at(offset)
            .ok_or_else(|| "Unexpected end of RTON bytes".to_string())
    }
}
