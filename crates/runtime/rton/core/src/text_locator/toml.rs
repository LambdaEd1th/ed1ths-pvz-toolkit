use crate::{RtonValue, ValuePathSegment};

use super::{
    TextLineInfo, TextLocator, ValuePathTraceSegment,
    common::{LocatedTextLine, key_forms, leading_spaces, text_lines, trace_value_path},
};

pub(super) struct TomlTextLocator;

impl TextLocator for TomlTextLocator {
    fn locate_value_offset(
        &self,
        text: &str,
        root: &RtonValue,
        segments: &[ValuePathSegment],
    ) -> Option<usize> {
        locate_toml_value_offset(text, root, segments)
    }
}

pub(super) fn locate_toml_value_offset(
    text: &str,
    root: &RtonValue,
    path: &[ValuePathSegment],
) -> Option<usize> {
    let trace = trace_value_path(root, path)?;
    if trace.is_empty() {
        return Some(0);
    }

    let lines = text_lines(text);
    let root_scope_end = toml_section_end(&lines, 0);
    locate_toml_trace(text, &lines, &trace, 0, root_scope_end)
}

fn locate_toml_trace(
    text: &str,
    lines: &[TextLineInfo<'_>],
    trace: &[ValuePathTraceSegment<'_>],
    scope_start: usize,
    scope_end: usize,
) -> Option<usize> {
    let Some(segment) = trace.first() else {
        return Some(lines.get(scope_start).map(|line| line.offset).unwrap_or(0));
    };

    match segment {
        ValuePathTraceSegment::Object { key, value, .. } => {
            if let Some(ValuePathTraceSegment::Array { index, .. }) = trace.get(1) {
                if let Some(line) =
                    find_toml_assignment_line_in_scope(lines, key, scope_start, scope_end)
                {
                    let offset =
                        find_toml_array_item_offset(text, line, *index).unwrap_or(line.offset);
                    return Some(offset);
                }

                if let Some(header) = find_toml_array_header_line(lines, key, *index, scope_start) {
                    if trace.len() <= 2 {
                        return Some(header.offset);
                    }
                    let child_start = header.line.index.saturating_add(1);
                    let child_end = toml_section_end(lines, child_start);
                    return locate_toml_trace(text, lines, &trace[2..], child_start, child_end);
                }
            }

            if trace.len() == 1 {
                return find_toml_assignment_line_in_scope(lines, key, scope_start, scope_end)
                    .map(|line| line.offset)
                    .or_else(|| {
                        find_toml_header_line_in_scope(lines, key, scope_start)
                            .map(|line| line.offset)
                    });
            }

            if matches!(value, RtonValue::Object(_))
                && let Some(header) = find_toml_header_line_in_scope(lines, key, scope_start)
            {
                let child_start = header.line.index.saturating_add(1);
                let child_end = toml_section_end(lines, child_start);
                return locate_toml_trace(text, lines, &trace[1..], child_start, child_end);
            }

            find_toml_assignment_line_in_scope(lines, key, scope_start, scope_end)
                .map(|line| line.offset)
                .or_else(|| {
                    find_toml_header_line_in_scope(lines, key, scope_start).map(|line| line.offset)
                })
        }
        ValuePathTraceSegment::Array { .. } => None,
    }
}

fn find_toml_assignment_line_in_scope<'a>(
    lines: &'a [TextLineInfo<'a>],
    key: &str,
    start: usize,
    end: usize,
) -> Option<LocatedTextLine<'a>> {
    for line in lines.iter().skip(start).take(end.saturating_sub(start)) {
        let leading = leading_spaces(line.text);
        let content = &line.text[leading..];
        if content.is_empty() || content.starts_with('#') || content.starts_with('[') {
            continue;
        }
        for form in key_forms(key) {
            if let Some(rest) = content.strip_prefix(&form)
                && rest.trim_start().starts_with('=')
            {
                return Some(LocatedTextLine {
                    line: *line,
                    offset: line.offset + leading,
                });
            }
        }
    }
    None
}

fn find_toml_header_line_in_scope<'a>(
    lines: &'a [TextLineInfo<'a>],
    key: &str,
    start: usize,
) -> Option<LocatedTextLine<'a>> {
    for line in lines {
        if line.index < start {
            continue;
        }
        let Some(header) = toml_header(line.text) else {
            continue;
        };
        if !header.array && toml_header_matches_key(header.name, key) {
            let key_offset = line
                .text
                .find(key)
                .map(|index| line.offset + index)
                .unwrap_or(line.offset + leading_spaces(line.text));
            return Some(LocatedTextLine {
                line: *line,
                offset: key_offset,
            });
        }
    }
    None
}

fn find_toml_array_header_line<'a>(
    lines: &'a [TextLineInfo<'a>],
    key: &str,
    target_index: usize,
    start: usize,
) -> Option<LocatedTextLine<'a>> {
    let mut seen = 0usize;
    for line in lines {
        if line.index < start {
            continue;
        }
        let Some(header) = toml_header(line.text) else {
            continue;
        };
        if header.array && toml_header_matches_key(header.name, key) {
            if seen == target_index {
                let key_offset = line
                    .text
                    .find(key)
                    .map(|index| line.offset + index)
                    .unwrap_or(line.offset + leading_spaces(line.text));
                return Some(LocatedTextLine {
                    line: *line,
                    offset: key_offset,
                });
            }
            seen += 1;
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct TomlHeader<'a> {
    name: &'a str,
    array: bool,
}

fn toml_header(text: &str) -> Option<TomlHeader<'_>> {
    let content = text.trim();
    if content.starts_with("[[") {
        let end = content.find("]]")?;
        return Some(TomlHeader {
            name: content[2..end].trim(),
            array: true,
        });
    }
    if content.starts_with('[') {
        let end = content.find(']')?;
        return Some(TomlHeader {
            name: content[1..end].trim(),
            array: false,
        });
    }
    None
}

fn toml_header_matches_key(header: &str, key: &str) -> bool {
    let last = header.rsplit('.').next().unwrap_or(header).trim();
    key_forms(key).into_iter().any(|form| last == form)
}

fn toml_section_end(lines: &[TextLineInfo<'_>], start: usize) -> usize {
    lines
        .iter()
        .skip(start)
        .find(|line| toml_header(line.text).is_some())
        .map(|line| line.index)
        .unwrap_or(lines.len())
}

fn find_toml_array_item_offset(
    text: &str,
    line: LocatedTextLine<'_>,
    target_index: usize,
) -> Option<usize> {
    let equals = line.line.text.find('=')?;
    let array_start = line.offset + equals + 1 + line.line.text[equals + 1..].find('[')?;
    let mut cursor = array_start + 1;
    let mut seen = 0usize;

    loop {
        cursor = skip_ascii_whitespace(text, cursor);
        if text.as_bytes().get(cursor) == Some(&b']') {
            return None;
        }

        let item_start = cursor;
        let item_end = skip_toml_inline_array_item(text, cursor)?;
        if seen == target_index {
            return Some(item_start);
        }

        cursor = skip_ascii_whitespace(text, item_end);
        match text.as_bytes().get(cursor) {
            Some(b',') => {
                cursor += 1;
                seen += 1;
            }
            Some(b']') | None => return None,
            _ => return None,
        }
    }
}

fn skip_ascii_whitespace(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
}

fn skip_toml_inline_array_item(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut cursor = start;
    let mut square_depth = 0usize;
    let mut brace_depth = 0usize;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\'' | b'"' => cursor = skip_toml_string(text, cursor)?,
            b'[' => {
                square_depth += 1;
                cursor += 1;
            }
            b']' if square_depth == 0 && brace_depth == 0 => return Some(cursor),
            b']' => {
                square_depth = square_depth.checked_sub(1)?;
                cursor += 1;
            }
            b'{' => {
                brace_depth += 1;
                cursor += 1;
            }
            b'}' => {
                brace_depth = brace_depth.checked_sub(1)?;
                cursor += 1;
            }
            b',' if square_depth == 0 && brace_depth == 0 => return Some(cursor),
            _ => cursor += 1,
        }
    }
    None
}

fn skip_toml_string(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let quote = *bytes.get(start)?;
    if start + 3 <= bytes.len()
        && bytes[start] == quote
        && bytes[start + 1] == quote
        && bytes[start + 2] == quote
    {
        let mut cursor = start + 3;
        while cursor + 3 <= bytes.len() {
            if bytes[cursor] == quote && bytes[cursor + 1] == quote && bytes[cursor + 2] == quote {
                return Some(cursor + 3);
            }
            cursor += 1;
        }
        return None;
    }

    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if quote == b'"' && bytes[cursor] == b'\\' {
            cursor = cursor.saturating_add(2);
            continue;
        }
        if bytes[cursor] == quote {
            return Some(cursor + 1);
        }
        cursor += 1;
    }
    None
}
