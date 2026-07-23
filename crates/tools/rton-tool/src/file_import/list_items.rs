use rton_editor_core::format_bytes;
use std::collections::{HashMap, HashSet};

use crate::components::{FileListItem, TabHeader};
use crate::domain::{
    file_list_file_key, file_list_tab_key, leaf_display_name, loadable_file_kind_label,
};
use crate::i18n::I18n;

use super::state::LoadedFileState;

pub(crate) fn build_file_list_items(
    loaded_files: &[LoadedFileState],
    tabs: &[TabHeader],
    active_tab_id: usize,
    i18n: I18n,
) -> Vec<FileListItem> {
    #[cfg(not(target_arch = "wasm32"))]
    if loaded_files.len().saturating_add(tabs.len()) >= 512 {
        return build_file_list_items_parallel(loaded_files, tabs, active_tab_id, i18n);
    }

    let mut items = Vec::new();
    let tabs_by_id = tabs
        .iter()
        .map(|tab| (tab.id, tab))
        .collect::<HashMap<_, _>>();
    let linked_tab_ids = loaded_files
        .iter()
        .filter_map(|file| file.tab_id)
        .collect::<HashSet<_>>();

    for file in loaded_files {
        let tab = file.tab_id.and_then(|id| tabs_by_id.get(&id).copied());
        let path = tab
            .map(|tab| tab.file_name.clone())
            .unwrap_or_else(|| file.display_name.clone());
        let name = leaf_display_name(&path);
        let detail = if let Some(tab) = tab {
            let size = file
                .size
                .map(format_bytes)
                .unwrap_or_else(|| i18n.t("common-empty"));
            format!("{size} · {}", tab.mode.label())
        } else {
            i18n.t_args(
                "file-list-closed-detail",
                &[
                    (
                        "size",
                        file.size
                            .map(format_bytes)
                            .unwrap_or_else(|| i18n.t("common-empty")),
                    ),
                    ("kind", loadable_file_kind_label(&file.display_name)),
                ],
            )
        };

        items.push(FileListItem {
            key: file_list_file_key(file.id),
            file_id: Some(file.id),
            tab_id: tab.map(|tab| tab.id),
            path,
            name,
            detail,
            active: tab.is_some_and(|tab| tab.id == active_tab_id),
            dirty: tab.is_some_and(|tab| tab.dirty),
            closeable: tab.is_some_and(|tab| tab.closeable),
        });
    }

    for tab in tabs {
        if linked_tab_ids.contains(&tab.id) {
            continue;
        }
        let path = tab.file_name.clone();
        items.push(FileListItem {
            key: file_list_tab_key(tab.id),
            file_id: None,
            tab_id: Some(tab.id),
            name: leaf_display_name(&path),
            path,
            detail: tab.mode.label().to_string(),
            active: tab.id == active_tab_id,
            dirty: tab.dirty,
            closeable: tab.closeable,
        });
    }

    items
}

#[cfg(not(target_arch = "wasm32"))]
fn build_file_list_items_parallel(
    loaded_files: &[LoadedFileState],
    tabs: &[TabHeader],
    active_tab_id: usize,
    i18n: I18n,
) -> Vec<FileListItem> {
    use rayon::prelude::*;

    let tabs_by_id = tabs
        .iter()
        .map(|tab| (tab.id, tab))
        .collect::<HashMap<_, _>>();
    let linked_tab_ids = loaded_files
        .iter()
        .filter_map(|file| file.tab_id)
        .collect::<HashSet<_>>();

    let mut drafts = loaded_files
        .par_iter()
        .map(|file| {
            let tab = file.tab_id.and_then(|id| tabs_by_id.get(&id).copied());
            let path = tab
                .map(|tab| tab.file_name.clone())
                .unwrap_or_else(|| file.display_name.clone());
            let name = leaf_display_name(&path);
            let detail = if let Some(tab) = tab {
                FileListDetailDraft::Open {
                    size: file.size.map(format_bytes),
                    mode: tab.mode.label(),
                }
            } else {
                FileListDetailDraft::Closed {
                    size: file.size.map(format_bytes),
                    kind: loadable_file_kind_label(&file.display_name),
                }
            };

            FileListItemDraft {
                key: file_list_file_key(file.id),
                file_id: Some(file.id),
                tab_id: tab.map(|tab| tab.id),
                path,
                name,
                detail,
                active: tab.is_some_and(|tab| tab.id == active_tab_id),
                dirty: tab.is_some_and(|tab| tab.dirty),
                closeable: tab.is_some_and(|tab| tab.closeable),
            }
        })
        .collect::<Vec<_>>();

    drafts.extend(
        tabs.par_iter()
            .filter(|tab| !linked_tab_ids.contains(&tab.id))
            .map(|tab| {
                let path = tab.file_name.clone();
                FileListItemDraft {
                    key: file_list_tab_key(tab.id),
                    file_id: None,
                    tab_id: Some(tab.id),
                    name: leaf_display_name(&path),
                    path,
                    detail: FileListDetailDraft::Ready(tab.mode.label().to_string()),
                    active: tab.id == active_tab_id,
                    dirty: tab.dirty,
                    closeable: tab.closeable,
                }
            })
            .collect::<Vec<_>>(),
    );

    drafts
        .into_iter()
        .map(|draft| draft.into_item(i18n))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
struct FileListItemDraft {
    key: String,
    file_id: Option<usize>,
    tab_id: Option<usize>,
    path: String,
    name: String,
    detail: FileListDetailDraft,
    active: bool,
    dirty: bool,
    closeable: bool,
}

#[cfg(not(target_arch = "wasm32"))]
enum FileListDetailDraft {
    Open {
        size: Option<String>,
        mode: &'static str,
    },
    Ready(String),
    Closed {
        size: Option<String>,
        kind: String,
    },
}

#[cfg(not(target_arch = "wasm32"))]
impl FileListItemDraft {
    fn into_item(self, i18n: I18n) -> FileListItem {
        let detail = match self.detail {
            FileListDetailDraft::Open { size, mode } => {
                let size = size.unwrap_or_else(|| i18n.t("common-empty"));
                format!("{size} · {mode}")
            }
            FileListDetailDraft::Ready(detail) => detail,
            FileListDetailDraft::Closed { size, kind } => i18n.t_args(
                "file-list-closed-detail",
                &[
                    ("size", size.unwrap_or_else(|| i18n.t("common-empty"))),
                    ("kind", kind),
                ],
            ),
        };
        FileListItem {
            key: self.key,
            file_id: self.file_id,
            tab_id: self.tab_id,
            path: self.path,
            name: self.name,
            detail,
            active: self.active,
            dirty: self.dirty,
            closeable: self.closeable,
        }
    }
}
