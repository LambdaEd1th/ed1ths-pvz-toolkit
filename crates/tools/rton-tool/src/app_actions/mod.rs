mod document;
mod editing;
mod export;
mod file_selection;
mod opening;
mod tabs;
mod text_editor_search;
mod toolbar;
mod workspace_reducer;

pub(crate) use document::{
    navigate_to_value_path, switch_active_mode, toggle_active_tree_path, update_active_search,
    update_compact_output, validate_active_tab,
};
pub(crate) use editing::{
    redo_active_edit, tab_can_redo, tab_can_undo, undo_active_edit, update_active_hex_edits,
    update_active_text_range,
};
pub(crate) use export::{export_active_rton, export_active_text};
pub(crate) use file_selection::{
    batch_export_selected_file_items, clear_visible_file_selection, select_visible_files,
    toggle_selected_file_key, toggle_selected_file_path,
};
pub(crate) use opening::{
    activate_tab_by_id, open_blank_tab, open_loaded_file_by_id, open_native_files_dialog,
    open_native_folder_dialog,
};
pub(crate) use tabs::{
    close_tab_by_id, finish_tab_drag_state, remove_file_list_item_by_key,
    remove_file_list_items_by_keys, tab_requires_close_confirmation,
};
pub(crate) use text_editor_search::{
    go_to_next_text_match, go_to_previous_text_match, handle_text_find_key,
    handle_text_replace_key, replace_all_text_matches_in_editor, replace_current_text_match,
};
pub(crate) use toolbar::{finish_toolbar_drag_state, initial_toolbar_rows};

#[cfg(test)]
pub(crate) use editing::{redo_hex_tab, redo_text_tab, undo_hex_tab, undo_text_tab};
#[cfg(test)]
pub(crate) use opening::create_blank_tab_state;
#[cfg(test)]
pub(crate) use tabs::reorder_tabs_by_id;
