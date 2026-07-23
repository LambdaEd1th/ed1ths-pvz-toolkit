use crate::app_actions::{
    create_blank_tab_state, redo_hex_tab, redo_text_tab, reorder_tabs_by_id, tab_can_redo,
    tab_can_undo, tab_requires_close_confirmation, undo_hex_tab, undo_text_tab,
};
use crate::app_constants::{PANEL_MAX_WIDTH, PANEL_MIN_WIDTH, TEXT_SEARCH_MATCH_DISPLAY_LIMIT};
use crate::app_i18n::{load_i18n_sources, resolve_locale};
use crate::app_sample::SAMPLE_JSON;
use crate::components::{
    FileListItem, FileSelection, FileTreeNode, build_file_tree, clamp_panel_width,
    collect_file_tree_keys, value_tree_visible_indices,
};
use crate::domain::{
    BatchExportMode, ByteDocument, BytePiece, BytePieceSource, ByteSource, DropPlacement,
    EditorMode, EditorTabState, FILE_LIST_DEFAULT_VIEWPORT_HEIGHT,
    FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT, FILE_LIST_ROW_HEIGHT, HEX_DEFAULT_VIEWPORT_HEIGHT,
    HEX_MAX_VIEWPORT_ROWS, HEX_MAX_VIRTUAL_SCROLL_HEIGHT, HEX_OVERSCAN_ROWS, HEX_ROW_HEIGHT,
    HEX_SEARCH_MATCH_DISPLAY_LIMIT, HEX_VISIBLE_ROWS, HexEdit, HexHistory, HexSearchMatch,
    HexUndoEdit, PieceBytes, RtonStringMode, TextContentState, TextHistory, TextSearchMatch,
    ThemePreference, ToolbarGroupId, VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT,
    VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT, VALUE_SEARCH_ROW_HEIGHT,
    VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT, VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT,
    VALUE_TREE_ROW_HEIGHT, ZipFileEntry, apply_hex_edit, apply_hex_edits, batch_output_path,
    create_tab_from_byte_document, create_tab_from_bytes, create_text_tab, create_zip_archive,
    default_expanded_paths, default_toolbar_rows, document_for_tab, edited_len, empty_editor_text,
    empty_tree_rows, encode_batch_export_document, export_rton_name, file_list_file_key,
    file_list_viewport_rows, file_list_virtual_row_top, file_list_virtual_scroll,
    find_hex_search_matches, find_hex_search_result, find_text_search_result, hex_viewport_rows,
    hex_virtual_row_top, hex_virtual_scroll, inspect_rton_payload, inspect_rton_string_info,
    is_loadable_display_name, leaf_display_name, locate_rton_value_offset,
    locate_value_path_in_text, maybe_collect_string_tables, measured_hex_viewport_height,
    move_toolbar_group, move_toolbar_group_to_row_end, normalize_toolbar_rows,
    offset_to_text_position, overwrite_byte_range, parse_ascii_pattern, parse_hex_pattern,
    prepare_hex_edits, push_hex_undo_batch, push_text_undo_snapshot, read_rton_varint,
    replace_all_byte_edits, replace_all_text_matches, replace_all_text_query, replace_byte_span,
    replace_text_span, rton_tag_info, tab_surface_for_document, toolbar_rows_to_json,
    unique_zip_path, value_search_result_for_doc, value_search_viewport_rows,
    value_search_virtual_row_top, value_search_virtual_scroll, value_tree_rows_for_doc,
    value_tree_viewport_rows, value_tree_virtual_row_top, value_tree_virtual_scroll,
};
use crate::i18n::{self, I18n, Locale};
use rton_editor_core::{
    BinaryEncoding, DecodedDocument, ENCRYPTED_RTON_PREFIX, EncodeOptions, RtonValue, TextFormat,
    ValueRow, decode_rton_reader, encode_rton_bytes, flatten_value_tree, parse_text, value_to_text,
};
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

fn install_test_i18n() {
    i18n::clear_locales_for_tests();
    assert!(load_i18n_sources() >= 5);
}

fn tab(id: usize) -> EditorTabState {
    EditorTabState {
        id,
        content_revision: 0,
        file_name: format!("sample-{id}.json"),
        doc: None,
        stats: None,
        worker_document_id: None,
        worker_surface_mode: None,
        byte_doc: None,
        source_encode_options: EncodeOptions::default(),
        tree_rows: empty_tree_rows(),
        search_result: None,
        editor_text: empty_editor_text(),
        text_buffer: None,
        mode: EditorMode::Json,
        search_query: String::new(),
        selected_path: "$".to_string(),
        text_history: TextHistory::default(),
        hex_history: HexHistory::default(),
        text_state: TextContentState::None,
        task_state: None,
        task_generation: 0,
        tree_generation: 0,
        search_generation: 0,
        expanded_paths: default_expanded_paths(),
        text_cache: Vec::new(),
        rton_cache: None,
        dirty: false,
    }
}

fn tab_ids(tabs: &[EditorTabState]) -> Vec<usize> {
    tabs.iter().map(|tab| tab.id).collect()
}

fn value_tree_test_row(
    path: &str,
    label: &str,
    kind: &str,
    depth: usize,
    child_count: usize,
) -> ValueRow {
    ValueRow {
        path: path.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        preview: String::new(),
        depth,
        child_count,
    }
}

fn file_list_item_for_tree(id: usize, path: &str) -> FileListItem {
    FileListItem {
        key: file_list_file_key(id),
        file_id: Some(id),
        tab_id: None,
        path: path.to_string(),
        name: leaf_display_name(path),
        detail: "RTON".to_string(),
        active: false,
        dirty: false,
        closeable: true,
    }
}

mod documents;
mod exports;
mod file_list;
mod i18n_tests;
mod search_hex;
mod tabs_toolbar;
mod virtual_scroll;
