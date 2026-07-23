pub(crate) mod app_state;
pub(crate) mod batch_export;
pub(crate) mod byte_document;
pub(crate) mod editor_preferences;
pub(crate) mod editor_tab;
pub(crate) mod file_paths;
pub(crate) mod hex_edit;
pub(crate) mod identity_arc;
pub(crate) mod rton_inspector;
pub(crate) mod text_buffer;
pub(crate) mod text_history;
pub(crate) mod text_search;
pub(crate) mod toolbar_layout;
pub(crate) mod virtual_scroll;

pub(crate) use app_state::{OpenTabError, Status, Tone};
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use batch_export::encode_batch_export_document;
pub(crate) use batch_export::{
    BatchExportMode, ZipArchiveBuilder, batch_archive_name, batch_output_path, unique_zip_path,
};
#[cfg(test)]
pub(crate) use batch_export::{ZipFileEntry, create_zip_archive};
pub(crate) use byte_document::ByteDocument;
#[cfg(test)]
pub(crate) use byte_document::{BytePiece, BytePieceSource, ByteSource, PieceBytes};
pub(crate) use editor_preferences::EditorMode;
pub use editor_preferences::ThemePreference;
#[cfg(test)]
pub(crate) use editor_tab::create_tab_from_bytes;
#[cfg(test)]
pub(crate) use editor_tab::document_for_tab;
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use editor_tab::tab_surface_for_document;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use editor_tab::text_surface_from_arc;
pub(crate) use editor_tab::{
    EditorTabState, RtonSurfaceCache, TabTaskState, TextContentState, TextSurfaceCache,
    create_tab_from_byte_document, create_text_tab, create_text_tab_from_surface,
    default_expanded_paths, empty_editor_text, empty_tree_rows, text_surface_from_text,
};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use editor_tab::{document_for_owned_tab, value_tree_rows_for_doc_with_expansion};
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use editor_tab::{value_search_result_for_doc, value_tree_rows_for_doc};
#[cfg(target_arch = "wasm32")]
pub(crate) use file_paths::normalize_display_path;
pub(crate) use file_paths::{
    export_rton_name, export_text_name, file_list_file_key, file_list_tab_key,
    is_loadable_display_name, leaf_display_name, loadable_file_kind_label,
    parse_file_list_file_key, parse_file_list_tab_key,
};
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use hex_edit::find_hex_search_result;
#[cfg(test)]
pub(crate) use hex_edit::{
    HEX_SEARCH_MATCH_DISPLAY_LIMIT, HexUndoEdit, apply_hex_edit, apply_hex_edits, edited_len,
    find_hex_search_matches, overwrite_byte_range, parse_ascii_pattern, parse_hex_pattern,
    replace_byte_span,
};
pub(crate) use hex_edit::{
    HexEdit, HexHistory, HexSearchMatch, HexSearchMode, HexSearchResult, edited_len_after_edits,
    find_containing_match, hex_search_status_text, parse_replace_pattern, parse_search_pattern,
    prepare_hex_edits, push_hex_past_batch, push_hex_redo_batch, push_hex_undo_batch,
    relative_search_match, replace_all_byte_edits,
};
pub(crate) use identity_arc::IdentityArc;
pub(crate) use rton_editor_core::TextPosition;
pub(crate) use rton_editor_core::TextSearchMatch;
#[cfg(test)]
pub(crate) use rton_editor_core::find_text_search_result;
#[cfg(test)]
pub(crate) use rton_editor_core::locate_value_path_in_text;
#[cfg(test)]
pub(crate) use rton_editor_core::offset_to_text_position;
#[cfg(test)]
pub(crate) use rton_inspector::RtonStringMode;
pub(crate) use rton_inspector::{
    format_inspector_offset, inspect_ascii_run, inspect_rton_payload, inspect_rton_string_info,
    inspect_special_region, locate_rton_value_offset, maybe_collect_string_tables,
    read_rton_varint, rton_tag_info, string_mode_label,
};
pub(crate) use text_buffer::{TextBuffer, TextRangeReplacement};
pub(crate) use text_history::{
    TextHistory, TextHistoryEntry, TextRangeHistoryEntry, can_record_text_undo,
    push_text_past_range, push_text_past_snapshot, push_text_redo_range, push_text_redo_snapshot,
    push_text_undo_range, push_text_undo_snapshot,
};
#[cfg(test)]
pub(crate) use text_search::replace_all_text_matches;
pub(crate) use text_search::{
    next_text_search_index, previous_text_search_index, replace_all_text_query, replace_text_span,
    text_search_status_text,
};
pub(crate) use toolbar_layout::{
    DropMarker, DropPlacement, ToolbarDropTarget, ToolbarGroupId, apply_toolbar_drop_target,
    normalize_toolbar_rows, toolbar_rows_to_json,
};
#[cfg(test)]
pub(crate) use toolbar_layout::{
    default_toolbar_rows, move_toolbar_group, move_toolbar_group_to_row_end,
};
pub(crate) use virtual_scroll::{
    FILE_LIST_DEFAULT_VIEWPORT_HEIGHT, FILE_LIST_ROW_HEIGHT, FileListVirtualScroll,
    HEX_DEFAULT_VIEWPORT_HEIGHT, TEXT_DEFAULT_VIEWPORT_HEIGHT, TEXT_ROW_HEIGHT,
    VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT, VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT,
    ValueTreeVirtualScroll, file_list_virtual_row_top, file_list_virtual_scroll,
    hex_scroll_top_for_row, hex_virtual_row_top, hex_virtual_scroll,
    measured_file_list_viewport_height, measured_hex_viewport_height,
    measured_text_viewport_height, measured_value_search_viewport_height,
    measured_value_tree_viewport_height, text_virtual_row_top, text_virtual_scroll,
    value_search_virtual_row_top, value_search_virtual_scroll, value_tree_virtual_row_top,
    value_tree_virtual_scroll,
};
#[cfg(test)]
pub(crate) use virtual_scroll::{
    FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT, HEX_MAX_VIEWPORT_ROWS, HEX_MAX_VIRTUAL_SCROLL_HEIGHT,
    HEX_OVERSCAN_ROWS, HEX_ROW_HEIGHT, HEX_VISIBLE_ROWS, VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT,
    VALUE_SEARCH_ROW_HEIGHT, VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT, VALUE_TREE_ROW_HEIGHT,
    file_list_viewport_rows, hex_viewport_rows, value_search_viewport_rows,
    value_tree_viewport_rows,
};
