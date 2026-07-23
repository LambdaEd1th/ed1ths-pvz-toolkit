use std::sync::Arc;

use crate::components::{FileListItem, FileSelection, TabHeader, file_item_matches_search};
use crate::domain::{EditorTabState, IdentityArc, TextSearchMatch, text_search_status_text};
use crate::file_import::{LoadedFileState, build_file_list_items};
use crate::i18n::I18n;

pub(super) struct EditorSearchSnapshot {
    pub(super) matches: Arc<[TextSearchMatch]>,
    pub(super) match_count: usize,
    pub(super) status_text: String,
    pub(super) controls_disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorSearchResultSnapshot {
    pub(super) matches: Arc<[TextSearchMatch]>,
    pub(super) capped: bool,
}

pub(super) fn editor_search_result_from_core(
    result: rton_editor_core::TextSearchResult,
) -> EditorSearchResultSnapshot {
    EditorSearchResultSnapshot {
        matches: Arc::from(result.matches),
        capped: result.capped,
    }
}

pub(super) fn empty_editor_search_result_snapshot() -> EditorSearchResultSnapshot {
    EditorSearchResultSnapshot {
        matches: Arc::from([]),
        capped: false,
    }
}

pub(super) fn editor_search_snapshot(
    result: &EditorSearchResultSnapshot,
    query: &str,
    match_index: usize,
    i18n: I18n,
) -> EditorSearchSnapshot {
    let matches = result.matches.clone();
    let match_count = matches.len();
    let current_index = if match_count == 0 {
        None
    } else {
        Some(match_index.min(match_count - 1))
    };
    let status_text =
        text_search_status_text(query, match_count, current_index, result.capped, i18n);

    EditorSearchSnapshot {
        matches,
        match_count,
        status_text,
        controls_disabled: match_count == 0,
    }
}

pub(super) fn empty_editor_search_snapshot(query: &str, i18n: I18n) -> EditorSearchSnapshot {
    EditorSearchSnapshot {
        matches: Arc::from([]),
        match_count: 0,
        status_text: text_search_status_text(query, 0, None, false, i18n),
        controls_disabled: true,
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct FilePanelSnapshot {
    pub(super) all_items: IdentityArc<Vec<FileListItem>>,
    pub(super) filtered_items: IdentityArc<Vec<FileListItem>>,
    pub(super) empty_message: String,
    pub(super) selected_count: usize,
    pub(super) selected_visible_count: usize,
    pub(super) subtitle: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct FilePanelCacheKey {
    pub(super) loaded_files_fingerprint: u64,
    pub(super) tab_headers: Vec<TabHeader>,
    pub(super) active_tab_id: usize,
    pub(super) search_query: String,
    pub(super) selection: FileSelection,
    pub(super) locale_code: &'static str,
    pub(super) i18n_revision: u64,
}

pub(super) fn loaded_files_fingerprint(files: &[LoadedFileState]) -> u64 {
    let mut hasher = DefaultHasher::new();
    files.len().hash(&mut hasher);
    for file in files {
        file.id.hash(&mut hasher);
        file.display_name.hash(&mut hasher);
        file.size.hash(&mut hasher);
        file.tab_id.hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn tab_headers_for_tabs(tabs: &[EditorTabState]) -> Vec<TabHeader> {
    tabs.iter()
        .map(|tab| TabHeader {
            id: tab.id,
            file_name: tab.file_name.clone(),
            mode: tab.mode,
            dirty: tab.dirty,
            closeable: true,
        })
        .collect()
}

pub(super) fn file_panel_snapshot(
    loaded_files: &[LoadedFileState],
    tab_headers: &[TabHeader],
    active_tab_id: usize,
    search_query: &str,
    selection: &FileSelection,
    i18n: I18n,
) -> FilePanelSnapshot {
    let all_items = IdentityArc::new(build_file_list_items(
        loaded_files,
        tab_headers,
        active_tab_id,
        i18n,
    ));
    let search_active = !search_query.trim().is_empty();
    let filtered_items = if !search_active {
        all_items.clone()
    } else {
        IdentityArc::new(
            all_items
                .iter()
                .filter(|item| file_item_matches_search(item, search_query))
                .cloned()
                .collect::<Vec<_>>(),
        )
    };
    let empty_message = if !search_active {
        i18n.t("common-no-file")
    } else {
        i18n.t("file-list-no-matches")
    };
    let selected_count = selection.selected_count(&all_items);
    let selected_visible_count = filtered_items
        .iter()
        .filter(|item| selection.is_selected(item))
        .count();
    let subtitle = if !search_active {
        i18n.t_args(
            "file-list-selected-count",
            &[
                ("selected", selected_count.to_string()),
                ("total", all_items.len().to_string()),
            ],
        )
    } else {
        i18n.t_args(
            "file-list-match-count",
            &[
                ("visible", filtered_items.len().to_string()),
                ("total", all_items.len().to_string()),
                ("selected", selected_count.to_string()),
            ],
        )
    };

    FilePanelSnapshot {
        all_items,
        filtered_items,
        empty_message,
        selected_count,
        selected_visible_count,
        subtitle,
    }
}
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
