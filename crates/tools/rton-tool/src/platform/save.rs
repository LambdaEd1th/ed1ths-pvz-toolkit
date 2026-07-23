#[cfg(not(target_arch = "wasm32"))]
pub fn save_bytes(default_file_name: &str, bytes: &[u8]) -> Result<bool, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_file_name)
        .save_file()
    else {
        return Ok(false);
    };

    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(target_arch = "wasm32")]
pub fn save_bytes(default_file_name: &str, bytes: &[u8]) -> Result<bool, String> {
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
    anchor.set_download(default_file_name);
    anchor.click();
    web_sys::Url::revoke_object_url(&url).map_err(|error| format!("{error:?}"))?;
    Ok(true)
}

pub fn save_text(default_file_name: &str, text: &str) -> Result<bool, String> {
    save_bytes(default_file_name, text.as_bytes())
}
