pub(crate) mod context_menu_transition;
pub(crate) mod file_list;
pub(crate) mod hex_editor;
mod log_viewer_dialog;
pub(crate) mod panels;
pub(crate) mod tab_strip;
pub(crate) mod text_editor;
pub(crate) mod ui_helpers;
pub(crate) mod unsaved_changes_dialog;
pub(crate) mod value_panels;

pub(crate) use file_list::{
    FileList, FileListItem, FileSelection, file_item_matches_search, file_path_matches_scope,
};
#[cfg(test)]
pub(crate) use file_list::{FileTreeNode, build_file_tree, collect_file_tree_keys};
pub(crate) use hex_editor::{HexEditor, HexJumpTarget};
pub(crate) use log_viewer_dialog::LogViewerDialog;
pub(crate) use panels::{MetaItem, PanelHeader};
pub(crate) use tab_strip::{TabHeader, TabStrip};
pub(crate) use text_editor::TextJumpTarget;
pub(crate) use ui_helpers::{button_class, lucide_icon};
pub(crate) use unsaved_changes_dialog::UnsavedChangesDialog;
#[cfg(test)]
pub(crate) use value_panels::value_tree_visible_indices;
pub(crate) use value_panels::{StatsGrid, ValueSearchResults, ValueTree};
