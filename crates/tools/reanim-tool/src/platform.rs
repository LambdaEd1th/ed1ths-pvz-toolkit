use reanim_codec::Reanim;

pub const XFL_AVAILABLE: bool = cfg!(not(target_arch = "wasm32"));

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_bytes(
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
pub async fn save_bytes(
    default_name: &str,
    _filter_name: &str,
    _extensions: &[&str],
    bytes: &[u8],
) -> Result<bool, String> {
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
    anchor.set_download(default_name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(true)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn open_xfl() -> Result<Option<(String, Reanim)>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(None);
    };
    let name = folder.file_name();
    reanim_codec::decode_xfl(folder.path())
        .map(|reanim| Some((format!("{name}.xfl"), reanim)))
        .map_err(|error| format!("无法导入 XFL：{error}"))
}

#[cfg(target_arch = "wasm32")]
pub async fn open_xfl() -> Result<Option<(String, Reanim)>, String> {
    Err("Web 端暂不支持选择 XFL 目录；请使用桌面版。".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_xfl(default_name: &str, reanim: &Reanim) -> Result<bool, String> {
    let Some(parent) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(false);
    };
    let mut output = parent.path().join(default_name);
    if output.exists() {
        for index in 2..10_000 {
            let candidate = parent.path().join(format!("{default_name}-{index}"));
            if !candidate.exists() {
                output = candidate;
                break;
            }
        }
    }
    reanim_codec::encode_xfl(reanim, &output)
        .map(|()| true)
        .map_err(|error| format!("无法导出 XFL：{error}"))
}

#[cfg(target_arch = "wasm32")]
pub async fn save_xfl(_default_name: &str, _reanim: &Reanim) -> Result<bool, String> {
    Err("Web 端暂不支持导出 XFL 目录；请先导出 JSON 或 compiled。".to_string())
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: wasm_bindgen::JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
