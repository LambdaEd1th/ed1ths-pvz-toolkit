mod dialogs;
mod i18n_sources;
pub mod log_buffer;
mod preferences;
mod save;
mod task;
mod worker;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOpenFile {
    pub path: std::path::PathBuf,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct I18nSource {
    pub code: String,
    pub source: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub use dialogs::files_in_folder;
pub use dialogs::{open_files, open_folder};
#[cfg(not(target_arch = "wasm32"))]
pub use i18n_sources::read_i18n_sources;
#[cfg(target_arch = "wasm32")]
pub use i18n_sources::read_i18n_sources_async;
#[cfg(not(target_arch = "wasm32"))]
pub use preferences::save_window_size_preference;
pub use preferences::{
    read_editor_mode_preference, read_line_wrapping_preference, read_locale_preference,
    save_editor_mode_preference, save_line_wrapping_preference, save_locale_preference,
    system_locale,
};
pub use save::{save_bytes, save_text};
#[cfg(not(target_arch = "wasm32"))]
pub use task::run_cpu_task;
pub use task::sleep_ms;
#[cfg(target_arch = "wasm32")]
pub use worker::{
    release_worker_document, run_batch_worker, run_hex_search_worker, run_locate_text_worker,
    run_mode_switch_worker, run_open_text_file_worker, run_open_text_worker, run_parse_worker,
    run_rton_size_worker, run_text_search_worker, run_tree_worker, run_value_search_worker,
};
