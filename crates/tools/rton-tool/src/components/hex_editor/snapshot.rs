use crate::domain::{
    ByteDocument, HexSearchMatch, HexSearchMode, HexSearchResult, find_containing_match,
    hex_search_status_text, hex_virtual_row_top, hex_virtual_scroll, parse_replace_pattern,
    parse_search_pattern,
};
use crate::i18n::I18n;

use super::logic::{hex_offset_width, normalize_selection, to_offset_hex};
use super::state::ByteSelection;

pub(super) struct HexEditorSnapshot {
    pub(super) bytes_len: usize,
    pub(super) safe_selected_offset: usize,
    pub(super) selected_byte: u8,
    pub(super) offset_width: usize,
    pub(super) row_count: usize,
    pub(super) content_height: usize,
    pub(super) visible_rows: Vec<(usize, i64)>,
    pub(super) normalized_selection: Option<ByteSelection>,
    pub(super) search_mode: HexSearchMode,
    pub(super) search_query: String,
    pub(super) replace_query: String,
    pub(super) case_sensitive: bool,
    pub(super) search_matches: Vec<HexSearchMatch>,
    pub(super) current_match: Option<HexSearchMatch>,
    pub(super) search_status_text: String,
    pub(super) search_controls_disabled: bool,
    pub(super) replace_controls_disabled: bool,
    pub(super) search_pattern_bytes: Vec<u8>,
    pub(super) replace_pattern_bytes: Vec<u8>,
    pub(super) style: String,
    pub(super) mode_label: String,
    pub(super) selection_label: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn hex_editor_snapshot(
    bytes: &ByteDocument,
    selected_offset: usize,
    selection_range: Option<ByteSelection>,
    insert_mode: bool,
    scroll_top: f64,
    viewport_height: usize,
    bytes_per_row: usize,
    inspector_width: i32,
    search_panel_visible: bool,
    search_mode: HexSearchMode,
    search_query: String,
    replace_query: String,
    case_sensitive: bool,
    search_result: HexSearchResult,
    i18n: I18n,
) -> HexEditorSnapshot {
    let bytes_len = bytes.len();
    let bytes_per_row = bytes_per_row.max(1);
    let safe_selected_offset = if bytes_len == 0 {
        0
    } else {
        selected_offset.min(bytes_len - 1)
    };
    let selected_byte = bytes.byte_at(safe_selected_offset).unwrap_or_default();
    let offset_width = hex_offset_width(bytes_len);
    let row_count = bytes_len.div_ceil(bytes_per_row);
    let virtual_scroll = hex_virtual_scroll(row_count, scroll_top, viewport_height);
    let visible_rows = (virtual_scroll.start_row..virtual_scroll.end_row)
        .map(|row_index| {
            (
                row_index,
                hex_virtual_row_top(row_index, scroll_top, virtual_scroll),
            )
        })
        .collect::<Vec<_>>();
    let normalized_selection = normalize_selection(selection_range, bytes_len);
    let search_pattern = parse_search_pattern(search_mode, &search_query, i18n);
    let replace_pattern = parse_replace_pattern(search_mode, &replace_query, i18n);
    let search_result =
        if search_panel_visible && search_pattern.valid && !search_pattern.bytes.is_empty() {
            search_result
        } else {
            HexSearchResult {
                matches: Vec::new(),
                capped: false,
            }
        };
    let search_matches = search_result.matches;
    let current_match = find_containing_match(&search_matches, safe_selected_offset);
    let current_match_index = current_match.and_then(|current| {
        search_matches
            .iter()
            .position(|candidate| *candidate == current)
    });
    let search_status_text = hex_search_status_text(
        &search_query,
        &search_pattern,
        &search_matches,
        current_match_index,
        search_result.capped,
        i18n,
    );
    let search_controls_disabled = !search_pattern.valid || search_matches.is_empty();
    let replace_controls_disabled = search_controls_disabled || !replace_pattern.valid;
    let style = format!(
        "--rton-hex-columns: {bytes_per_row}; --rton-hex-offset-width: {offset_width}ch; --rton-hex-ascii-width: {bytes_per_row}ch; --rton-hex-inspector-width: {inspector_width}px;"
    );
    let mode_label = if insert_mode {
        i18n.t("hex-insert")
    } else {
        i18n.t("hex-overwrite")
    };
    let selection_label = normalized_selection.map(|selection| {
        i18n.t_args(
            "hex-selection",
            &[
                (
                    "start",
                    to_offset_hex(selection.anchor.min(selection.focus), offset_width - 2),
                ),
                (
                    "end",
                    to_offset_hex(selection.anchor.max(selection.focus), offset_width - 2),
                ),
                (
                    "count",
                    (selection.anchor.max(selection.focus) - selection.anchor.min(selection.focus)
                        + 1)
                    .to_string(),
                ),
            ],
        )
    });

    HexEditorSnapshot {
        bytes_len,
        safe_selected_offset,
        selected_byte,
        offset_width,
        row_count,
        content_height: virtual_scroll.content_height,
        visible_rows,
        normalized_selection,
        search_mode,
        search_query,
        replace_query,
        case_sensitive,
        search_matches,
        current_match,
        search_status_text,
        search_controls_disabled,
        replace_controls_disabled,
        search_pattern_bytes: search_pattern.bytes,
        replace_pattern_bytes: replace_pattern.bytes,
        style,
        mode_label,
        selection_label,
    }
}
