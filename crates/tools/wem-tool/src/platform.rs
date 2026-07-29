#[cfg(not(target_arch = "wasm32"))]
pub fn audio_url(bytes: &[u8], mime: &str) -> Result<String, String> {
    use base64::Engine;

    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(target_arch = "wasm32")]
pub fn audio_url(bytes: &[u8], mime: &str) -> Result<String, String> {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(array.as_ref());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(js_error_string)?;
    web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error_string)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn release_audio_url(_url: &str) {}

#[cfg(target_arch = "wasm32")]
pub fn release_audio_url(url: &str) {
    if url.starts_with("blob:") {
        let _ = web_sys::Url::revoke_object_url(url);
    }
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
    std::fs::write(file.path(), bytes)
        .map(|()| true)
        .map_err(|error| format!("无法保存文件：{error}"))
}

#[cfg(target_arch = "wasm32")]
pub async fn save_bytes(default_name: &str, bytes: &[u8]) -> Result<bool, String> {
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

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: wasm_bindgen::JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
