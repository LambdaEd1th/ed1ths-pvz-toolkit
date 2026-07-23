#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use rton_editor_core::encode_batch_export_document;
pub(crate) use rton_editor_core::{
    BatchExportMode, ZipArchiveBuilder, batch_output_path, unique_zip_path,
};
#[cfg(test)]
pub(crate) use rton_editor_core::{ZipFileEntry, create_zip_archive};

pub(crate) fn batch_archive_name(mode: BatchExportMode) -> String {
    format!(
        "rton-editor-{}-{}.zip",
        mode.archive_token(),
        timestamp_for_file_name()
    )
}

fn timestamp_for_file_name() -> String {
    current_unix_timestamp_seconds().to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
fn current_unix_timestamp_seconds() -> u64 {
    let millis = js_sys::Date::now();
    if !millis.is_finite() || millis <= 0.0 {
        0
    } else {
        (millis / 1000.0).floor() as u64
    }
}
