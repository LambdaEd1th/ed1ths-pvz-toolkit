#[cfg(target_arch = "wasm32")]
use std::io::{Cursor, Write};
use std::sync::Arc;

use pak_archive::{PakArchive, PakEntryKind};

fn decode_owned(bytes: &[u8]) -> Result<PakArchive, String> {
    pak_archive::from_bytes(bytes).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn decode_archive(bytes: Arc<[u8]>) -> Result<PakArchive, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let _ = sender.send(decode_owned(&bytes));
    });
    receiver
        .await
        .map_err(|_| "PAK 解析任务意外终止".to_string())?
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn decode_archive(bytes: Arc<[u8]>) -> Result<PakArchive, String> {
    decode_owned(&bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn encode_archive(archive: Arc<PakArchive>) -> Result<Vec<u8>, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let result = pak_archive::to_bytes(&archive).map_err(|error| error.to_string());
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| "PAK 编码任务意外终止".to_string())?
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn encode_archive(archive: Arc<PakArchive>) -> Result<Vec<u8>, String> {
    pak_archive::to_bytes(&archive).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn save_bytes(
    default_name: &str,
    filter_name: &str,
    extensions: &[&str],
    bytes: &[u8],
) -> Result<bool, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name(default_name)
        .add_filter(filter_name, extensions)
        .save_file()
        .await
    else {
        return Ok(false);
    };
    std::fs::write(file.path(), bytes)
        .map(|()| true)
        .map_err(|error| format!("无法保存文件：{error}"))
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn save_bytes(
    default_name: &str,
    _filter_name: &str,
    _extensions: &[&str],
    bytes: &[u8],
) -> Result<bool, String> {
    download(default_name, bytes)?;
    Ok(true)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn extract_entries(
    archive_name: &str,
    archive: Arc<PakArchive>,
    indices: Vec<usize>,
) -> Result<Option<usize>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_title(format!("提取 {archive_name}"))
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let root = folder.path().to_path_buf();
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let result = (|| {
            let mut written = 0_usize;
            for index in indices {
                let Some(entry) = archive.entry_at(index) else {
                    continue;
                };
                let relative = entry
                    .path()
                    .to_safe_relative_path()
                    .map_err(|error| format!("不安全的归档路径 {}：{error}", entry.path()))?;
                let target = root.join(relative);
                match entry.kind() {
                    PakEntryKind::Directory => {
                        std::fs::create_dir_all(&target)
                            .map_err(|error| format!("无法创建 {}：{error}", target.display()))?;
                    }
                    PakEntryKind::File | PakEntryKind::Symlink => {
                        if let Some(parent) = target.parent() {
                            std::fs::create_dir_all(parent).map_err(|error| {
                                format!("无法创建 {}：{error}", parent.display())
                            })?;
                        }
                        // Symlinks are intentionally exported as target-text files.
                        std::fs::write(&target, entry.data())
                            .map_err(|error| format!("无法写入 {}：{error}", target.display()))?;
                        written += 1;
                    }
                }
            }
            Ok(written)
        })();
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| "PAK 提取任务意外终止".to_string())?
        .map(Some)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn extract_entries(
    archive_name: &str,
    archive: Arc<PakArchive>,
    indices: Vec<usize>,
) -> Result<Option<usize>, String> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut written = 0_usize;
        for index in indices {
            let Some(entry) = archive.entry_at(index) else {
                continue;
            };
            let name = entry.path().to_string_lossy().replace('\\', "/");
            match entry.kind() {
                PakEntryKind::Directory => zip
                    .add_directory(name, options)
                    .map_err(|error| format!("无法创建导出目录：{error}"))?,
                PakEntryKind::File | PakEntryKind::Symlink => {
                    zip.start_file(name, options)
                        .map_err(|error| format!("无法创建导出文件：{error}"))?;
                    zip.write_all(entry.data())
                        .map_err(|error| format!("无法写入导出文件：{error}"))?;
                    written += 1;
                }
            }
        }
        zip.finish()
            .map_err(|error| format!("无法完成导出 ZIP：{error}"))?;
        let name = format!("{}-extracted.zip", archive_stem(archive_name));
        download(&name, cursor.get_ref())?;
        Ok(Some(written))
    }
}

#[cfg(target_arch = "wasm32")]
fn archive_stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

#[cfg(target_arch = "wasm32")]
fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "window 不可用".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "document 不可用".to_string())?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(array.as_ref());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(js_error_string)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error_string)?;
    let anchor = document
        .create_element("a")
        .map_err(js_error_string)?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "无法创建下载链接".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: wasm_bindgen::JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
