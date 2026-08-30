use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    BatchExportMode, DecodedDocument, EncodeOptions, HexSearchResult, Result, TextFormat,
    TextPosition, TextSearchResult, TreeRows, ValueSearchResult, ValueStats, decode_rton_bytes,
    encode_rton_bytes, find_hex_search_result, find_text_search_result,
    flatten_expanded_value_tree, locate_value_path_in_text, parse_text, search_value_tree,
    value_to_text,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerEditorMode {
    RtonHex,
    Json,
    Yaml,
    Toml,
}

impl WorkerEditorMode {
    pub fn text_format(self) -> Option<TextFormat> {
        match self {
            Self::RtonHex => None,
            Self::Json => Some(TextFormat::Json),
            Self::Yaml => Some(TextFormat::Yaml),
            Self::Toml => Some(TextFormat::Toml),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkerDocumentSource {
    RtonBytes(#[serde(with = "serde_bytes")] Vec<u8>),
    Text { text: String, format: TextFormat },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkerSurface {
    RtonBytes(#[serde(with = "serde_bytes")] Vec<u8>),
    Text {
        text: String,
        byte_count: usize,
        line_count: usize,
        format: TextFormat,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerModeSwitchRequest {
    pub previous_document_id: Option<u64>,
    pub source: Option<WorkerDocumentSource>,
    pub target_mode: WorkerEditorMode,
    pub search_query: String,
    pub encode_options: EncodeOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerModeSwitchResponse {
    pub worker_document_id: Option<u64>,
    pub stats: ValueStats,
    pub surface: WorkerSurface,
    pub tree_rows: TreeRows,
    pub search_result: Option<ValueSearchResult>,
    pub search_query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerParseRequest {
    pub previous_document_id: Option<u64>,
    pub source: Option<WorkerDocumentSource>,
    pub search_query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerParseResponse {
    pub worker_document_id: Option<u64>,
    pub stats: ValueStats,
    pub tree_rows: TreeRows,
    pub search_result: Option<ValueSearchResult>,
    pub search_query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerRtonSizeRequest {
    pub document_id: Option<u64>,
    pub source: Option<WorkerDocumentSource>,
    pub encode_options: EncodeOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRtonSizeResponse {
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerTextSurfaceRequest {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    pub format: TextFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerTextSurfaceResponse {
    pub surface: WorkerSurface,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerOpenTextRequest {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    pub format: TextFormat,
    pub search_query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerOpenTextResponse {
    pub worker_document_id: Option<u64>,
    pub stats: Option<ValueStats>,
    pub surface: WorkerSurface,
    pub tree_rows: TreeRows,
    pub search_result: Option<ValueSearchResult>,
    pub search_query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerTreeRequest {
    pub document_id: u64,
    pub expanded_paths: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerTreeResponse {
    pub tree_rows: TreeRows,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerValueSearchRequest {
    pub document_id: u64,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerValueSearchResponse {
    pub search_result: Option<ValueSearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLocateTextRequest {
    pub document_id: u64,
    pub path: String,
    pub format: TextFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLocateTextResponse {
    pub position: Option<TextPosition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkerSurfaceSearchSource {
    DocumentId(u64),
    Text(String),
    Bytes(#[serde(with = "serde_bytes")] Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerTextSearchRequest {
    pub source: WorkerSurfaceSearchSource,
    pub query: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerTextSearchResponse {
    pub result: TextSearchResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerHexSearchRequest {
    pub source: WorkerSurfaceSearchSource,
    #[serde(with = "serde_bytes")]
    pub pattern: Vec<u8>,
    pub ascii_insensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHexSearchResponse {
    pub result: HexSearchResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerReleaseDocumentRequest {
    pub document_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerReleaseDocumentResponse;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkerBatchSource {
    DocumentId(u64),
    Bytes {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        format: Option<TextFormat>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerBatchJob {
    pub index: usize,
    pub source: WorkerBatchSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerBatchRequest {
    pub jobs: Vec<WorkerBatchJob>,
    pub mode: BatchExportMode,
    pub encode_options: EncodeOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerBatchItemResponse {
    pub index: usize,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerBatchResponse {
    pub results: Vec<WorkerBatchItemResponse>,
}

#[derive(Debug)]
pub struct WorkerModeSwitchOutcome {
    pub response: WorkerModeSwitchResponse,
    pub document: DecodedDocument,
}

#[derive(Debug)]
pub struct WorkerParseOutcome {
    pub response: WorkerParseResponse,
    pub document: DecodedDocument,
}

#[derive(Debug)]
pub struct WorkerOpenTextOutcome {
    pub response: WorkerOpenTextResponse,
    pub document: Option<DecodedDocument>,
}

pub fn perform_worker_mode_switch(
    request: WorkerModeSwitchRequest,
) -> Result<WorkerModeSwitchOutcome> {
    let source = request.source.ok_or_else(|| {
        crate::CoreError::InvalidWorkerRequest("document source is unavailable".to_string())
    })?;
    let doc = decode_worker_document_source(source)?;
    let response = perform_worker_mode_switch_for_document(
        &doc,
        request.target_mode,
        request.encode_options,
        request.search_query,
    )?;

    Ok(WorkerModeSwitchOutcome {
        response,
        document: doc,
    })
}

pub fn perform_worker_mode_switch_for_document(
    doc: &DecodedDocument,
    target_mode: WorkerEditorMode,
    encode_options: EncodeOptions,
    search_query: String,
) -> Result<WorkerModeSwitchResponse> {
    let (surface, (tree_rows, search_result)) =
        worker_mode_switch_parts(doc, target_mode, encode_options, &search_query)?;
    Ok(WorkerModeSwitchResponse {
        worker_document_id: None,
        stats: doc.stats.clone(),
        surface,
        tree_rows,
        search_result,
        search_query,
    })
}

pub fn perform_worker_parse(request: WorkerParseRequest) -> Result<WorkerParseOutcome> {
    let source = request.source.ok_or_else(|| {
        crate::CoreError::InvalidWorkerRequest("document source is unavailable".to_string())
    })?;
    let doc = decode_worker_document_source(source)?;
    let response = perform_worker_parse_for_document(&doc, request.search_query);

    Ok(WorkerParseOutcome {
        response,
        document: doc,
    })
}

pub fn perform_worker_parse_for_document(
    doc: &DecodedDocument,
    search_query: String,
) -> WorkerParseResponse {
    let (tree_rows, search_result) = worker_document_metadata(doc, &search_query);
    WorkerParseResponse {
        worker_document_id: None,
        stats: doc.stats.clone(),
        tree_rows,
        search_result,
        search_query,
    }
}

pub fn perform_worker_rton_size(request: WorkerRtonSizeRequest) -> Result<WorkerRtonSizeResponse> {
    let source = request.source.ok_or_else(|| {
        crate::CoreError::InvalidWorkerRequest("document source is unavailable".to_string())
    })?;
    let doc = decode_worker_document_source(source)?;
    perform_worker_rton_size_for_document(&doc, request.encode_options)
}

pub fn perform_worker_rton_size_for_document(
    doc: &DecodedDocument,
    encode_options: EncodeOptions,
) -> Result<WorkerRtonSizeResponse> {
    let bytes = encode_rton_bytes(&doc.value, encode_options)?;
    Ok(WorkerRtonSizeResponse {
        byte_len: bytes.len(),
    })
}

pub fn perform_worker_text_search(
    text: &str,
    request: &WorkerTextSearchRequest,
) -> WorkerTextSearchResponse {
    WorkerTextSearchResponse {
        result: find_text_search_result(text, &request.query, request.case_sensitive),
    }
}

pub fn perform_worker_hex_search(
    bytes: &[u8],
    request: &WorkerHexSearchRequest,
) -> WorkerHexSearchResponse {
    WorkerHexSearchResponse {
        result: find_hex_search_result(bytes, &request.pattern, request.ascii_insensitive),
    }
}

pub fn perform_worker_text_surface(
    request: WorkerTextSurfaceRequest,
) -> Result<WorkerTextSurfaceResponse> {
    let text = String::from_utf8_lossy(&request.bytes).to_string();
    let byte_count = text.len();
    let line_count = text_line_count(&text);
    Ok(WorkerTextSurfaceResponse {
        surface: WorkerSurface::Text {
            text,
            byte_count,
            line_count,
            format: request.format,
        },
    })
}

pub fn perform_worker_open_text(request: WorkerOpenTextRequest) -> Result<WorkerOpenTextOutcome> {
    let text = String::from_utf8_lossy(&request.bytes).to_string();
    #[cfg(not(target_arch = "wasm32"))]
    let (line_count, parsed) = rayon::join(
        || text_line_count(&text),
        || parse_text(&text, request.format).ok(),
    );
    #[cfg(target_arch = "wasm32")]
    let (line_count, parsed) = (
        text_line_count(&text),
        parse_text(&text, request.format).ok(),
    );
    let byte_count = text.len();
    let (tree_rows, search_result) = parsed.as_ref().map_or_else(
        || {
            (
                TreeRows {
                    rows: Vec::new(),
                    truncated: false,
                },
                None,
            )
        },
        |doc| worker_document_metadata(doc, &request.search_query),
    );

    Ok(WorkerOpenTextOutcome {
        response: WorkerOpenTextResponse {
            worker_document_id: None,
            stats: parsed.as_ref().map(|doc| doc.stats.clone()),
            surface: WorkerSurface::Text {
                text,
                byte_count,
                line_count,
                format: request.format,
            },
            tree_rows,
            search_result,
            search_query: request.search_query,
        },
        document: parsed,
    })
}

pub fn perform_worker_locate_text(
    document: &DecodedDocument,
    text: &str,
    request: &WorkerLocateTextRequest,
) -> WorkerLocateTextResponse {
    WorkerLocateTextResponse {
        position: locate_value_path_in_text(&document.value, &request.path, text, request.format),
    }
}

pub fn decode_worker_document_source(source: WorkerDocumentSource) -> Result<DecodedDocument> {
    match source {
        WorkerDocumentSource::RtonBytes(bytes) => decode_rton_bytes(&bytes),
        WorkerDocumentSource::Text { text, format } => parse_text(&text, format),
    }
}

fn worker_surface_for_document(
    doc: &DecodedDocument,
    mode: WorkerEditorMode,
    encode_options: EncodeOptions,
) -> Result<WorkerSurface> {
    match mode {
        WorkerEditorMode::RtonHex => Ok(WorkerSurface::RtonBytes(encode_rton_bytes(
            &doc.value,
            encode_options,
        )?)),
        WorkerEditorMode::Json | WorkerEditorMode::Yaml | WorkerEditorMode::Toml => {
            let format = mode.text_format().expect("text worker mode has format");
            let text = value_to_text(&doc.value, format)?;
            let byte_count = text.len();
            let line_count = text_line_count(&text);
            Ok(WorkerSurface::Text {
                text,
                byte_count,
                line_count,
                format,
            })
        }
    }
}

fn worker_mode_switch_parts(
    doc: &DecodedDocument,
    mode: WorkerEditorMode,
    encode_options: EncodeOptions,
    search_query: &str,
) -> Result<(WorkerSurface, (TreeRows, Option<ValueSearchResult>))> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (surface, metadata) = rayon::join(
            || worker_surface_for_document(doc, mode, encode_options),
            || worker_document_metadata(doc, search_query),
        );
        Ok((surface?, metadata))
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok((
            worker_surface_for_document(doc, mode, encode_options)?,
            worker_document_metadata(doc, search_query),
        ))
    }
}

fn worker_document_metadata(
    doc: &DecodedDocument,
    search_query: &str,
) -> (TreeRows, Option<ValueSearchResult>) {
    let tree =
        || flatten_expanded_value_tree(&doc.value, &default_worker_expanded_paths(), usize::MAX);
    let search = || {
        (!search_query.trim().is_empty())
            .then(|| search_value_tree(&doc.value, search_query, usize::MAX))
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::join(tree, search)
    }
    #[cfg(target_arch = "wasm32")]
    {
        (tree(), search())
    }
}

fn default_worker_expanded_paths() -> HashSet<String> {
    HashSet::from(["$".to_string()])
}

fn text_line_count(text: &str) -> usize {
    text.lines().count()
}
