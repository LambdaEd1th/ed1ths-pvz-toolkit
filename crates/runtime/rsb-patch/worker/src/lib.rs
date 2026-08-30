use std::io::Cursor;

use rsb_patch::{ArchiveDecodeOptions, ArchiveEncodeOptions, RsbPatch};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchMode {
    #[default]
    Stored,
    Raw,
}

impl PatchMode {
    fn encode_options(self) -> ArchiveEncodeOptions {
        match self {
            Self::Stored => ArchiveEncodeOptions::stored(),
            Self::Raw => ArchiveEncodeOptions::raw(),
        }
    }

    fn decode_options(self) -> ArchiveDecodeOptions {
        match self {
            Self::Stored => ArchiveDecodeOptions::stored(),
            Self::Raw => ArchiveDecodeOptions::raw(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchRecordSummary {
    pub name: String,
    pub changed: bool,
    pub delta_size: u64,
    pub before_md5: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchSummary {
    pub name: String,
    pub container_size: u64,
    pub target_archive_size: u64,
    pub information_changed: bool,
    pub information_delta_size: u64,
    pub packet_count: u64,
    pub changed_packet_count: u64,
    pub total_delta_size: u64,
    pub records: Vec<PatchRecordSummary>,
}

impl PatchSummary {
    fn from_patch(name: String, container_size: usize, patch: &RsbPatch) -> Self {
        let records = patch
            .packets
            .iter()
            .map(|record| PatchRecordSummary {
                name: record.name.clone(),
                changed: record.patch.is_some(),
                delta_size: record.patch.as_ref().map_or(0, |delta| delta.len() as u64),
                before_md5: hash_hex(&record.before_hash),
            })
            .collect::<Vec<_>>();
        let changed_packet_count = records.iter().filter(|record| record.changed).count() as u64;
        let information_delta_size = patch
            .information_patch
            .as_ref()
            .map_or(0, |delta| delta.len() as u64);
        let total_delta_size = records
            .iter()
            .map(|record| record.delta_size)
            .sum::<u64>()
            .saturating_add(information_delta_size);
        Self {
            name,
            container_size: container_size as u64,
            target_archive_size: u64::from(patch.all_after_size),
            information_changed: patch.information_patch.is_some(),
            information_delta_size,
            packet_count: records.len() as u64,
            changed_packet_count,
            total_delta_size,
            records,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectRequest {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectedPatch {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub summary: PatchSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRequest {
    pub before_name: String,
    #[serde(with = "serde_bytes")]
    pub before: Vec<u8>,
    pub after_name: String,
    #[serde(with = "serde_bytes")]
    pub after: Vec<u8>,
    pub mode: PatchMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatedPatch {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub summary: PatchSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyRequest {
    pub before_name: String,
    #[serde(with = "serde_bytes")]
    pub before: Vec<u8>,
    pub patch_name: String,
    #[serde(with = "serde_bytes")]
    pub patch: Vec<u8>,
    pub mode: PatchMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppliedArchive {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("RSBP 处理失败：{0}")]
    Patch(String),
}

pub type Result<T> = std::result::Result<T, WorkerError>;

pub fn inspect(request: InspectRequest) -> Result<InspectedPatch> {
    let patch = RsbPatch::read(Cursor::new(&request.data)).map_err(patch_error)?;
    let summary =
        PatchSummary::from_patch(ensure_patch_name(&request.name), request.data.len(), &patch);
    Ok(InspectedPatch {
        data: request.data,
        summary,
    })
}

pub fn create(request: CreateRequest) -> Result<CreatedPatch> {
    let patch = RsbPatch::create(
        &request.before,
        &request.after,
        request.mode.encode_options(),
    )
    .map_err(patch_error)?;
    let data = patch.to_bytes().map_err(patch_error)?;
    let name = patch_name_from_after(&request.after_name);
    let summary = PatchSummary::from_patch(name.clone(), data.len(), &patch);
    Ok(CreatedPatch {
        name,
        data,
        summary,
    })
}

pub fn apply(request: ApplyRequest) -> Result<AppliedArchive> {
    let patch = RsbPatch::read(Cursor::new(&request.patch)).map_err(patch_error)?;
    let data = patch
        .apply_to_archive(&request.before, request.mode.decode_options())
        .map_err(patch_error)?;
    Ok(AppliedArchive {
        name: archive_name_from_patch(&request.patch_name, &request.before_name),
        data,
    })
}

fn patch_error(error: rsb_patch::PatchError) -> WorkerError {
    WorkerError::Patch(error.to_string())
}

fn ensure_patch_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".rsbpatch") {
        name.to_string()
    } else if name.is_empty() {
        "patch.rsbpatch".to_string()
    } else {
        format!("{name}.rsbpatch")
    }
}

fn patch_name_from_after(name: &str) -> String {
    let stem = strip_extension(name, &["rsb", "obb"]);
    format!("{}.rsbpatch", non_empty(stem, "patch"))
}

fn archive_name_from_patch(patch_name: &str, before_name: &str) -> String {
    let patch_stem = strip_extension(patch_name, &["rsbpatch"]);
    if !patch_stem.is_empty() {
        return format!("{patch_stem}.rsb");
    }
    let before_stem = strip_extension(before_name, &["rsb", "obb"]);
    format!("{}.patched.rsb", non_empty(before_stem, "archive"))
}

fn strip_extension<'a>(name: &'a str, extensions: &[&str]) -> &'a str {
    extensions
        .iter()
        .find_map(|extension| {
            name.get(..name.len().saturating_sub(extension.len() + 1))
                .filter(|_| {
                    name.to_ascii_lowercase()
                        .ends_with(&format!(".{extension}"))
                })
        })
        .unwrap_or(name)
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}

fn hash_hex(hash: &[u8; 16]) -> String {
    let mut result = String::with_capacity(32);
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

#[cfg(target_arch = "wasm32")]
fn wasm_request<I, O>(
    request: wasm_bindgen::JsValue,
    process: impl FnOnce(I) -> Result<O>,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>
where
    I: for<'de> Deserialize<'de>,
    O: Serialize,
{
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<I>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        process(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn inspect_rsb_patch(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_request(request, inspect)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn create_rsb_patch(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_request(request, create)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn apply_rsb_patch(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_request(request, apply)
}

#[cfg(test)]
mod tests {
    use rsb_patch::{PacketPatch, RSB_PATCH_MAGIC_BYTES, md5_hash};

    use super::*;

    #[test]
    fn inspects_semantic_rsbp_records() {
        let patch = RsbPatch {
            all_after_size: 4096,
            before_hash: md5_hash(b"information"),
            information_patch: Some(vec![1, 2, 3]),
            packets: vec![
                PacketPatch {
                    name: "Changed".into(),
                    before_hash: md5_hash(b"before"),
                    patch: Some(vec![4, 5]),
                },
                PacketPatch {
                    name: "Stable".into(),
                    before_hash: md5_hash(b"stable"),
                    patch: None,
                },
            ],
        };
        let data = patch.to_bytes().unwrap();
        assert_eq!(&data[..4], &RSB_PATCH_MAGIC_BYTES);
        let inspected = inspect(InspectRequest {
            name: "update.rsbpatch".into(),
            data,
        })
        .unwrap();
        let summary = inspected.summary;
        assert_eq!(summary.packet_count, 2);
        assert_eq!(summary.changed_packet_count, 1);
        assert_eq!(summary.total_delta_size, 5);
        assert!(summary.information_changed);
    }

    #[test]
    fn derives_stable_output_names() {
        assert_eq!(patch_name_from_after("main.rsb"), "main.rsbpatch");
        assert_eq!(patch_name_from_after("main.obb"), "main.rsbpatch");
        assert_eq!(
            archive_name_from_patch("12.7.rsbpatch", "old.rsb"),
            "12.7.rsb"
        );
    }
}
