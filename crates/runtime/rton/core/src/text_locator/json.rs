use crate::{RtonValue, ValuePathSegment};

use super::TextLocator;

pub(super) struct JsonTextLocator;

impl TextLocator for JsonTextLocator {
    fn locate_value_offset(
        &self,
        text: &str,
        root: &RtonValue,
        segments: &[ValuePathSegment],
    ) -> Option<usize> {
        locate_json_value_offset(text, skip_json_whitespace(text, 0), root, segments)
    }
}

pub(super) fn locate_json_value_offset(
    text: &str,
    position: usize,
    value: &RtonValue,
    path: &[ValuePathSegment],
) -> Option<usize> {
    let pos = skip_json_whitespace(text, position);
    if path.is_empty() {
        return Some(pos);
    }

    let (segment, rest) = path.split_first()?;
    match segment {
        ValuePathSegment::Object(target_index) => {
            let RtonValue::Object(entries) = value else {
                return None;
            };
            if text.as_bytes().get(pos) != Some(&b'{') {
                return None;
            }

            let mut cursor = pos + 1;
            for (index, (_, item)) in entries.iter().enumerate() {
                cursor = skip_json_whitespace(text, cursor);
                if text.as_bytes().get(cursor) == Some(&b'}') {
                    return None;
                }

                let key_start = cursor;
                let key_end = scan_json_string_end(text, key_start)?;
                cursor = skip_json_whitespace(text, key_end);
                if text.as_bytes().get(cursor) != Some(&b':') {
                    return None;
                }

                let child_start = skip_json_whitespace(text, cursor + 1);
                if index == *target_index {
                    return if rest.is_empty() {
                        Some(key_start)
                    } else {
                        locate_json_value_offset(text, child_start, item, rest)
                    };
                }

                let next_cursor = skip_json_value(text, child_start)?;
                cursor = skip_json_whitespace(text, next_cursor);
                if text.as_bytes().get(cursor) == Some(&b',') {
                    cursor += 1;
                }
            }
            None
        }
        ValuePathSegment::Array(target_index) => {
            let RtonValue::Array(items) = value else {
                return None;
            };
            if text.as_bytes().get(pos) != Some(&b'[') {
                return None;
            }

            let mut cursor = pos + 1;
            for (index, item) in items.iter().enumerate() {
                let child_start = skip_json_whitespace(text, cursor);
                if text.as_bytes().get(child_start) == Some(&b']') {
                    return None;
                }
                if index == *target_index {
                    return if rest.is_empty() {
                        Some(child_start)
                    } else {
                        locate_json_value_offset(text, child_start, item, rest)
                    };
                }

                let next_cursor = skip_json_value(text, child_start)?;
                cursor = skip_json_whitespace(text, next_cursor);
                if text.as_bytes().get(cursor) == Some(&b',') {
                    cursor += 1;
                }
            }
            None
        }
    }
}

pub(super) fn skip_json_whitespace(text: &str, position: usize) -> usize {
    let bytes = text.as_bytes();
    let mut cursor = position;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
}

fn scan_json_string_end(text: &str, position: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(position) != Some(&b'"') {
        return None;
    }

    let mut cursor = position + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = cursor.saturating_add(2),
            b'"' => return Some(cursor + 1),
            _ => cursor += 1,
        }
    }
    None
}

fn skip_json_value(text: &str, position: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let pos = skip_json_whitespace(text, position);
    let first = *bytes.get(pos)?;
    if first == b'"' {
        return scan_json_string_end(text, pos);
    }

    if first == b'{' || first == b'[' {
        let close = if first == b'{' { b'}' } else { b']' };
        let mut depth = 0usize;
        let mut cursor = pos;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'"' => cursor = scan_json_string_end(text, cursor)?,
                byte if byte == first => {
                    depth += 1;
                    cursor += 1;
                }
                byte if byte == close => {
                    depth = depth.checked_sub(1)?;
                    cursor += 1;
                    if depth == 0 {
                        return Some(cursor);
                    }
                }
                _ => cursor += 1,
            }
        }
        return None;
    }

    let mut cursor = pos;
    while cursor < bytes.len() && !matches!(bytes[cursor], b',' | b']' | b'}') {
        cursor += 1;
    }
    Some(cursor)
}
