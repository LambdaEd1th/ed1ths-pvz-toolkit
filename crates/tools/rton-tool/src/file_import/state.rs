use dioxus::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

use crate::components::FileSelection;

#[derive(Debug, Clone)]
pub(crate) struct LoadedFileState {
    pub(crate) id: usize,
    pub(crate) display_name: String,
    pub(crate) source: LoadedFileSource,
    pub(crate) size: Option<usize>,
    pub(crate) tab_id: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) enum LoadedFileSource {
    NativePath(PathBuf),
    Bytes(Arc<[u8]>),
    #[cfg(target_arch = "wasm32")]
    WebFile(web_sys::File),
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedFileDraft {
    pub(crate) display_name: String,
    pub(crate) source: LoadedFileSource,
    pub(crate) size: Option<usize>,
}

pub(crate) fn stage_loaded_file_drafts(
    mut loaded_files: Signal<Vec<LoadedFileState>>,
    mut next_loaded_file_id: Signal<usize>,
    mut file_selection: Signal<FileSelection>,
    drafts: Vec<LoadedFileDraft>,
) -> usize {
    let mut next_id = *next_loaded_file_id.read();
    let files = drafts
        .into_iter()
        .map(|draft| {
            let id = next_id;
            next_id += 1;
            LoadedFileState {
                id,
                display_name: draft.display_name,
                source: draft.source,
                size: draft.size,
                tab_id: None,
            }
        })
        .collect::<Vec<_>>();
    let count = files.len();
    next_loaded_file_id.set(next_id);
    file_selection.write().clear();
    loaded_files.set(files);
    count
}

pub(crate) fn set_loaded_file_tab_id(
    mut loaded_files: Signal<Vec<LoadedFileState>>,
    file_id: usize,
    tab_id: Option<usize>,
) {
    if let Some(file) = loaded_files
        .write()
        .iter_mut()
        .find(|file| file.id == file_id)
    {
        file.tab_id = tab_id;
    }
}

pub(crate) fn unlink_loaded_file_tab(
    mut loaded_files: Signal<Vec<LoadedFileState>>,
    tab_id: usize,
) {
    for file in loaded_files.write().iter_mut() {
        if file.tab_id == Some(tab_id) {
            file.tab_id = None;
        }
    }
}
