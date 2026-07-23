#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU8, Ordering};

use pam_viewer_core::WorkerInputFile;

use crate::state::{Locale, Preferences};

pub mod log_buffer {
    use std::collections::VecDeque;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};

    const MAX_ENTRIES: usize = 500;
    static LOGS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

    fn display_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
            .strip_prefix('v')
            .unwrap_or(env!("CARGO_PKG_VERSION"))
    }

    fn normalize_entry(entry: &str) -> String {
        entry.replace(
            &format!("PAM Viewer {}", display_version()),
            &format!("PAM Viewer v{}", display_version()),
        )
    }

    fn entries() -> MutexGuard<'static, VecDeque<String>> {
        LOGS.get_or_init(|| Mutex::new(VecDeque::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn timestamp() -> String {
        #[cfg(not(target_arch = "wasm32"))]
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        #[cfg(target_arch = "wasm32")]
        let seconds = (js_sys::Date::now() / 1_000.0) as u64;
        let seconds = seconds % 86_400;
        format!(
            "{:02}:{:02}:{:02}",
            seconds / 3_600,
            (seconds / 60) % 60,
            seconds % 60
        )
    }

    pub fn initialize() {
        let mut entries = entries();
        if entries.is_empty() {
            entries.push_back(format!(
                "[{}] [INFO] PAM Viewer v{}",
                timestamp(),
                display_version()
            ));
        }
    }

    pub fn push(level: &str, message: &str) {
        if message.is_empty() {
            return;
        }
        let mut entries = entries();
        entries.push_back(format!("[{}] [{level}] {message}", timestamp()));
        while entries.len() > MAX_ENTRIES {
            entries.pop_front();
        }
    }

    pub fn snapshot() -> String {
        entries()
            .iter()
            .map(|entry| normalize_entry(entry))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn clear() {
        entries().clear();
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn adds_the_version_prefix_to_legacy_entries() {
            let legacy = format!("[12:00:00] [INFO] PAM Viewer {}", display_version());
            let expected = format!("[12:00:00] [INFO] PAM Viewer v{}", display_version());
            assert_eq!(normalize_entry(&legacy), expected);
        }
    }
}

pub mod processing;

#[cfg(target_arch = "wasm32")]
pub mod web_renderer;

#[cfg(not(target_arch = "wasm32"))]
pub mod native_renderer;

#[cfg(target_arch = "wasm32")]
const SETTINGS_KEY: &str = "pam-viewer-settings-v2";

#[cfg(not(target_arch = "wasm32"))]
static NATIVE_SYSTEM_APPEARANCE: AtomicU8 = AtomicU8::new(0);

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_animation_folder() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn input_files_in_folder(root: &std::path::Path) -> Result<Vec<WorkerInputFile>, String> {
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("animation")
        .to_string();
    let mut paths = Vec::new();
    collect_file_paths(root, &mut paths)?;
    paths.sort();
    read_folder_paths(root, &root_name, &paths)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn input_files_from_paths(
    paths: &[(String, std::path::PathBuf)],
) -> Result<Vec<WorkerInputFile>, String> {
    let read_path = |(display_name, path): &(String, std::path::PathBuf)| {
        std::fs::read(path)
            .map(|bytes| WorkerInputFile::new(display_name.clone(), bytes))
            .map_err(|error| error.to_string())
    };
    if paths.len() >= 8 {
        use rayon::prelude::*;
        paths.par_iter().map(read_path).collect()
    } else {
        paths.iter().map(read_path).collect()
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn input_files_from_web_drop(
    event: web_sys::DragEvent,
) -> Result<Vec<WorkerInputFile>, String> {
    use wasm_bindgen::JsCast;

    let transfer = event
        .data_transfer()
        .ok_or_else(|| "drop has no data transfer".to_string())?;
    let items = transfer.items();
    let mut entries = Vec::<web_sys::FileSystemEntry>::new();
    for index in 0..items.length() {
        let Some(item) = items.get(index) else {
            continue;
        };
        if item.kind() != "file" {
            continue;
        }
        if let Some(entry) = item.webkit_get_as_entry().map_err(js_error)? {
            entries.push(entry);
        }
    }
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    while let Some(entry) = entries.pop() {
        if entry.is_file() {
            let path = entry.full_path().trim_start_matches('/').to_string();
            let file_entry = entry
                .dyn_into::<web_sys::FileSystemFileEntry>()
                .map_err(|_| "invalid dropped file entry".to_string())?;
            let file = file_from_entry(&file_entry).await?;
            let path = if path.is_empty() { file.name() } else { path };
            files.push((path, file));
        } else if entry.is_directory() {
            let directory = entry
                .dyn_into::<web_sys::FileSystemDirectoryEntry>()
                .map_err(|_| "invalid dropped directory entry".to_string())?;
            let reader = directory.create_reader();
            loop {
                let batch = directory_entries(&reader).await?;
                if batch.is_empty() {
                    break;
                }
                entries.extend(batch);
            }
        }
    }
    use futures_util::stream::{self, StreamExt};
    let mut output = stream::iter(files.into_iter().map(|(path, file)| async move {
        let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
            .await
            .map_err(js_error)?;
        Ok::<_, String>(WorkerInputFile::new(
            path,
            js_sys::Uint8Array::new(&buffer).to_vec(),
        ))
    }))
    .buffer_unordered(4)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    output.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
async fn file_from_entry(entry: &web_sys::FileSystemFileEntry) -> Result<web_sys::File, String> {
    use wasm_bindgen::JsCast;

    let entry = entry.clone();
    let promise = js_sys::Promise::new(&mut move |resolve, reject| {
        entry.file_with_callback_and_callback(&resolve, &reject);
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(js_error)?
        .dyn_into::<web_sys::File>()
        .map_err(|_| "dropped entry did not resolve to a file".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn directory_entries(
    reader: &web_sys::FileSystemDirectoryReader,
) -> Result<Vec<web_sys::FileSystemEntry>, String> {
    use wasm_bindgen::JsCast;

    let reader = reader.clone();
    let promise = js_sys::Promise::new(&mut move |resolve, reject| {
        if let Err(error) = reader.read_entries_with_callback_and_callback(&resolve, &reject) {
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &error);
        }
    });
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(js_error)?;
    Ok(js_sys::Array::from(&value)
        .iter()
        .filter_map(|entry| entry.dyn_into::<web_sys::FileSystemEntry>().ok())
        .collect())
}

#[cfg(target_arch = "wasm32")]
fn js_error(error: wasm_bindgen::JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_file_paths(
    directory: &std::path::Path,
    output: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_file_paths(&path, output)?;
        } else if file_type.is_file() {
            output.push(path);
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn read_folder_paths(
    root: &std::path::Path,
    root_name: &str,
    paths: &[std::path::PathBuf],
) -> Result<Vec<WorkerInputFile>, String> {
    let named_paths = paths
        .iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            (format!("{root_name}/{relative}"), path.clone())
        })
        .collect::<Vec<_>>();
    input_files_from_paths(&named_paths)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_bytes(default_name: &str, bytes: &[u8]) -> Result<bool, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_name)
        .save_file()
    else {
        return Ok(false);
    };
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn thumbnail_url(bytes: &[u8]) -> Option<String> {
    use base64::Engine;

    Some(format!(
        "data:{};base64,{}",
        image_mime(bytes),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(target_arch = "wasm32")]
pub fn thumbnail_url(bytes: &[u8]) -> Option<String> {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn release_thumbnail_url(_url: &str) {}

#[cfg(target_arch = "wasm32")]
pub fn release_thumbnail_url(url: &str) {
    if url.starts_with("blob:") {
        let _ = web_sys::Url::revoke_object_url(url);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_bytes(default_name: &str, bytes: &[u8]) -> Result<bool, String> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "document is unavailable".to_string())?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let blob =
        web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(|error| format!("{error:?}"))?;
    let url =
        web_sys::Url::create_object_url_with_blob(&blob).map_err(|error| format!("{error:?}"))?;
    let anchor = document
        .create_element("a")
        .map_err(|error| format!("{error:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "failed to create download anchor".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(default_name);
    anchor.click();
    web_sys::Url::revoke_object_url(&url).map_err(|error| format!("{error:?}"))?;
    Ok(true)
}

pub fn load_preferences() -> Preferences {
    let loaded = preference_text()
        .and_then(|text| serde_json::from_str::<Preferences>(&text).ok())
        .unwrap_or_else(|| Preferences {
            locale: detect_locale(),
            ..Preferences::default()
        });
    loaded.normalized()
}

pub fn save_preferences(preferences: &Preferences) {
    let Ok(text) = serde_json::to_string_pretty(preferences) else {
        return;
    };
    save_preference_text(&text);
}

#[cfg(target_arch = "wasm32")]
pub fn system_is_dark() -> bool {
    web_sys::window()
        .and_then(|window| window.match_media("(prefers-color-scheme: dark)").ok())
        .flatten()
        .is_some_and(|query| query.matches())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn system_is_dark() -> bool {
    match NATIVE_SYSTEM_APPEARANCE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => std::env::var("APPLE_INTERFACE_STYLE")
            .or_else(|_| std::env::var("GTK_THEME"))
            .is_ok_and(|value| value.to_ascii_lowercase().contains("dark")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_native_system_appearance(dark: bool) {
    NATIVE_SYSTEM_APPEARANCE.store(if dark { 2 } else { 1 }, Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
fn preference_text() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(SETTINGS_KEY)
        .ok()?
}

#[cfg(target_arch = "wasm32")]
fn save_preference_text(text: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|window| window.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item(SETTINGS_KEY, text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn preference_text() -> Option<String> {
    std::fs::read_to_string(preference_path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_preference_text(text: &str) {
    let Some(path) = preference_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}

#[cfg(not(target_arch = "wasm32"))]
fn preference_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("io", "LambdaEd1th", "pam-viewer")
        .map(|directories| directories.config_dir().join("settings.json"))
}

#[cfg(target_arch = "wasm32")]
fn detect_locale() -> Locale {
    web_sys::window()
        .and_then(|window| window.navigator().language())
        .filter(|language| language.to_ascii_lowercase().starts_with("zh"))
        .map(|_| Locale::ZhCn)
        .unwrap_or(Locale::En)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_locale() -> Locale {
    std::env::var("LANG")
        .ok()
        .filter(|language| language.to_ascii_lowercase().starts_with("zh"))
        .map(|_| Locale::ZhCn)
        .unwrap_or(Locale::En)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep_ms(milliseconds: u64) {
    futures_timer::Delay::new(std::time::Duration::from_millis(milliseconds)).await;
}

#[cfg(target_arch = "wasm32")]
pub async fn sleep_ms(milliseconds: u64) {
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use wasm_bindgen_futures::JsFuture;

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let Some(window) = web_sys::window() else {
            let _ = resolve.call0(&JsValue::UNDEFINED);
            return;
        };
        let callback = Closure::once(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        if let Err(error) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            milliseconds.min(i32::MAX as u64) as i32,
        ) {
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
            return;
        }
        callback.forget();
    });
    let _ = JsFuture::from(promise).await;
}
