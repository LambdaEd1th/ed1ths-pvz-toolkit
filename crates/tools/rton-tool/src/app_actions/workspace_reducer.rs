use crate::domain::{
    DropPlacement, EditorTabState, parse_file_list_file_key, parse_file_list_tab_key,
};
use crate::file_import::LoadedFileState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemovedTab {
    pub(super) name: String,
    pub(super) next_active_id: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemovedFileListItem {
    pub(super) name: String,
    pub(super) removed_tab_id: Option<usize>,
}

pub(super) fn remove_tab_by_id_from_state(
    tabs: &mut Vec<EditorTabState>,
    active_tab_id: usize,
    id: usize,
) -> Option<RemovedTab> {
    let index = tabs.iter().position(|tab| tab.id == id)?;
    let name = tabs[index].file_name.clone();
    let was_active = tabs[index].id == active_tab_id;
    tabs.remove(index);
    let next_active_id = if tabs.is_empty() {
        Some(0)
    } else if was_active {
        let next_index = index.min(tabs.len().saturating_sub(1));
        Some(tabs[next_index].id)
    } else {
        None
    };

    Some(RemovedTab {
        name,
        next_active_id,
    })
}

pub(super) fn remove_file_list_item_by_key_from_state(
    key: &str,
    loaded_files: &mut Vec<LoadedFileState>,
) -> Option<RemovedFileListItem> {
    if let Some(file_id) = parse_file_list_file_key(key) {
        let index = loaded_files.iter().position(|file| file.id == file_id)?;
        let removed = loaded_files.remove(index);
        return Some(RemovedFileListItem {
            name: removed.display_name,
            removed_tab_id: removed.tab_id,
        });
    }

    if let Some(tab_id) = parse_file_list_tab_key(key) {
        for file in loaded_files.iter_mut() {
            if file.tab_id == Some(tab_id) {
                file.tab_id = None;
            }
        }
        return Some(RemovedFileListItem {
            name: String::new(),
            removed_tab_id: Some(tab_id),
        });
    }

    None
}

pub(super) fn reorder_tabs_by_id_in_state(
    tabs: &mut Vec<EditorTabState>,
    dragged_id: usize,
    target_id: usize,
    placement: DropPlacement,
) {
    let Some(source_index) = tabs.iter().position(|tab| tab.id == dragged_id) else {
        return;
    };
    let tab = tabs.remove(source_index);
    let Some(target_index) = tabs.iter().position(|candidate| candidate.id == target_id) else {
        tabs.insert(source_index.min(tabs.len()), tab);
        return;
    };
    let insert_index = match placement {
        DropPlacement::Before => target_index,
        DropPlacement::After => target_index + 1,
    };
    tabs.insert(insert_index.min(tabs.len()), tab);
}
