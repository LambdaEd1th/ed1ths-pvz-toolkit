mod list_items;
mod loading;
mod state;

#[cfg(target_arch = "wasm32")]
mod web_drop;

pub(crate) use list_items::build_file_list_items;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use loading::document_from_loaded_file_sync;
pub(crate) use loading::{
    create_tab_from_loaded_file, dropped_directory_files, file_data_display_name,
    loaded_file_draft_from_file_data, loaded_file_drafts_from_native,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use state::LoadedFileSource;
pub(crate) use state::{
    LoadedFileState, set_loaded_file_tab_id, stage_loaded_file_drafts, unlink_loaded_file_tab,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use web_drop::{
    collect_web_dropped_directory_files, loaded_file_draft_from_web_dropped, read_web_file_bytes,
};
