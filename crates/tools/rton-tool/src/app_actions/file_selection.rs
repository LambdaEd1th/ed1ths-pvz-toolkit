use dioxus::prelude::*;
use rton_editor_core::{BinaryEncoding, EncodeOptions};
use std::sync::Arc;

use crate::batch_export_runner::batch_export_selected_files;
use crate::components::{FileListItem, FileSelection};
use crate::domain::{BatchExportMode, EditorTabState, Status, Tone};
use crate::file_import::LoadedFileState;
use crate::i18n::I18n;

pub(crate) fn select_visible_files(
    search_query: &str,
    mut file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    file_selection.write().select_visible(search_query);
    status.set(Status::new(
        i18n.t("status-visible-files-selected"),
        Tone::Info,
    ));
}

pub(crate) fn clear_visible_file_selection(
    search_query: &str,
    mut file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    file_selection.write().clear_visible(search_query);
    status.set(Status::new(
        i18n.t("status-file-selection-cleared"),
        Tone::Info,
    ));
}

pub(crate) fn toggle_selected_file_key(
    key: String,
    checked: bool,
    mut file_selection: Signal<FileSelection>,
) {
    file_selection.write().toggle_key(key, checked);
}

pub(crate) fn toggle_selected_file_path(
    path: String,
    checked: bool,
    mut file_selection: Signal<FileSelection>,
) {
    file_selection.write().toggle_path(path, checked);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn batch_export_selected_file_items(
    mode: BatchExportMode,
    file_list_items: Arc<Vec<FileListItem>>,
    file_selection: Signal<FileSelection>,
    tabs: Signal<Vec<EditorTabState>>,
    loaded_files: Signal<Vec<LoadedFileState>>,
    compact_output: Signal<bool>,
    encrypt_output: Signal<bool>,
    status: Signal<Status>,
    i18n: I18n,
) {
    let selected_items = file_selection.read().selected_items(&file_list_items);
    let tabs_snapshot = tabs.read().clone();
    let loaded_files_snapshot = loaded_files.read().clone();
    let options = EncodeOptions {
        encoding: if *compact_output.read() {
            BinaryEncoding::Compact
        } else {
            BinaryEncoding::Standard
        },
        encrypted: *encrypt_output.read(),
    };

    spawn(async move {
        batch_export_selected_files(
            selected_items,
            tabs_snapshot,
            loaded_files_snapshot,
            mode,
            options,
            status,
            i18n,
        )
        .await;
    });
}
