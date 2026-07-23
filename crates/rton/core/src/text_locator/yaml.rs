use crate::{RtonValue, ValuePathSegment};

use super::{
    TextLineInfo, TextLocator, ValuePathTraceSegment,
    common::{LocatedTextLine, key_forms, leading_spaces, text_lines, trace_value_path},
};

pub(super) struct YamlTextLocator;

impl TextLocator for YamlTextLocator {
    fn locate_value_offset(
        &self,
        text: &str,
        root: &RtonValue,
        segments: &[ValuePathSegment],
    ) -> Option<usize> {
        locate_yaml_value_offset(text, root, segments)
    }
}

pub(super) fn locate_yaml_value_offset(
    text: &str,
    root: &RtonValue,
    path: &[ValuePathSegment],
) -> Option<usize> {
    let trace = trace_value_path(root, path)?;
    if trace.is_empty() {
        return Some(0);
    }

    let lines = text_lines(text);
    let mut start_line = 0usize;
    let mut indent = 0usize;
    let mut last_offset = None;

    for segment in trace {
        match segment {
            ValuePathTraceSegment::Object { key, .. } => {
                let found = find_yaml_key_line(&lines, start_line, indent, key)?;
                last_offset = Some(found.offset);
                start_line = found.line.index;
                indent = leading_spaces(found.line.text) + 2;
            }
            ValuePathTraceSegment::Array { index, .. } => {
                let found = find_yaml_array_item_line(&lines, start_line, indent, index)?;
                last_offset = Some(found.offset);
                start_line = found.line.index;
                indent = leading_spaces(found.line.text) + 2;
            }
        }
    }

    last_offset
}

fn find_yaml_key_line<'a>(
    lines: &'a [TextLineInfo<'a>],
    start_line: usize,
    indent: usize,
    key: &str,
) -> Option<LocatedTextLine<'a>> {
    for line in lines.iter().skip(start_line) {
        let trimmed = line.text.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let leading = leading_spaces(line.text);
        if line.index > start_line
            && indent > 0
            && leading < indent
            && !line.text.trim_start().starts_with("- ")
        {
            break;
        }

        if let Some(offset) = yaml_key_offset(*line, key, indent) {
            return Some(LocatedTextLine {
                line: *line,
                offset,
            });
        }
    }
    None
}

fn find_yaml_array_item_line<'a>(
    lines: &'a [TextLineInfo<'a>],
    start_line: usize,
    indent: usize,
    target_index: usize,
) -> Option<LocatedTextLine<'a>> {
    let mut seen = 0usize;
    for line in lines.iter().skip(start_line) {
        let trimmed = line.text.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let leading = leading_spaces(line.text);
        let content = &line.text[leading..];
        let indentationless_sequence =
            indent >= 2 && leading == indent - 2 && yaml_sequence_indicator(content);
        if line.index > start_line && indent > 0 && leading < indent && !indentationless_sequence {
            break;
        }

        if (leading == indent || indentationless_sequence) && yaml_sequence_indicator(content) {
            if seen == target_index {
                return Some(LocatedTextLine {
                    line: *line,
                    offset: line.offset + yaml_sequence_value_column(line.text, leading),
                });
            }
            seen += 1;
        }
    }
    None
}

fn yaml_sequence_indicator(content: &str) -> bool {
    content == "-" || content.starts_with("- ") || content.starts_with("-\t")
}

fn yaml_sequence_value_column(text: &str, leading: usize) -> usize {
    let after_dash = leading.saturating_add(1);
    after_dash
        + text[after_dash..]
            .bytes()
            .take_while(u8::is_ascii_whitespace)
            .count()
}

fn yaml_key_offset(line: TextLineInfo<'_>, key: &str, indent: usize) -> Option<usize> {
    let leading = leading_spaces(line.text);
    let content = &line.text[leading..];
    if leading == indent && yaml_key_prefix_offset(content, key).is_some() {
        return Some(line.offset + leading);
    }

    if leading == indent.saturating_sub(2) && content.starts_with("- ") {
        let inline = &content[2..];
        if yaml_key_prefix_offset(inline, key).is_some() {
            return Some(line.offset + leading + 2);
        }
    }

    None
}

fn yaml_key_prefix_offset(content: &str, key: &str) -> Option<usize> {
    key_forms(key).into_iter().find_map(|form| {
        (content.starts_with(&format!("{form}:")) || content.starts_with(&format!("{form} :")))
            .then_some(0)
    })
}
