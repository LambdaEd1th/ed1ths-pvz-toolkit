mod common;
mod json;
mod toml;
mod yaml;

use crate::{RtonValue, TextFormat, ValuePathSegment, parse_value_path_segments};
use serde::{Deserialize, Serialize};

pub use common::offset_to_text_position;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TextLineInfo<'a> {
    pub(super) index: usize,
    pub(super) offset: usize,
    pub(super) text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ValuePathTraceSegment<'a> {
    Object {
        index: usize,
        key: &'a str,
        value: &'a RtonValue,
    },
    Array {
        index: usize,
        value: &'a RtonValue,
    },
}

pub(super) trait TextLocator {
    fn locate_value_offset(
        &self,
        text: &str,
        root: &RtonValue,
        segments: &[ValuePathSegment],
    ) -> Option<usize>;
}

pub fn locate_value_path_in_text(
    root: &RtonValue,
    path: &str,
    text: &str,
    format: TextFormat,
) -> Option<TextPosition> {
    if text.is_empty() {
        return None;
    }

    let segments = parse_value_path_segments(path)?;
    let offset = locator_for_format(format).locate_value_offset(text, root, &segments)?;

    Some(offset_to_text_position(text, offset))
}

fn locator_for_format(format: TextFormat) -> &'static dyn TextLocator {
    match format {
        TextFormat::Json => &json::JsonTextLocator,
        TextFormat::Yaml => &yaml::YamlTextLocator,
        TextFormat::Toml => &toml::TomlTextLocator,
    }
}
