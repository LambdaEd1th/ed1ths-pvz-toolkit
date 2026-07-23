use dioxus::prelude::*;

#[cfg(test)]
use crate::domain::DropPlacement;
use crate::domain::{DropMarker, EditorTabState, Status, Tone};
use crate::file_import::LoadedFileState;
use crate::i18n::I18n;

use super::workspace_reducer::{
    remove_file_list_item_by_key_from_state, remove_tab_by_id_from_state,
    reorder_tabs_by_id_in_state,
};

pub(crate) fn tab_requires_close_confirmation(tabs: &[EditorTabState], id: usize) -> bool {
    tabs.iter().any(|tab| tab.id == id && tab.dirty)
}

pub(crate) fn close_tab_by_id(
    id: usize,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    if let Some(closed_name) = remove_tab_by_id(id, tabs, active_tab_id) {
        status.set(Status::new(
            i18n.t_args("status-closed", &[("name", closed_name)]),
            Tone::Info,
        ));
    }
}

pub(crate) fn remove_file_list_item_by_key(
    key: String,
    mut loaded_files: Signal<Vec<LoadedFileState>>,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) -> Option<String> {
    let removed = remove_file_list_item_by_key_from_state(&key, &mut loaded_files.write())?;

    if let Some(tab_id) = removed.removed_tab_id {
        if removed.name.is_empty() {
            return remove_tab_by_id(tab_id, tabs, active_tab_id);
        }
        remove_tab_by_id(tab_id, tabs, active_tab_id);
    }
    Some(removed.name)
}

pub(crate) fn remove_file_list_items_by_keys(
    keys: Vec<String>,
    loaded_files: Signal<Vec<LoadedFileState>>,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) -> usize {
    keys.into_iter()
        .filter(|key| {
            remove_file_list_item_by_key(key.clone(), loaded_files, tabs, active_tab_id).is_some()
        })
        .count()
}

pub(crate) fn remove_tab_by_id(
    id: usize,
    mut tabs: Signal<Vec<EditorTabState>>,
    mut active_tab_id: Signal<usize>,
) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    let worker_document_id = tabs
        .read()
        .iter()
        .find(|tab| tab.id == id)
        .and_then(|tab| tab.worker_document_id);
    let removed = remove_tab_by_id_from_state(&mut tabs.write(), *active_tab_id.read(), id)?;

    #[cfg(target_arch = "wasm32")]
    if let Some(document_id) = worker_document_id {
        spawn(async move {
            let _ = crate::platform::release_worker_document(document_id).await;
        });
    }

    if let Some(next_active) = removed.next_active_id {
        active_tab_id.set(next_active);
    }

    Some(removed.name)
}

pub(crate) fn update_tab(
    mut tabs: Signal<Vec<EditorTabState>>,
    id: usize,
    update: impl FnOnce(&mut EditorTabState),
) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == id) {
        update(tab);
    }
}

pub(crate) fn finish_tab_drag_state(
    mut tabs: Signal<Vec<EditorTabState>>,
    mut dragged_tab_id: Signal<Option<usize>>,
    mut tab_drop_marker: Signal<Option<DropMarker<usize>>>,
) {
    if let (Some(dragged_id), Some(marker)) = (*dragged_tab_id.read(), *tab_drop_marker.read()) {
        reorder_tabs_by_id_in_state(&mut tabs.write(), dragged_id, marker.id, marker.placement);
    }
    dragged_tab_id.set(None);
    tab_drop_marker.set(None);
}

#[cfg(test)]
pub(crate) fn reorder_tabs_by_id(
    tabs: &mut Vec<EditorTabState>,
    dragged_id: usize,
    target_id: usize,
    placement: DropPlacement,
) {
    reorder_tabs_by_id_in_state(tabs, dragged_id, target_id, placement);
}

pub(crate) fn active_tab(
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) -> Option<EditorTabState> {
    let active_id = *active_tab_id.read();
    tabs.read().iter().find(|tab| tab.id == active_id).cloned()
}
