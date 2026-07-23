mod edit;
mod file;
mod preferences;
mod rton_export;
mod settings;
mod text_export;

use dioxus::prelude::*;
use rton_editor_core::TextFormat;

use crate::components::FileSelection;
use crate::domain::{EditorMode, Status, ToolbarGroupId};
use crate::file_import::LoadedFileState;
use crate::i18n::I18n;

use edit::EditToolbarGroup;
use file::FileToolbarGroup;
pub(super) use file::WebFileOpenControl;
use preferences::PreferencesToolbarGroup;
use rton_export::RtonExportToolbarGroup;
pub(super) use settings::SettingsDialog;
use text_export::TextExportToolbarGroup;

#[component]
pub(super) fn ToolbarGroupContent(
    group_id: ToolbarGroupId,
    i18n: I18n,
    active_mode_snapshot: Option<EditorMode>,
    active_file_label: String,
    compact_snapshot: bool,
    encrypt_snapshot: bool,
    line_wrapping_snapshot: bool,
    editor_search_panel_visible_snapshot: bool,
    can_undo_snapshot: bool,
    can_redo_snapshot: bool,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    encrypt_output: Signal<bool>,
    line_wrapping: Signal<bool>,
    editor_search_panel_visible: Signal<bool>,
    status: Signal<Status>,
    open_native_files: EventHandler<()>,
    open_native_folder: EventHandler<()>,
    on_files_staged: EventHandler<()>,
    undo_edit: EventHandler<()>,
    redo_edit: EventHandler<()>,
    on_compact_change: EventHandler<bool>,
    export_text: EventHandler<TextFormat>,
    parse_current: EventHandler<()>,
    export_rton: EventHandler<()>,
) -> Element {
    match group_id {
        ToolbarGroupId::File => rsx! {
            FileToolbarGroup {
                i18n,
                active_file_label,
                loaded_files,
                next_loaded_file_id,
                file_selection,
                status,
                open_native_files,
                open_native_folder,
                on_files_staged
            }
        },
        ToolbarGroupId::Edit => rsx! {
            EditToolbarGroup {
                i18n,
                can_undo_snapshot,
                can_redo_snapshot,
                undo_edit,
                redo_edit
            }
        },
        ToolbarGroupId::TextExport => rsx! {
            TextExportToolbarGroup {
                active: active_mode_snapshot.is_some(),
                export_text
            }
        },
        ToolbarGroupId::RtonExport => rsx! {
            RtonExportToolbarGroup {
                i18n,
                active: active_mode_snapshot.is_some(),
                compact_snapshot,
                encrypt_snapshot,
                encrypt_output,
                on_compact_change,
                parse_current,
                export_rton
            }
        },
        ToolbarGroupId::Preferences => rsx! {
            PreferencesToolbarGroup {
                i18n,
                active: active_mode_snapshot.is_some(),
                line_wrapping_snapshot,
                editor_search_panel_visible_snapshot,
                line_wrapping,
                editor_search_panel_visible
            }
        },
    }
}
