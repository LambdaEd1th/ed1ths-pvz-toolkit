use serde::{Deserialize, Serialize};
use serde_rton::Value;
use std::collections::HashSet;

use crate::value_path::escape_path_key;

pub const TREE_ROW_LIMIT: usize = 900;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeRows {
    pub rows: Vec<ValueRow>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueRow {
    pub path: String,
    pub label: String,
    pub kind: String,
    pub preview: String,
    pub depth: usize,
    pub child_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueSearchResult {
    pub query: String,
    pub matches: Vec<ValueSearchMatch>,
    pub scanned: usize,
    pub done: bool,
    pub capped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueSearchMatch {
    pub path: String,
    pub display_path: String,
    pub preview: String,
}

pub fn flatten_value_tree(value: &Value, query: &str, limit: usize) -> TreeRows {
    let needle = query.trim().to_ascii_lowercase();
    let mut context = FlattenContext {
        needle: &needle,
        limit,
        rows: Vec::new(),
        truncated: false,
    };
    context.visit(value, "$", "root", 0);
    TreeRows {
        rows: context.rows,
        truncated: context.truncated,
    }
}

pub fn flatten_expanded_value_tree(
    value: &Value,
    expanded_paths: &HashSet<String>,
    limit: usize,
) -> TreeRows {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rows) = flatten_expanded_value_tree_parallel_top_level(value, expanded_paths, limit)
    {
        return rows;
    }

    let mut context = ExpandedFlattenContext {
        expanded_paths,
        limit,
        rows: Vec::new(),
        truncated: false,
    };
    context.visit(value, "$", "root", 0);
    TreeRows {
        rows: context.rows,
        truncated: context.truncated,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn flatten_expanded_value_tree_parallel_top_level(
    value: &Value,
    expanded_paths: &HashSet<String>,
    limit: usize,
) -> Option<TreeRows> {
    use rayon::prelude::*;

    const PARALLEL_TREE_MIN_CHILDREN: usize = 512;

    if limit != usize::MAX || !expanded_paths.contains("$") {
        return None;
    }

    let root = ValueRow {
        path: "$".to_string(),
        label: "root".to_string(),
        kind: value_kind(value).to_string(),
        preview: value_preview(value),
        depth: 0,
        child_count: value_child_count(value),
    };

    let parts = match value {
        Value::Array(items) if items.len() >= PARALLEL_TREE_MIN_CHILDREN => items
            .par_iter()
            .enumerate()
            .map(|(index, item)| {
                let child_path = format!("$[{index}]");
                let child_label = format!("[{index}]");
                let mut context = ExpandedFlattenContext {
                    expanded_paths,
                    limit: usize::MAX,
                    rows: Vec::new(),
                    truncated: false,
                };
                context.visit(item, &child_path, &child_label, 1);
                context.rows
            })
            .collect::<Vec<_>>(),
        Value::Object(entries) if entries.len() >= PARALLEL_TREE_MIN_CHILDREN => entries
            .par_iter()
            .enumerate()
            .map(|(index, (key, item))| {
                let child_path = format!("$.{}#{index}", escape_path_key(key));
                let mut context = ExpandedFlattenContext {
                    expanded_paths,
                    limit: usize::MAX,
                    rows: Vec::new(),
                    truncated: false,
                };
                context.visit(item, &child_path, key, 1);
                context.rows
            })
            .collect::<Vec<_>>(),
        _ => return None,
    };

    let mut rows = Vec::with_capacity(
        1 + parts
            .iter()
            .map(Vec::len)
            .fold(0usize, usize::saturating_add),
    );
    rows.push(root);
    for part in parts {
        rows.extend(part);
    }
    Some(TreeRows {
        rows,
        truncated: false,
    })
}

pub fn search_value_tree(value: &Value, query: &str, limit: usize) -> ValueSearchResult {
    let needle = query.trim().to_lowercase();
    let mut context = SearchContext {
        needle: &needle,
        limit,
        result: ValueSearchResult {
            query: needle.clone(),
            matches: Vec::new(),
            scanned: 0,
            done: true,
            capped: false,
        },
    };

    if !needle.is_empty() && limit > 0 {
        context.visit(value, "$", "$");
    }

    context.result
}

struct FlattenContext<'a> {
    needle: &'a str,
    limit: usize,
    rows: Vec<ValueRow>,
    truncated: bool,
}

struct ExpandedFlattenContext<'a> {
    expanded_paths: &'a HashSet<String>,
    limit: usize,
    rows: Vec<ValueRow>,
    truncated: bool,
}

impl ExpandedFlattenContext<'_> {
    fn visit(&mut self, value: &Value, path: &str, label: &str, depth: usize) {
        if self.rows.len() >= self.limit {
            self.truncated = true;
            return;
        }

        self.rows.push(ValueRow {
            path: path.to_string(),
            label: label.to_string(),
            kind: value_kind(value).to_string(),
            preview: value_preview(value),
            depth,
            child_count: value_child_count(value),
        });

        if !self.expanded_paths.contains(path) {
            return;
        }

        match value {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    let child_path = format!("{path}[{index}]");
                    let child_label = format!("[{index}]");
                    self.visit(item, &child_path, &child_label, depth + 1);
                    if self.truncated {
                        return;
                    }
                }
            }
            Value::Object(entries) => {
                for (index, (key, item)) in entries.iter().enumerate() {
                    let child_path = format!("{path}.{}#{index}", escape_path_key(key));
                    self.visit(item, &child_path, key, depth + 1);
                    if self.truncated {
                        return;
                    }
                }
            }
            _ => {}
        }
    }
}

impl FlattenContext<'_> {
    fn visit(&mut self, value: &Value, path: &str, label: &str, depth: usize) {
        if self.rows.len() >= self.limit {
            self.truncated = true;
            return;
        }

        let kind = value_kind(value).to_string();
        let preview = value_preview(value);
        let child_count = value_child_count(value);
        let row = ValueRow {
            path: path.to_string(),
            label: label.to_string(),
            kind,
            preview,
            depth,
            child_count,
        };

        if self.needle.is_empty()
            || row.path.to_ascii_lowercase().contains(self.needle)
            || row.label.to_ascii_lowercase().contains(self.needle)
            || row.kind.to_ascii_lowercase().contains(self.needle)
            || row.preview.to_ascii_lowercase().contains(self.needle)
        {
            self.rows.push(row);
        }

        match value {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    let child_path = format!("{path}[{index}]");
                    let child_label = format!("[{index}]");
                    self.visit(item, &child_path, &child_label, depth + 1);
                    if self.truncated {
                        return;
                    }
                }
            }
            Value::Object(entries) => {
                for (index, (key, item)) in entries.iter().enumerate() {
                    let child_path = format!("{path}.{}#{index}", escape_path_key(key));
                    self.visit(item, &child_path, key, depth + 1);
                    if self.truncated {
                        return;
                    }
                }
            }
            _ => {}
        }
    }
}

struct SearchContext<'a> {
    needle: &'a str,
    limit: usize,
    result: ValueSearchResult,
}

impl SearchContext<'_> {
    fn visit(&mut self, value: &Value, path: &str, display_path: &str) {
        if self.result.matches.len() >= self.limit {
            self.result.done = false;
            self.result.capped = true;
            return;
        }

        self.result.scanned += 1;
        let preview = value_preview(value);
        if display_path.to_lowercase().contains(self.needle)
            || preview.to_lowercase().contains(self.needle)
        {
            self.result.matches.push(ValueSearchMatch {
                path: path.to_string(),
                display_path: display_path.to_string(),
                preview,
            });
            if self.result.matches.len() >= self.limit {
                self.result.done = false;
                self.result.capped = true;
                return;
            }
        }

        match value {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    let child_path = format!("{path}[{index}]");
                    let child_display_path = format!("{display_path}[{index}]");
                    self.visit(item, &child_path, &child_display_path);
                    if self.result.capped {
                        return;
                    }
                }
            }
            Value::Object(entries) => {
                for (index, (key, item)) in entries.iter().enumerate() {
                    let child_path = format!("{path}.{}#{index}", escape_path_key(key));
                    let child_display_path = display_child_path(display_path, key);
                    self.visit(item, &child_path, &child_display_path);
                    if self.result.capped {
                        return;
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int8(_) => "i8",
        Value::UInt8(_) => "u8",
        Value::Int16(_) => "i16",
        Value::UInt16(_) => "u16",
        Value::Int32(_) => "i32",
        Value::UInt32(_) => "u32",
        Value::Int64(_) => "i64",
        Value::UInt64(_) => "u64",
        Value::VarIntI32(_) => "varint i32",
        Value::VarIntU32(_) => "varint u32",
        Value::VarIntI64(_) => "varint i64",
        Value::VarIntU64(_) => "varint u64",
        Value::Float(_) => "f32",
        Value::Double(_) => "f64",
        Value::String(_) => "string",
        Value::Binary(_) => "binary",
        Value::Rtid(_) => "rtid",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn value_preview(value: &Value) -> String {
    let preview = match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Int8(value) => value.to_string(),
        Value::UInt8(value) => value.to_string(),
        Value::Int16(value) => value.to_string(),
        Value::UInt16(value) => value.to_string(),
        Value::Int32(value) => value.to_string(),
        Value::UInt32(value) => value.to_string(),
        Value::Int64(value) => value.to_string(),
        Value::UInt64(value) => value.to_string(),
        Value::VarIntI32(value) => value.0.to_string(),
        Value::VarIntU32(value) => value.0.to_string(),
        Value::VarIntI64(value) => value.0.to_string(),
        Value::VarIntU64(value) => value.0.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Double(value) => value.to_string(),
        Value::String(value) => format!("\"{value}\""),
        Value::Binary(value) => value.to_string(),
        Value::Rtid(value) => value.to_string(),
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(entries) => format!("{} entries", entries.len()),
    };
    truncate(&preview, 140)
}

fn value_child_count(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len(),
        Value::Object(entries) => entries.len(),
        _ => 0,
    }
}

fn display_child_path(parent: &str, key: &str) -> String {
    if is_display_identifier(key) {
        format!("{parent}.{key}")
    } else {
        let quoted = serde_json::to_string(key).unwrap_or_else(|_| format!("{key:?}"));
        format!("{parent}[{quoted}]")
    }
}

fn is_display_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    out.push_str("...");
    out
}
