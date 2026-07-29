// Keep every native file dialog asynchronous. A synchronous NSSavePanel/NSOpenPanel
// starts a nested macOS event loop, which can re-enter Dioxus while it is handling
// the originating event and abort during a second VDOM diff.

#[cfg(not(target_arch = "wasm32"))]
pub async fn pick_archive() -> Option<std::path::PathBuf> {
    // macOS cannot always resolve a UTI for PopCap's private `.rsb`
    // extension, which leaves otherwise valid files disabled in NSOpenPanel.
    // The loader validates the semantic `rsb1` magic after selection.
    rfd::AsyncFileDialog::new()
        .pick_file()
        .await
        .map(|file| file.path().to_path_buf())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn pick_files() -> Result<Vec<crate::editing::AddedFile>, String> {
    rfd::AsyncFileDialog::new()
        .pick_files()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| {
            let path = file.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .ok_or_else(|| format!("无法读取文件名：{}", path.display()))?;
            let data = std::fs::read(path)
                .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
            Ok(crate::editing::AddedFile { name, data })
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn pick_replacement() -> Result<Option<crate::editing::AddedFile>, String> {
    let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else {
        return Ok(None);
    };
    let path = file.path();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("无法读取文件名：{}", path.display()))?;
    let data =
        std::fs::read(path).map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    Ok(Some(crate::editing::AddedFile { name, data }))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn pick_rsg_replacement() -> Result<Option<crate::editing::AddedFile>, String> {
    // Keep unknown extensions selectable on macOS; the RSG reader validates
    // the semantic `rsgp` magic before the file is accepted.
    let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else {
        return Ok(None);
    };
    let path = file.path();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| format!("无法读取文件名：{}", path.display()))?;
    let data =
        std::fs::read(path).map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    Ok(Some(crate::editing::AddedFile { name, data }))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedArchive {
    #[cfg(not(target_arch = "wasm32"))]
    Native(std::path::PathBuf),
    #[cfg(target_arch = "wasm32")]
    Downloaded,
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_archive(
    default_name: &str,
    bytes: &[u8],
    overwrite: Option<&std::path::Path>,
) -> Result<Option<SavedArchive>, String> {
    let path = if let Some(path) = overwrite {
        path.to_path_buf()
    } else {
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_file_name(default_name)
            .save_file()
            .await
        else {
            return Ok(None);
        };
        file.path().to_path_buf()
    };
    write_atomic(&path, bytes)?;
    Ok(Some(SavedArchive::Native(path)))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.rsb");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    for attempt in 0..16_u8 {
        let temporary = parent.join(format!(
            ".{file_name}.toolkit-{}-{stamp}-{attempt}.tmp",
            std::process::id()
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("无法在 {} 创建临时文件：{error}", parent.display()));
            }
        };
        let write_result = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .and_then(|()| std::fs::rename(&temporary, path));
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temporary);
            return Err(format!("无法安全写入 {}：{error}", path.display()));
        }
        return Ok(());
    }
    Err(format!("无法为 {} 分配临时保存文件", path.display()))
}

#[cfg(target_arch = "wasm32")]
pub async fn save_archive(
    default_name: &str,
    bytes: &[u8],
    _overwrite: Option<&std::path::Path>,
) -> Result<Option<SavedArchive>, String> {
    save_bytes(default_name, bytes)
        .await
        .map(|saved| saved.then_some(SavedArchive::Downloaded))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_bytes(default_name: &str, bytes: &[u8]) -> Result<bool, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name(default_name)
        .save_file()
        .await
    else {
        return Ok(false);
    };
    std::fs::write(file.path(), bytes).map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(target_arch = "wasm32")]
pub async fn save_bytes(default_name: &str, bytes: &[u8]) -> Result<bool, String> {
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

#[cfg(not(target_arch = "wasm32"))]
pub fn preview_url(bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;

    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(target_arch = "wasm32")]
pub fn preview_url(bytes: &[u8]) -> Result<String, String> {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(array.as_ref());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|error| format!("{error:?}"))?;
    web_sys::Url::create_object_url_with_blob(&blob).map_err(|error| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn release_preview_url(_url: &str) {}

#[cfg(target_arch = "wasm32")]
pub fn release_preview_url(url: &str) {
    if url.starts_with("blob:") {
        let _ = web_sys::Url::revoke_object_url(url);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn copy_preview_image(url: &str) -> Result<(), String> {
    use base64::Engine;
    use std::borrow::Cow;

    let encoded = url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| "预览图像不是可复制的 PNG Data URL".to_string())?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("无法读取预览 PNG：{error}"))?;
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|error| format!("无法解码预览 PNG：{error}"))?
        .into_rgba8();
    let width = usize::try_from(image.width()).map_err(|_| "图像宽度超出范围".to_string())?;
    let height = usize::try_from(image.height()).map_err(|_| "图像高度超出范围".to_string())?;
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("无法访问系统剪贴板：{error}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width,
            height,
            bytes: Cow::Owned(image.into_raw()),
        })
        .map_err(|error| format!("无法复制图像：{error}"))
}

#[cfg(target_arch = "wasm32")]
pub async fn copy_preview_image(url: &str) -> Result<(), String> {
    use base64::Engine;

    let encoded_url = base64::engine::general_purpose::STANDARD.encode(url.as_bytes());
    let script = format!(
        r#"
        (async () => {{
            try {{
                if (!navigator.clipboard?.write || typeof ClipboardItem !== "function") {{
                    throw new Error("当前浏览器不支持复制 PNG 图像");
                }}
                const url = atob("{encoded_url}");
                const response = await fetch(url);
                if (!response.ok) {{
                    throw new Error(`无法读取预览图像：${{response.status}}`);
                }}
                const source = await response.blob();
                const png = source.type === "image/png"
                    ? source
                    : new Blob([await source.arrayBuffer()], {{ type: "image/png" }});
                await navigator.clipboard.write([
                    new ClipboardItem({{ "image/png": png }})
                ]);
                dioxus.send("ok");
            }} catch (error) {{
                dioxus.send(`error:${{error?.message ?? String(error)}}`);
            }}
        }})();
        "#
    );
    let mut evaluator = dioxus::document::eval(&script);
    let result = evaluator
        .recv::<String>()
        .await
        .map_err(|error| format!("无法读取剪贴板操作结果：{error}"))?;
    match result.strip_prefix("error:") {
        Some(error) => Err(error.to_string()),
        None if result == "ok" => Ok(()),
        None => Err("浏览器没有确认图像复制结果".to_string()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn extract_entries(entries: &[(String, Vec<u8>)]) -> Result<Option<usize>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(None);
    };
    let root = folder.path();
    let mut written = 0;
    for (relative, bytes) in entries {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, bytes).map_err(|error| error.to_string())?;
        written += 1;
    }
    Ok(Some(written))
}
