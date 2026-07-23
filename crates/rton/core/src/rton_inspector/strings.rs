use super::payload::read_string_payload;
use super::primitives::{
    byte_to_hex, decode_latin1, format_inspector_offset, has_rton_header, is_footer_at,
    is_printable_ascii, read_rton_varint, safe_length, trim_preview,
};
use super::types::{
    RtonAsciiRun, RtonStringInfo, RtonStringMode, RtonStringTables, RtonVarintInfo,
};
use crate::ByteRead;

const RTON_STRING_TABLE_SCAN_LIMIT: usize = 768 * 1024;
const RTON_ASCII_RUN_LIMIT: usize = 96;

pub fn inspect_rton_string_info(
    bytes: &impl ByteRead,
    offset: usize,
    tables: Option<&RtonStringTables>,
) -> Option<RtonStringInfo> {
    let tag = bytes.byte_at(offset)?;
    let payload_offset = offset.checked_add(1)?;
    let ascii_definition = matches!(tag, 0x90 | 0xb0 | 0xb4);
    let utf8_definition = matches!(tag, 0x92 | 0xb2 | 0xb6);
    let ascii_reference = matches!(tag, 0x91 | 0xb1 | 0xb5);
    let utf8_reference = matches!(tag, 0x93 | 0xb3 | 0xb7);
    let ascii_direct = tag == 0x81;
    let utf8_direct = tag == 0x82;

    if ascii_direct || ascii_definition || utf8_direct || utf8_definition {
        return match read_string_payload(bytes, payload_offset, utf8_direct || utf8_definition) {
            Ok(parsed) => {
                let table = if ascii_definition {
                    Some("ASCII")
                } else if utf8_definition {
                    Some("UTF-8")
                } else {
                    None
                };
                let index = match table {
                    Some("ASCII") if tables.is_some_and(|tables| !tables.limited) => {
                        tables.map(|tables| tables.ascii.len().to_string())
                    }
                    Some("UTF-8") if tables.is_some_and(|tables| !tables.limited) => {
                        tables.map(|tables| tables.utf8.len().to_string())
                    }
                    _ => None,
                };
                Some(RtonStringInfo {
                    mode: if ascii_direct || utf8_direct {
                        RtonStringMode::Direct
                    } else {
                        RtonStringMode::Definition
                    },
                    encoding: if utf8_direct || utf8_definition {
                        "UTF-8"
                    } else {
                        "ASCII"
                    },
                    table,
                    index,
                    length: parsed.length.map(|value| value.to_string()),
                    byte_length: Some(parsed.byte_length.to_string()),
                    text: Some(trim_preview(&parsed.text)),
                    resolved_text: None,
                    scan_limited: tables.is_some_and(|tables| tables.limited),
                    scan_error: tables.and_then(|tables| tables.error.clone()),
                })
            }
            Err(error) => Some(RtonStringInfo {
                mode: if ascii_direct || utf8_direct {
                    RtonStringMode::Direct
                } else {
                    RtonStringMode::Definition
                },
                encoding: if utf8_direct || utf8_definition {
                    "UTF-8"
                } else {
                    "ASCII"
                },
                table: if ascii_direct || utf8_direct {
                    None
                } else if utf8_definition {
                    Some("UTF-8")
                } else {
                    Some("ASCII")
                },
                index: None,
                length: None,
                byte_length: None,
                text: None,
                resolved_text: None,
                scan_limited: tables.is_some_and(|tables| tables.limited),
                scan_error: Some(error),
            }),
        };
    }

    if ascii_reference || utf8_reference {
        let table = if ascii_reference { "ASCII" } else { "UTF-8" };
        return match read_rton_varint(bytes, payload_offset) {
            Some(index) => {
                let resolved_text = safe_length(index.raw_value).ok().and_then(|numeric_index| {
                    let values = if ascii_reference {
                        tables.map(|tables| &tables.ascii)
                    } else {
                        tables.map(|tables| &tables.utf8)
                    }?;
                    values.get(numeric_index).map(|value| trim_preview(value))
                });
                Some(RtonStringInfo {
                    mode: RtonStringMode::Reference,
                    encoding: table,
                    table: Some(table),
                    index: Some(index.value),
                    length: None,
                    byte_length: None,
                    text: None,
                    resolved_text,
                    scan_limited: tables.is_some_and(|tables| tables.limited),
                    scan_error: tables.and_then(|tables| tables.error.clone()),
                })
            }
            None => Some(RtonStringInfo {
                mode: RtonStringMode::Reference,
                encoding: table,
                table: Some(table),
                index: None,
                length: None,
                byte_length: None,
                text: None,
                resolved_text: None,
                scan_limited: tables.is_some_and(|tables| tables.limited),
                scan_error: Some("Unexpected end of RTON varint".to_string()),
            }),
        };
    }

    None
}

pub fn maybe_collect_string_tables(
    bytes: &impl ByteRead,
    target_offset: usize,
) -> Option<RtonStringTables> {
    if target_offset == 0 {
        return Some(RtonStringTables {
            ascii: Vec::new(),
            utf8: Vec::new(),
            limited: false,
            error: None,
        });
    }
    if target_offset > RTON_STRING_TABLE_SCAN_LIMIT {
        return Some(RtonStringTables {
            ascii: Vec::new(),
            utf8: Vec::new(),
            limited: true,
            error: None,
        });
    }

    Some(RtonStringTableScanner::new(bytes, target_offset).scan())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RtonStringTableKind {
    Ascii,
    Utf8,
}

struct RtonStringTableScanner<'a, B: ByteRead> {
    bytes: &'a B,
    target_offset: usize,
    ascii: Vec<String>,
    utf8: Vec<String>,
}

impl<'a, B: ByteRead> RtonStringTableScanner<'a, B> {
    fn new(bytes: &'a B, target_offset: usize) -> Self {
        Self {
            bytes,
            target_offset,
            ascii: Vec::new(),
            utf8: Vec::new(),
        }
    }

    fn scan(mut self) -> RtonStringTables {
        let result = self.scan_inner();
        RtonStringTables {
            ascii: self.ascii,
            utf8: self.utf8,
            limited: false,
            error: result.err(),
        }
    }

    fn scan_inner(&mut self) -> Result<(), String> {
        let mut cursor = if has_rton_header(self.bytes) { 8 } else { 0 };
        while cursor < self.target_offset
            && cursor < self.bytes.len()
            && !is_footer_at(self.bytes, cursor)
        {
            let next = self.scan_value(cursor)?;
            if next <= cursor {
                return Err("RTON scanner did not advance".to_string());
            }
            cursor = next;
        }
        Ok(())
    }

    fn scan_value(&mut self, offset: usize) -> Result<usize, String> {
        if offset >= self.target_offset {
            return Ok(offset);
        }

        let tag = self.byte_at(offset)?;
        let cursor = offset + 1;
        match tag {
            0x00 | 0x01 | 0x02 | 0x09 | 0x0b | 0x11 | 0x13 | 0x21 | 0x23 | 0x27 | 0x41 | 0x43
            | 0x47 | 0x84 | 0xfe | 0xff => Ok(cursor),
            0x08 | 0x0a | 0xbc => self.advance_fixed(cursor, 1),
            0x10 | 0x12 => self.advance_fixed(cursor, 2),
            0x20 | 0x22 | 0x26 => self.advance_fixed(cursor, 4),
            0x40 | 0x42 | 0x46 => self.advance_fixed(cursor, 8),
            0x24 | 0x25 | 0x28 | 0x29 | 0x44 | 0x45 | 0x48 | 0x49 | 0x91 | 0x93 | 0xb1 | 0xb3
            | 0xb5 | 0xb7 | 0xfd => Ok(self.read_varint(cursor)?.next_offset),
            0x81 => self.skip_string(cursor, false, None),
            0x82 => self.skip_string(cursor, true, None),
            0x90 | 0xb0 | 0xb4 => self.skip_string(cursor, false, Some(RtonStringTableKind::Ascii)),
            0x92 | 0xb2 | 0xb6 => self.skip_string(cursor, true, Some(RtonStringTableKind::Utf8)),
            0x83 | 0xba => self.skip_rtid(cursor),
            0x85 | 0xb8 => self.scan_object(cursor),
            0x86 | 0xb9 => self.scan_array(cursor),
            0x87 => {
                let cursor = self.advance_fixed(cursor, 1)?;
                let cursor = self.skip_string(cursor, false, None)?;
                Ok(self.read_varint(cursor)?.next_offset)
            }
            0xbb => {
                let length = self.read_varint(cursor)?;
                let byte_length = safe_length(length.raw_value)?;
                length
                    .next_offset
                    .checked_add(byte_length)
                    .ok_or_else(|| "RTON length is too large".to_string())
            }
            _ => Err(format!(
                "Unknown RTON tag 0x{} at offset {}",
                byte_to_hex(tag),
                format_inspector_offset(offset)
            )),
        }
    }

    fn scan_object(&mut self, offset: usize) -> Result<usize, String> {
        let mut cursor = offset;
        while cursor < self.bytes.len() && cursor < self.target_offset {
            if self.byte_at(cursor)? == 0xff || is_footer_at(self.bytes, cursor) {
                return Ok(cursor + 1);
            }
            cursor = self.scan_value(cursor)?;
            if cursor >= self.target_offset {
                return Ok(cursor);
            }
            cursor = self.scan_value(cursor)?;
        }
        Ok(cursor)
    }

    fn scan_array(&mut self, offset: usize) -> Result<usize, String> {
        let mut cursor = offset;
        if self.byte_at(cursor)? != 0xfd {
            return Err("Missing RTON array capacity tag".to_string());
        }
        let capacity = self.read_varint(cursor + 1)?;
        cursor = capacity.next_offset;
        let count = safe_length(capacity.raw_value)?.min(1_000_000);
        for _ in 0..count {
            if cursor >= self.bytes.len() || cursor >= self.target_offset {
                return Ok(cursor);
            }
            if self.byte_at(cursor)? == 0xfe {
                return Ok(cursor + 1);
            }
            cursor = self.scan_value(cursor)?;
        }
        if cursor < self.bytes.len() && self.byte_at(cursor)? == 0xfe {
            Ok(cursor + 1)
        } else {
            Ok(cursor)
        }
    }

    fn skip_rtid(&mut self, offset: usize) -> Result<usize, String> {
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
                cursor = self.skip_string(cursor, true, None)?;
                cursor = self.read_varint(cursor)?.next_offset;
                cursor = self.read_varint(cursor)?.next_offset;
                self.advance_fixed(cursor, 4)
            }
            0x03 => {
                cursor = self.skip_string(cursor, true, None)?;
                self.skip_string(cursor, true, None)
            }
            _ => Err(format!("Unknown RTID subtag 0x{}", byte_to_hex(sub_tag))),
        }
    }

    fn skip_string(
        &mut self,
        offset: usize,
        utf8: bool,
        table: Option<RtonStringTableKind>,
    ) -> Result<usize, String> {
        let parsed = read_string_payload(self.bytes, offset, utf8)?;
        if parsed.end_offset <= self.target_offset {
            match table {
                Some(RtonStringTableKind::Ascii) => self.ascii.push(parsed.text),
                Some(RtonStringTableKind::Utf8) => self.utf8.push(parsed.text),
                None => {}
            }
        }
        Ok(parsed.end_offset)
    }

    fn read_varint(&self, offset: usize) -> Result<RtonVarintInfo, String> {
        read_rton_varint(self.bytes, offset)
            .ok_or_else(|| "Unexpected end of RTON varint".to_string())
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

pub fn inspect_ascii_run(bytes: &impl ByteRead, offset: usize) -> Option<RtonAsciiRun> {
    if bytes.is_empty() || !bytes.byte_at(offset).is_some_and(is_printable_ascii) {
        return None;
    }
    let mut start = offset;
    while start > 0
        && offset - start < RTON_ASCII_RUN_LIMIT / 2
        && bytes.byte_at(start - 1).is_some_and(is_printable_ascii)
    {
        start -= 1;
    }
    let mut end = offset + 1;
    while end < bytes.len()
        && end - offset < RTON_ASCII_RUN_LIMIT / 2
        && bytes.byte_at(end).is_some_and(is_printable_ascii)
    {
        end += 1;
    }
    let text_bytes = bytes.range_to_vec(start, end)?;
    Some(RtonAsciiRun {
        text: decode_latin1(&text_bytes),
        start,
        end,
    })
}
