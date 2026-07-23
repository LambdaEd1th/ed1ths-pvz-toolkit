use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use serde::Serialize;
use wasm_bindgen::prelude::*;

use rton_editor_core::{
    DecodedDocument, WorkerBatchItemResponse, WorkerBatchRequest, WorkerBatchResponse,
    WorkerBatchSource, WorkerDocumentSource, WorkerHexSearchRequest, WorkerLocateTextRequest,
    WorkerModeSwitchRequest, WorkerOpenTextRequest, WorkerParseRequest,
    WorkerReleaseDocumentRequest, WorkerReleaseDocumentResponse, WorkerRtonSizeRequest,
    WorkerSurface, WorkerSurfaceSearchSource, WorkerTextSearchRequest, WorkerTextSurfaceRequest,
    WorkerTreeRequest, WorkerTreeResponse, WorkerValueSearchRequest, WorkerValueSearchResponse,
    decode_worker_document_source, encode_batch_export_document, flatten_expanded_value_tree,
    perform_worker_hex_search, perform_worker_locate_text, perform_worker_mode_switch,
    perform_worker_mode_switch_for_document, perform_worker_open_text, perform_worker_parse,
    perform_worker_parse_for_document, perform_worker_rton_size,
    perform_worker_rton_size_for_document, perform_worker_text_search, perform_worker_text_surface,
    search_value_tree,
};

#[derive(Serialize)]
struct WorkerRuntimeInfo {
    backend: &'static str,
    thread_count: usize,
}

const DOCUMENT_CACHE_LIMIT: usize = 32;
const DOCUMENT_CACHE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Default)]
struct WorkerDocumentStore {
    next_id: u64,
    documents: HashMap<u64, WorkerDocumentEntry>,
    order: VecDeque<u64>,
    estimated_bytes: usize,
}

struct WorkerDocumentEntry {
    document: Arc<DecodedDocument>,
    surface: Option<WorkerStoredSurface>,
    estimated_bytes: usize,
}

#[derive(Clone)]
enum WorkerStoredSurface {
    Text(Arc<str>),
    Bytes(Arc<[u8]>),
}

impl WorkerStoredSurface {
    fn from_surface(surface: &WorkerSurface) -> Self {
        match surface {
            WorkerSurface::RtonBytes(bytes) => Self::Bytes(Arc::from(bytes.as_slice())),
            WorkerSurface::Text { text, .. } => Self::Text(Arc::from(text.as_str())),
        }
    }

    fn from_document_source(source: &WorkerDocumentSource) -> Self {
        match source {
            WorkerDocumentSource::RtonBytes(bytes) => Self::Bytes(Arc::from(bytes.as_slice())),
            WorkerDocumentSource::Text { text, .. } => Self::Text(Arc::from(text.as_str())),
        }
    }
}

thread_local! {
    static DOCUMENT_STORE: RefCell<WorkerDocumentStore> = RefCell::new(WorkerDocumentStore {
        next_id: 1,
        documents: HashMap::new(),
        order: VecDeque::new(),
        estimated_bytes: 0,
    });
}

impl WorkerDocumentStore {
    fn reserve_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn insert(&mut self, id: u64, document: DecodedDocument, surface: Option<WorkerStoredSurface>) {
        self.remove(id);
        let estimated_bytes = estimate_document_bytes(&document, surface.as_ref());
        self.documents.insert(
            id,
            WorkerDocumentEntry {
                document: Arc::new(document),
                surface,
                estimated_bytes,
            },
        );
        self.estimated_bytes = self.estimated_bytes.saturating_add(estimated_bytes);
        self.order.push_back(id);
        self.enforce_budget();
    }

    fn enforce_budget(&mut self) {
        while self.order.len() > DOCUMENT_CACHE_LIMIT
            || (self.estimated_bytes > DOCUMENT_CACHE_BYTES && self.order.len() > 1)
        {
            if let Some(expired) = self.order.pop_front()
                && let Some(entry) = self.documents.remove(&expired)
            {
                self.estimated_bytes = self.estimated_bytes.saturating_sub(entry.estimated_bytes);
            }
        }
    }

    fn remove(&mut self, id: u64) {
        if let Some(entry) = self.documents.remove(&id) {
            self.estimated_bytes = self.estimated_bytes.saturating_sub(entry.estimated_bytes);
        }
        self.order.retain(|candidate| *candidate != id);
    }
}

fn estimate_document_bytes(
    document: &DecodedDocument,
    surface: Option<&WorkerStoredSurface>,
) -> usize {
    let surface_bytes = surface.map_or(0, |surface| match surface {
        WorkerStoredSurface::Text(text) => text.len(),
        WorkerStoredSurface::Bytes(bytes) => bytes.len(),
    });
    surface_bytes.saturating_add(document.stats.nodes.saturating_mul(64))
}

fn reserve_document_id() -> u64 {
    DOCUMENT_STORE.with(|store| store.borrow_mut().reserve_id())
}

fn store_document(id: u64, document: DecodedDocument, surface: Option<WorkerStoredSurface>) {
    DOCUMENT_STORE.with(|store| store.borrow_mut().insert(id, document, surface));
}

fn replace_document(
    previous_id: Option<u64>,
    id: u64,
    document: DecodedDocument,
    surface: Option<WorkerStoredSurface>,
) {
    DOCUMENT_STORE.with(|store| {
        let mut store = store.borrow_mut();
        if let Some(previous_id) = previous_id {
            store.remove(previous_id);
        }
        store.insert(id, document, surface);
    });
}

fn update_document_surface(id: u64, surface: WorkerStoredSurface) -> Result<(), JsValue> {
    DOCUMENT_STORE.with(|store| {
        let mut store = store.borrow_mut();
        let (previous_bytes, next_bytes) = {
            let entry = store
                .documents
                .get_mut(&id)
                .ok_or_else(|| JsValue::from_str("Worker document is no longer available"))?;
            let previous_bytes = entry.estimated_bytes;
            entry.surface = Some(surface);
            entry.estimated_bytes =
                estimate_document_bytes(&entry.document, entry.surface.as_ref());
            (previous_bytes, entry.estimated_bytes)
        };
        store.estimated_bytes = store
            .estimated_bytes
            .saturating_sub(previous_bytes)
            .saturating_add(next_bytes);
        store.order.retain(|candidate| *candidate != id);
        store.order.push_back(id);
        store.enforce_budget();
        Ok(())
    })
}

fn with_document<T>(id: u64, operation: impl FnOnce(&DecodedDocument) -> T) -> Result<T, JsValue> {
    DOCUMENT_STORE.with(|store| {
        let store = store.borrow();
        let document = store
            .documents
            .get(&id)
            .ok_or_else(|| JsValue::from_str("Worker document is no longer available"))?;
        Ok(operation(document.document.as_ref()))
    })
}

fn with_text_document<T>(
    id: u64,
    operation: impl FnOnce(&DecodedDocument, &str) -> T,
) -> Result<T, JsValue> {
    DOCUMENT_STORE.with(|store| {
        let store = store.borrow();
        let entry = store
            .documents
            .get(&id)
            .ok_or_else(|| JsValue::from_str("Worker document is no longer available"))?;
        let Some(WorkerStoredSurface::Text(text)) = entry.surface.as_ref() else {
            return Err(JsValue::from_str(
                "Worker text surface is no longer available",
            ));
        };
        Ok(operation(entry.document.as_ref(), text))
    })
}

fn with_surface<T>(
    source: WorkerSurfaceSearchSource,
    operation: impl FnOnce(WorkerStoredSurface) -> Result<T, JsValue>,
) -> Result<T, JsValue> {
    let surface = match source {
        WorkerSurfaceSearchSource::DocumentId(id) => DOCUMENT_STORE.with(|store| {
            store
                .borrow()
                .documents
                .get(&id)
                .and_then(|entry| entry.surface.clone())
                .ok_or_else(|| JsValue::from_str("Worker surface is no longer available"))
        })?,
        WorkerSurfaceSearchSource::Text(text) => WorkerStoredSurface::Text(Arc::from(text)),
        WorkerSurfaceSearchSource::Bytes(bytes) => WorkerStoredSurface::Bytes(Arc::from(bytes)),
    };
    operation(surface)
}

fn to_js_value<T: Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(
            &serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true),
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn rton_worker_runtime_info() -> Result<JsValue, JsValue> {
    let info = WorkerRuntimeInfo {
        backend: "worker",
        thread_count: 1,
    };
    to_js_value(&info)
}

#[wasm_bindgen]
pub fn rton_worker_mode_switch(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerModeSwitchRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let previous_document_id = request.previous_document_id;
    if request.source.is_none()
        && let Some(document_id) = previous_document_id
    {
        let mut response = with_document(document_id, |document| {
            perform_worker_mode_switch_for_document(
                document,
                request.target_mode,
                request.encode_options,
                request.search_query,
            )
        })?
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        response.worker_document_id = Some(document_id);
        let surface = WorkerStoredSurface::from_surface(&response.surface);
        let value = to_js_value(&response)?;
        update_document_surface(document_id, surface)?;
        return Ok(value);
    }
    let outcome = perform_worker_mode_switch(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let mut response = outcome.response;
    let document_id = reserve_document_id();
    response.worker_document_id = Some(document_id);
    let surface = WorkerStoredSurface::from_surface(&response.surface);
    let value = to_js_value(&response)?;
    replace_document(
        previous_document_id,
        document_id,
        outcome.document,
        Some(surface),
    );
    Ok(value)
}

#[wasm_bindgen]
pub fn rton_worker_parse(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerParseRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let previous_document_id = request.previous_document_id;
    if request.source.is_none()
        && let Some(document_id) = previous_document_id
    {
        let mut response = with_document(document_id, |document| {
            perform_worker_parse_for_document(document, request.search_query)
        })?;
        response.worker_document_id = Some(document_id);
        return to_js_value(&response);
    }
    let surface = request
        .source
        .as_ref()
        .map(WorkerStoredSurface::from_document_source);
    let outcome =
        perform_worker_parse(request).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let mut response = outcome.response;
    let document_id = reserve_document_id();
    response.worker_document_id = Some(document_id);
    let value = to_js_value(&response)?;
    replace_document(previous_document_id, document_id, outcome.document, surface);
    Ok(value)
}

#[wasm_bindgen]
pub fn rton_worker_rton_size(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerRtonSizeRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    if request.source.is_none()
        && let Some(document_id) = request.document_id
    {
        let response = with_document(document_id, |document| {
            perform_worker_rton_size_for_document(document, request.encode_options)
        })?
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        return to_js_value(&response);
    }
    let response =
        perform_worker_rton_size(request).map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_text_surface(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerTextSurfaceRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let response = perform_worker_text_surface(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_open_text(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerOpenTextRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let outcome =
        perform_worker_open_text(request).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let mut response = outcome.response;
    let document_id = outcome.document.as_ref().map(|_| reserve_document_id());
    response.worker_document_id = document_id;
    let surface = WorkerStoredSurface::from_surface(&response.surface);
    let value = to_js_value(&response)?;
    if let (Some(document_id), Some(document)) = (document_id, outcome.document) {
        store_document(document_id, document, Some(surface));
    }
    Ok(value)
}

#[wasm_bindgen]
pub fn rton_worker_tree(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerTreeRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let response = with_document(request.document_id, |document| WorkerTreeResponse {
        tree_rows: flatten_expanded_value_tree(
            &document.value,
            &request.expanded_paths,
            usize::MAX,
        ),
    })?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_value_search(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerValueSearchRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let response = with_document(request.document_id, |document| WorkerValueSearchResponse {
        search_result: (!request.query.trim().is_empty())
            .then(|| search_value_tree(&document.value, &request.query, usize::MAX)),
    })?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_locate_text(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerLocateTextRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let response = with_text_document(request.document_id, |document, text| {
        perform_worker_locate_text(document, text, &request)
    })?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_text_search(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerTextSearchRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let source = request.source.clone();
    let response = with_surface(source, |surface| match surface {
        WorkerStoredSurface::Text(text) => Ok(perform_worker_text_search(&text, &request)),
        WorkerStoredSurface::Bytes(_) => {
            Err(JsValue::from_str("Text search requires a text surface"))
        }
    })?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_hex_search(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerHexSearchRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let source = request.source.clone();
    let response = with_surface(source, |surface| match surface {
        WorkerStoredSurface::Bytes(bytes) => Ok(perform_worker_hex_search(&bytes, &request)),
        WorkerStoredSurface::Text(_) => {
            Err(JsValue::from_str("Hex search requires a byte surface"))
        }
    })?;
    to_js_value(&response)
}

#[wasm_bindgen]
pub fn rton_worker_release_document(request: JsValue) -> Result<JsValue, JsValue> {
    let request = serde_wasm_bindgen::from_value::<WorkerReleaseDocumentRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    DOCUMENT_STORE.with(|store| store.borrow_mut().remove(request.document_id));
    to_js_value(&WorkerReleaseDocumentResponse)
}

enum ResolvedBatchSource {
    Document(Arc<DecodedDocument>),
    Bytes {
        bytes: Vec<u8>,
        format: Option<rton_editor_core::TextFormat>,
    },
    Error(String),
}

struct ResolvedBatchJob {
    index: usize,
    source: ResolvedBatchSource,
}

#[wasm_bindgen]
pub fn rton_worker_batch(request: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request = serde_wasm_bindgen::from_value::<WorkerBatchRequest>(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let jobs = request
        .jobs
        .into_iter()
        .map(|job| ResolvedBatchJob {
            index: job.index,
            source: resolve_batch_source(job.source),
        })
        .collect::<Vec<_>>();

    let results = jobs
        .into_iter()
        .map(|job| process_batch_job(job, request.mode, request.encode_options))
        .collect::<Vec<_>>();

    to_js_value(&WorkerBatchResponse { results })
}

fn resolve_batch_source(source: WorkerBatchSource) -> ResolvedBatchSource {
    match source {
        WorkerBatchSource::DocumentId(document_id) => DOCUMENT_STORE.with(|store| {
            store
                .borrow()
                .documents
                .get(&document_id)
                .map(|entry| Arc::clone(&entry.document))
                .map(ResolvedBatchSource::Document)
                .unwrap_or_else(|| {
                    ResolvedBatchSource::Error("Worker document is no longer available".to_string())
                })
        }),
        WorkerBatchSource::Bytes { bytes, format } => ResolvedBatchSource::Bytes { bytes, format },
    }
}

fn process_batch_job(
    job: ResolvedBatchJob,
    mode: rton_editor_core::BatchExportMode,
    encode_options: rton_editor_core::EncodeOptions,
) -> WorkerBatchItemResponse {
    let result = match job.source {
        ResolvedBatchSource::Document(document) => {
            encode_batch_export_document(&document, mode, encode_options)
        }
        ResolvedBatchSource::Bytes { bytes, format } => {
            let source = match format {
                Some(format) => WorkerDocumentSource::Text {
                    text: String::from_utf8_lossy(&bytes).into_owned(),
                    format,
                },
                None => WorkerDocumentSource::RtonBytes(bytes),
            };
            decode_worker_document_source(source)
                .map_err(|error| error.to_string())
                .and_then(|document| encode_batch_export_document(&document, mode, encode_options))
        }
        ResolvedBatchSource::Error(error) => Err(error),
    };
    match result {
        Ok(bytes) => WorkerBatchItemResponse {
            index: job.index,
            bytes,
            error: None,
        },
        Err(error) => WorkerBatchItemResponse {
            index: job.index,
            bytes: Vec::new(),
            error: Some(error),
        },
    }
}
