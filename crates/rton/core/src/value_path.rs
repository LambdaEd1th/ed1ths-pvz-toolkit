use serde_rton::Value;

use crate::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuePathSegment {
    Array(usize),
    Object(usize),
}

pub fn value_at_path<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    let segments = parse_value_path(path)?;
    let mut current = value;

    for segment in segments {
        current = value_child_at(current, segment, path)?;
    }

    Ok(current)
}

pub fn replace_value_at_path(root: &mut Value, path: &str, replacement: Value) -> Result<()> {
    let segments = parse_value_path(path)?;
    let Some((last, parents)) = segments.split_last() else {
        *root = replacement;
        return Ok(());
    };

    let mut current = root;
    for segment in parents {
        current = value_child_at_mut(current, *segment, path)?;
    }

    match (current, last) {
        (Value::Array(items), ValuePathSegment::Array(index)) => {
            let Some(item) = items.get_mut(*index) else {
                return Err(CoreError::PathNotFound(path.to_string()));
            };
            *item = replacement;
            Ok(())
        }
        (Value::Object(entries), ValuePathSegment::Object(index)) => {
            let Some((_, item)) = entries.get_mut(*index) else {
                return Err(CoreError::PathNotFound(path.to_string()));
            };
            *item = replacement;
            Ok(())
        }
        _ => Err(CoreError::PathNotFound(path.to_string())),
    }
}

fn parse_value_path(path: &str) -> Result<Vec<ValuePathSegment>> {
    parse_value_path_segments(path).ok_or_else(|| CoreError::InvalidPath(path.to_string()))
}

pub fn parse_value_path_segments(path: &str) -> Option<Vec<ValuePathSegment>> {
    if path == "$" {
        return Some(Vec::new());
    }
    if !path.starts_with('$') {
        return None;
    }

    let mut segments = Vec::new();
    let mut rest = &path[1..];
    while !rest.is_empty() {
        if let Some(next) = rest.strip_prefix('[') {
            let close = next.find(']')?;
            let index = next[..close].parse::<usize>().ok()?;
            segments.push(ValuePathSegment::Array(index));
            rest = &next[close + 1..];
            continue;
        }

        if let Some(next) = rest.strip_prefix('.') {
            let hash_index = find_unescaped_hash(next)?;
            let after_hash = &next[hash_index + 1..];
            let digit_len = after_hash.bytes().take_while(u8::is_ascii_digit).count();
            if digit_len == 0 {
                return None;
            }
            let index = after_hash[..digit_len].parse::<usize>().ok()?;
            segments.push(ValuePathSegment::Object(index));
            rest = &after_hash[digit_len..];
            if !rest.is_empty() && !rest.starts_with('.') && !rest.starts_with('[') {
                return None;
            }
            continue;
        }

        return None;
    }

    Some(segments)
}

fn find_unescaped_hash(segment: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, ch) in segment.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '#' => return Some(index),
            _ => {}
        }
    }
    None
}

fn value_child_at<'a>(
    value: &'a Value,
    segment: ValuePathSegment,
    path: &str,
) -> Result<&'a Value> {
    match (value, segment) {
        (Value::Array(items), ValuePathSegment::Array(index)) => items
            .get(index)
            .ok_or_else(|| CoreError::PathNotFound(path.to_string())),
        (Value::Object(entries), ValuePathSegment::Object(index)) => entries
            .get(index)
            .map(|(_, value)| value)
            .ok_or_else(|| CoreError::PathNotFound(path.to_string())),
        _ => Err(CoreError::PathNotFound(path.to_string())),
    }
}

fn value_child_at_mut<'a>(
    value: &'a mut Value,
    segment: ValuePathSegment,
    path: &str,
) -> Result<&'a mut Value> {
    match (value, segment) {
        (Value::Array(items), ValuePathSegment::Array(index)) => items
            .get_mut(index)
            .ok_or_else(|| CoreError::PathNotFound(path.to_string())),
        (Value::Object(entries), ValuePathSegment::Object(index)) => entries
            .get_mut(index)
            .map(|(_, value)| value)
            .ok_or_else(|| CoreError::PathNotFound(path.to_string())),
        _ => Err(CoreError::PathNotFound(path.to_string())),
    }
}

pub(crate) fn escape_path_key(key: &str) -> String {
    key.replace('.', "\\.").replace('#', "\\#")
}
