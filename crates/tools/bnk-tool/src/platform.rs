use std::sync::Arc;

use bnk_archive::SoundBank;

#[derive(Debug)]
pub(crate) struct DecodedBank {
    pub bank: SoundBank,
    pub permissive: bool,
    pub strict_error: Option<String>,
}

fn decode_owned(bytes: &[u8]) -> Result<DecodedBank, String> {
    match bnk_archive::from_bytes(bytes) {
        Ok(bank) => Ok(DecodedBank {
            bank,
            permissive: false,
            strict_error: None,
        }),
        Err(strict_error) => bnk_archive::from_bytes_lossless(bytes)
            .map(|bank| DecodedBank {
                bank,
                permissive: true,
                strict_error: Some(strict_error.to_string()),
            })
            .map_err(|lossless_error| {
                format!("严格解析失败：{strict_error}；保留模式也失败：{lossless_error}")
            }),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn decode_bank(bytes: Arc<[u8]>) -> Result<DecodedBank, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let _ = sender.send(decode_owned(&bytes));
    });
    receiver
        .await
        .map_err(|_| "BNK 解析任务意外终止".to_string())?
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn encode_bank(bank: Arc<SoundBank>) -> Result<Vec<u8>, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let result = bnk_archive::to_bytes(&bank).map_err(|error| error.to_string());
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| "BNK 编码任务意外终止".to_string())?
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn encode_bank(bank: Arc<SoundBank>) -> Result<Vec<u8>, String> {
    bnk_archive::to_bytes(&bank).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn decode_bank(bytes: Arc<[u8]>) -> Result<DecodedBank, String> {
    decode_owned(&bytes)
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
