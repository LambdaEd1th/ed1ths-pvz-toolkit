use std::io::Cursor;

use serde::{Deserialize, Serialize};
use smf_container::{
    EncodeOptions, SMF_MAGIC_BYTES, SmfMetadata, SmfVariant, decode, encode, inspect, md5_hex,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Smf,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderVariant {
    Compact32,
    Extended64,
}

impl HeaderVariant {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact32 => "32-bit / 8-byte",
            Self::Extended64 => "64-bit / 16-byte",
        }
    }
}

impl From<SmfVariant> for HeaderVariant {
    fn from(value: SmfVariant) -> Self {
        match value {
            SmfVariant::Compact32 => Self::Compact32,
            SmfVariant::Extended64 => Self::Extended64,
        }
    }
}

impl From<HeaderVariant> for SmfVariant {
    fn from(value: HeaderVariant) -> Self {
        match value {
            HeaderVariant::Compact32 => Self::Compact32,
            HeaderVariant::Extended64 => Self::Extended64,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub variant: HeaderVariant,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub total_size: u64,
    pub md5: String,
}

impl DocumentMetadata {
    fn new(value: SmfMetadata, encoded: &[u8]) -> Self {
        Self {
            variant: value.variant.into(),
            uncompressed_size: value.uncompressed_size,
            compressed_size: value.compressed_size,
            total_size: value.total_size,
            md5: md5_hex(encoded),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrepareRequest {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedDocument {
    pub name: String,
    pub source_kind: SourceKind,
    pub payload_kind: String,
    pub payload_name: String,
    pub smf_name: String,
    pub tag_name: String,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub encoded: Vec<u8>,
    pub metadata: DocumentMetadata,
    pub compression_level: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RebuildRequest {
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    pub variant: HeaderVariant,
    pub compression_level: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RebuiltDocument {
    #[serde(with = "serde_bytes")]
    pub encoded: Vec<u8>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(".tag.smf 是校验旁车文件，不能作为 SMF 容器打开")]
    TagSidecar,
    #[error("扩展名为 .smf，但文件没有有效的 SMF 魔数")]
    InvalidNamedSmf,
    #[error("SMF 处理失败：{0}")]
    Codec(String),
}

pub type Result<T> = std::result::Result<T, WorkerError>;

pub fn prepare(request: PrepareRequest) -> Result<PreparedDocument> {
    if request.name.to_ascii_lowercase().ends_with(".tag.smf") {
        return Err(WorkerError::TagSidecar);
    }
    let is_smf = request.data.starts_with(&SMF_MAGIC_BYTES);
    if !is_smf && request.name.to_ascii_lowercase().ends_with(".smf") {
        return Err(WorkerError::InvalidNamedSmf);
    }

    if is_smf {
        prepare_encoded(request)
    } else {
        prepare_raw(request)
    }
}

pub fn rebuild(request: RebuildRequest) -> Result<RebuiltDocument> {
    let options = EncodeOptions::new(request.variant.into(), request.compression_level);
    let encoded = encode(&request.payload, options).map_err(codec_error)?;
    let metadata = inspect(&mut Cursor::new(&encoded)).map_err(codec_error)?;
    Ok(RebuiltDocument {
        metadata: DocumentMetadata::new(metadata, &encoded),
        encoded,
    })
}

fn prepare_encoded(request: PrepareRequest) -> Result<PreparedDocument> {
    let metadata = inspect(&mut Cursor::new(&request.data)).map_err(codec_error)?;
    let payload = decode(Cursor::new(&request.data)).map_err(codec_error)?;
    let payload_name = decoded_name(&request.name);
    let smf_name = ensure_smf_name(&request.name);
    let tag_name = tag_name(&smf_name);
    Ok(PreparedDocument {
        name: request.name,
        source_kind: SourceKind::Smf,
        payload_kind: detect_payload_kind(&payload).to_string(),
        payload_name,
        smf_name,
        tag_name,
        payload,
        metadata: DocumentMetadata::new(metadata, &request.data),
        encoded: request.data,
        compression_level: 9,
    })
}

fn prepare_raw(request: PrepareRequest) -> Result<PreparedDocument> {
    let payload_kind = detect_payload_kind(&request.data).to_string();
    let rebuilt = rebuild(RebuildRequest {
        payload: request.data.clone(),
        variant: HeaderVariant::Compact32,
        compression_level: 9,
    })?;
    let smf_name = ensure_smf_name(&request.name);
    let tag_name = tag_name(&smf_name);
    Ok(PreparedDocument {
        payload_name: request.name.clone(),
        name: request.name,
        source_kind: SourceKind::Raw,
        payload_kind,
        smf_name,
        tag_name,
        payload: request.data,
        encoded: rebuilt.encoded,
        metadata: rebuilt.metadata,
        compression_level: 9,
    })
}

fn codec_error(error: smf_container::SmfError) -> WorkerError {
    WorkerError::Codec(error.to_string())
}

fn decoded_name(name: &str) -> String {
    name.strip_suffix(".smf")
        .or_else(|| name.strip_suffix(".SMF"))
        .filter(|name| !name.is_empty())
        .unwrap_or("decoded.bin")
        .to_string()
}

fn ensure_smf_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".smf") {
        name.to_string()
    } else if name.is_empty() {
        "payload.bin.smf".to_string()
    } else {
        format!("{name}.smf")
    }
}

fn tag_name(smf_name: &str) -> String {
    let stem = smf_name
        .strip_suffix(".smf")
        .or_else(|| smf_name.strip_suffix(".SMF"))
        .unwrap_or(smf_name);
    format!("{stem}.tag.smf")
}

fn detect_payload_kind(data: &[u8]) -> &'static str {
    if data.starts_with(b"RTON") {
        "RTON data"
    } else if data.starts_with(b"1bsr") || data.starts_with(b"rsb1") {
        "RSB archive"
    } else if data.starts_with(b"pgsr") || data.starts_with(b"rsgp") {
        "RSG package"
    } else if data.starts_with(b"RIFF") {
        "RIFF / WEM audio"
    } else if data.starts_with(b"OggS") {
        "Ogg audio"
    } else if data.starts_with(b"\x89PNG\r\n\x1A\n") {
        "PNG image"
    } else if data.starts_with(b"PK\x03\x04") {
        "ZIP archive"
    } else if looks_like_text(data) {
        "Text"
    } else {
        "Binary data"
    }
}

fn looks_like_text(data: &[u8]) -> bool {
    let sample = &data[..data.len().min(4096)];
    !sample.contains(&0) && std::str::from_utf8(sample).is_ok()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn prepare_smf(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<PrepareRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        prepare(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn rebuild_smf(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<RebuildRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        rebuild(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_and_smf_inputs_share_a_roundtrip() {
        let source = b"RTON sample payload".repeat(4096);
        let raw = prepare(PrepareRequest {
            name: "profile.rton".to_string(),
            data: source.clone(),
        })
        .unwrap();
        assert_eq!(raw.source_kind, SourceKind::Raw);
        assert_eq!(raw.payload_kind, "RTON data");
        assert_eq!(raw.smf_name, "profile.rton.smf");

        let wrapped = prepare(PrepareRequest {
            name: raw.smf_name.clone(),
            data: raw.encoded,
        })
        .unwrap();
        assert_eq!(wrapped.source_kind, SourceKind::Smf);
        assert_eq!(wrapped.payload, source);
        assert_eq!(wrapped.payload_name, "profile.rton");
        assert_eq!(wrapped.tag_name, "profile.rton.tag.smf");
    }

    #[test]
    fn tag_sidecars_are_rejected_explicitly() {
        assert!(matches!(
            prepare(PrepareRequest {
                name: "archive.rsb.tag.smf".to_string(),
                data: b"ABCDEF\r\n".to_vec(),
            }),
            Err(WorkerError::TagSidecar)
        ));
    }
}
