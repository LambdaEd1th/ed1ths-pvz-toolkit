use crate::{RtonValue, ValuePathSegment};

use super::{TextLineInfo, TextPosition, ValuePathTraceSegment};

#[derive(Debug, Clone, Copy)]
pub(super) struct LocatedTextLine<'a> {
    pub(super) line: TextLineInfo<'a>,
    pub(super) offset: usize,
}

pub(super) fn trace_value_path<'a>(
    root: &'a RtonValue,
    path: &[ValuePathSegment],
) -> Option<Vec<ValuePathTraceSegment<'a>>> {
    let mut trace = Vec::new();
    let mut value = root;

    for segment in path {
        match segment {
            ValuePathSegment::Object(index) => {
                let RtonValue::Object(entries) = value else {
                    return None;
                };
                let (key, item) = entries.get(*index)?;
                trace.push(ValuePathTraceSegment::Object {
                    index: *index,
                    key,
                    value: item,
                });
                value = item;
            }
            ValuePathSegment::Array(index) => {
                let RtonValue::Array(items) = value else {
                    return None;
                };
                let item = items.get(*index)?;
                trace.push(ValuePathTraceSegment::Array {
                    index: *index,
                    value: item,
                });
                value = item;
            }
        }
    }

    Some(trace)
}

pub(super) fn text_lines(text: &str) -> Vec<TextLineInfo<'_>> {
    let mut offset = 0usize;
    text.split('\n')
        .enumerate()
        .map(|(index, line)| {
            let text = line.strip_suffix('\r').unwrap_or(line);
            let info = TextLineInfo {
                index,
                offset,
                text,
            };
            offset += line.len() + 1;
            info
        })
        .collect()
}

pub(super) fn leading_spaces(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

pub(super) fn key_forms(key: &str) -> Vec<String> {
    vec![
        key.to_string(),
        serde_json::to_string(key).unwrap_or_else(|_| format!("\"{key}\"")),
        format!("'{}'", key.replace('\'', "''")),
    ]
}

pub fn offset_to_text_position(text: &str, offset: usize) -> TextPosition {
    let bounded_offset = offset.min(text.len());
    let mut line = 1usize;
    let mut line_start = 0usize;
    for (index, byte) in text.bytes().enumerate().take(bounded_offset) {
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }

    TextPosition {
        line,
        column: bounded_offset.saturating_sub(line_start),
    }
}
