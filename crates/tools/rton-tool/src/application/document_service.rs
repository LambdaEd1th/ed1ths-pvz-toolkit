use std::collections::HashSet;
use std::sync::Arc;

use rton_editor_core::{
    DecodedDocument, EncodeOptions, TextFormat, TextPosition, TreeRows, ValueSearchResult,
    ValueStats,
};

use crate::domain::editor_tab::TabSurface;
use crate::domain::{EditorMode, EditorTabState, TextBuffer};
#[cfg(target_arch = "wasm32")]
use crate::domain::{TextContentState, empty_editor_text};

#[cfg(not(target_arch = "wasm32"))]
use crate::domain::{
    document_for_owned_tab, tab_surface_for_document, value_search_result_for_doc,
    value_tree_rows_for_doc, value_tree_rows_for_doc_with_expansion,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::run_cpu_task;
#[cfg(not(target_arch = "wasm32"))]
use rton_editor_core::locate_value_path_in_text;

#[cfg(target_arch = "wasm32")]
use crate::platform::{
    run_locate_text_worker, run_mode_switch_worker, run_parse_worker, run_tree_worker,
    run_value_search_worker,
};
#[cfg(target_arch = "wasm32")]
use rton_editor_core::{
    WorkerDocumentSource, WorkerEditorMode, WorkerLocateTextRequest, WorkerModeSwitchRequest,
    WorkerModeSwitchResponse, WorkerParseRequest, WorkerParseResponse, WorkerSurface,
    WorkerTreeRequest, WorkerValueSearchRequest,
};

#[derive(Debug)]
pub(crate) struct ModeSwitchPayload {
    pub(crate) worker_document_id: Option<u64>,
    pub(crate) doc: Option<Arc<DecodedDocument>>,
    pub(crate) stats: ValueStats,
    pub(crate) surface: TabSurface,
    pub(crate) tree_rows: Arc<TreeRows>,
    pub(crate) search_result: Option<Arc<ValueSearchResult>>,
    pub(crate) search_query: String,
    pub(crate) was_dirty: bool,
}

#[derive(Debug)]
pub(crate) struct ParsePayload {
    pub(crate) worker_document_id: Option<u64>,
    pub(crate) worker_surface_mode: Option<EditorMode>,
    pub(crate) doc: Option<Arc<DecodedDocument>>,
    pub(crate) stats: ValueStats,
    pub(crate) tree_rows: Arc<TreeRows>,
    pub(crate) search_result: Option<Arc<ValueSearchResult>>,
    pub(crate) search_query: String,
}

pub(crate) struct DocumentService;

impl DocumentService {
    pub(crate) async fn parse(tab: EditorTabState) -> Result<ParsePayload, String> {
        let search_query = tab.search_query.clone();
        if !tab.dirty && (tab.doc.is_some() || tab.worker_document_id.is_some()) {
            let stats = tab
                .stats
                .clone()
                .ok_or_else(|| "Parsed document metadata is unavailable".to_string())?;
            return Ok(ParsePayload {
                worker_document_id: tab.worker_document_id,
                worker_surface_mode: tab.worker_surface_mode,
                doc: tab.doc,
                stats,
                tree_rows: tab.tree_rows,
                search_result: tab.search_result,
                search_query,
            });
        }

        parse_uncached(tab, search_query).await
    }

    pub(crate) async fn switch_mode(
        tab: EditorTabState,
        next_mode: EditorMode,
        encode_options: EncodeOptions,
    ) -> Result<ModeSwitchPayload, String> {
        switch_mode_uncached(tab, next_mode, encode_options).await
    }

    pub(crate) async fn search_values(
        doc: Option<Arc<DecodedDocument>>,
        worker_document_id: Option<u64>,
        query: String,
    ) -> Option<ValueSearchResult> {
        search_values_backend(doc, worker_document_id, query).await
    }

    pub(crate) async fn tree_rows(
        doc: Option<Arc<DecodedDocument>>,
        worker_document_id: Option<u64>,
        expanded_paths: Arc<HashSet<String>>,
    ) -> Option<Arc<TreeRows>> {
        tree_rows_backend(doc, worker_document_id, expanded_paths).await
    }

    pub(crate) async fn locate_text(
        doc: Option<Arc<DecodedDocument>>,
        worker_document_id: Option<u64>,
        path: String,
        text_buffer: Option<Arc<TextBuffer>>,
        editor_text: Arc<str>,
        format: TextFormat,
    ) -> Option<TextPosition> {
        locate_text_backend(
            doc,
            worker_document_id,
            path,
            text_buffer,
            editor_text,
            format,
        )
        .await
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn parse_uncached(tab: EditorTabState, search_query: String) -> Result<ParsePayload, String> {
    let doc = run_cpu_task(move || document_for_owned_tab(tab).map_err(|error| error.to_string()))
        .await?;
    let (tree_rows, search_result) = metadata_for_document(doc.clone(), search_query.clone()).await;
    Ok(ParsePayload {
        worker_document_id: None,
        worker_surface_mode: None,
        stats: doc.stats.clone(),
        doc: Some(doc),
        tree_rows,
        search_result,
        search_query,
    })
}

#[cfg(target_arch = "wasm32")]
async fn parse_uncached(
    tab: EditorTabState,
    _search_query: String,
) -> Result<ParsePayload, String> {
    let mode = tab.mode;
    let request = WorkerParseRequest {
        previous_document_id: tab.worker_document_id,
        source: if tab.worker_document_id.is_some() {
            None
        } else {
            Some(worker_document_source(&tab)?)
        },
        search_query: tab.search_query.clone(),
    };
    let response = match run_parse_worker(request).await {
        Ok(response) => response,
        Err(_) if tab.worker_document_id.is_some() => {
            run_parse_worker(WorkerParseRequest {
                previous_document_id: None,
                source: Some(worker_document_source(&tab)?),
                search_query: tab.search_query,
            })
            .await?
        }
        Err(error) => return Err(error),
    };
    Ok(parse_worker_payload(response, mode))
}

#[cfg(not(target_arch = "wasm32"))]
async fn switch_mode_uncached(
    tab: EditorTabState,
    next_mode: EditorMode,
    encode_options: EncodeOptions,
) -> Result<ModeSwitchPayload, String> {
    let was_dirty = tab.dirty;
    let search_query = tab.search_query.clone();
    let doc = run_cpu_task(move || document_for_owned_tab(tab).map_err(|error| error.to_string()))
        .await?;
    let work_doc = doc.clone();
    let work_query = search_query.clone();
    let (surface, (tree_rows, search_result)) = run_cpu_task(move || {
        rayon::join(
            || {
                tab_surface_for_document(&work_doc, next_mode, encode_options)
                    .map_err(|error| error.to_string())
            },
            || metadata_for_document_sync(&work_doc, &work_query),
        )
    })
    .await;
    Ok(ModeSwitchPayload {
        worker_document_id: None,
        stats: doc.stats.clone(),
        doc: Some(doc),
        surface: surface?,
        tree_rows,
        search_result,
        search_query,
        was_dirty,
    })
}

#[cfg(target_arch = "wasm32")]
async fn switch_mode_uncached(
    tab: EditorTabState,
    next_mode: EditorMode,
    encode_options: EncodeOptions,
) -> Result<ModeSwitchPayload, String> {
    let was_dirty = tab.dirty;
    let request = WorkerModeSwitchRequest {
        previous_document_id: tab.worker_document_id,
        source: if tab.worker_document_id.is_some() {
            None
        } else {
            Some(worker_document_source(&tab)?)
        },
        target_mode: worker_editor_mode(next_mode),
        search_query: tab.search_query.clone(),
        encode_options,
    };
    let response = match run_mode_switch_worker(request).await {
        Ok(response) => response,
        Err(_) if tab.worker_document_id.is_some() => {
            run_mode_switch_worker(WorkerModeSwitchRequest {
                previous_document_id: None,
                source: Some(worker_document_source(&tab)?),
                target_mode: worker_editor_mode(next_mode),
                search_query: tab.search_query,
                encode_options,
            })
            .await?
        }
        Err(error) => return Err(error),
    };
    worker_mode_switch_payload(response, was_dirty)
}

#[cfg(not(target_arch = "wasm32"))]
async fn metadata_for_document(
    doc: Arc<DecodedDocument>,
    query: String,
) -> (Arc<TreeRows>, Option<Arc<ValueSearchResult>>) {
    run_cpu_task(move || metadata_for_document_sync(&doc, &query)).await
}

#[cfg(not(target_arch = "wasm32"))]
fn metadata_for_document_sync(
    doc: &DecodedDocument,
    query: &str,
) -> (Arc<TreeRows>, Option<Arc<ValueSearchResult>>) {
    rayon::join(
        || value_tree_rows_for_doc(doc),
        || value_search_result_for_doc(doc, query),
    )
}

#[cfg(target_arch = "wasm32")]
fn worker_document_source(tab: &EditorTabState) -> Result<WorkerDocumentSource, String> {
    match tab.mode {
        EditorMode::RtonHex => tab
            .byte_doc
            .as_ref()
            .map(|document| WorkerDocumentSource::RtonBytes(document.as_cow().into_owned()))
            .ok_or_else(|| "Missing RTON bytes".to_string()),
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            let format = tab
                .mode
                .text_format()
                .ok_or_else(|| "Missing text format".to_string())?;
            let text = tab
                .text_buffer
                .as_ref()
                .map(|buffer| buffer.materialize())
                .unwrap_or_else(|| tab.editor_text.to_string());
            Ok(WorkerDocumentSource::Text { text, format })
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn worker_editor_mode(mode: EditorMode) -> WorkerEditorMode {
    match mode {
        EditorMode::RtonHex => WorkerEditorMode::RtonHex,
        EditorMode::Json => WorkerEditorMode::Json,
        EditorMode::Yaml => WorkerEditorMode::Yaml,
        EditorMode::Toml => WorkerEditorMode::Toml,
    }
}

#[cfg(target_arch = "wasm32")]
fn worker_mode_switch_payload(
    response: WorkerModeSwitchResponse,
    was_dirty: bool,
) -> Result<ModeSwitchPayload, String> {
    Ok(ModeSwitchPayload {
        worker_document_id: response.worker_document_id,
        doc: None,
        stats: response.stats,
        surface: worker_surface_to_tab_surface(response.surface),
        tree_rows: Arc::new(response.tree_rows),
        search_result: response.search_result.map(Arc::new),
        search_query: response.search_query,
        was_dirty,
    })
}

#[cfg(target_arch = "wasm32")]
fn parse_worker_payload(response: WorkerParseResponse, mode: EditorMode) -> ParsePayload {
    ParsePayload {
        worker_document_id: response.worker_document_id,
        worker_surface_mode: response.worker_document_id.map(|_| mode),
        doc: None,
        stats: response.stats,
        tree_rows: Arc::new(response.tree_rows),
        search_result: response.search_result.map(Arc::new),
        search_query: response.search_query,
    }
}

#[cfg(target_arch = "wasm32")]
fn worker_surface_to_tab_surface(surface: WorkerSurface) -> TabSurface {
    match surface {
        WorkerSurface::RtonBytes(bytes) => TabSurface {
            byte_doc: Some(crate::domain::ByteDocument::from_vec(bytes)),
            editor_text: empty_editor_text(),
            text_buffer: None,
            text_state: TextContentState::None,
        },
        WorkerSurface::Text {
            text,
            byte_count,
            line_count,
            format,
        } => TabSurface {
            byte_doc: None,
            editor_text: empty_editor_text(),
            text_buffer: Some(Arc::new(TextBuffer::new(text))),
            text_state: TextContentState::Text {
                byte_count,
                line_count,
                format,
            },
        },
    }
}

async fn search_values_backend(
    doc: Option<Arc<DecodedDocument>>,
    worker_document_id: Option<u64>,
    query: String,
) -> Option<ValueSearchResult> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = doc;
        run_value_search_worker(WorkerValueSearchRequest {
            document_id: worker_document_id?,
            query,
        })
        .await
        .ok()
        .and_then(|response| response.search_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = worker_document_id;
        let doc = doc?;
        run_cpu_task(move || value_search_result_for_doc(&doc, &query).map(Arc::unwrap_or_clone))
            .await
    }
}

async fn tree_rows_backend(
    doc: Option<Arc<DecodedDocument>>,
    worker_document_id: Option<u64>,
    expanded_paths: Arc<HashSet<String>>,
) -> Option<Arc<TreeRows>> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = doc;
        run_tree_worker(WorkerTreeRequest {
            document_id: worker_document_id?,
            expanded_paths: expanded_paths.as_ref().clone(),
        })
        .await
        .ok()
        .map(|response| Arc::new(response.tree_rows))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = worker_document_id;
        let doc = doc?;
        Some(
            run_cpu_task(move || value_tree_rows_for_doc_with_expansion(&doc, &expanded_paths))
                .await,
        )
    }
}

async fn locate_text_backend(
    doc: Option<Arc<DecodedDocument>>,
    worker_document_id: Option<u64>,
    path: String,
    text_buffer: Option<Arc<TextBuffer>>,
    editor_text: Arc<str>,
    format: TextFormat,
) -> Option<TextPosition> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (doc, text_buffer, editor_text);
        run_locate_text_worker(WorkerLocateTextRequest {
            document_id: worker_document_id?,
            path,
            format,
        })
        .await
        .ok()
        .and_then(|response| response.position)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = worker_document_id;
        let doc = doc?;
        run_cpu_task(move || {
            let text = text_buffer
                .map(|buffer| buffer.materialize())
                .unwrap_or_else(|| editor_text.to_string());
            locate_value_path_in_text(&doc.value, &path, &text, format)
        })
        .await
    }
}
