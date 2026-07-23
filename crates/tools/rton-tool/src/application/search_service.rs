use std::sync::Arc;

use rton_editor_core::TextSearchResult;

use crate::domain::TextBuffer;

pub(crate) struct SearchService;

impl SearchService {
    pub(crate) async fn search_text(
        buffer: Arc<TextBuffer>,
        worker_document_id: Option<u64>,
        query: String,
        case_sensitive: bool,
    ) -> Result<TextSearchResult, String> {
        search_text_backend(buffer, worker_document_id, query, case_sensitive).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn search_text_backend(
    buffer: Arc<TextBuffer>,
    _worker_document_id: Option<u64>,
    query: String,
    case_sensitive: bool,
) -> Result<TextSearchResult, String> {
    Ok(crate::platform::run_cpu_task(move || {
        let text = buffer.materialize();
        rton_editor_core::find_text_search_result(&text, &query, case_sensitive)
    })
    .await)
}

#[cfg(target_arch = "wasm32")]
async fn search_text_backend(
    buffer: Arc<TextBuffer>,
    worker_document_id: Option<u64>,
    query: String,
    case_sensitive: bool,
) -> Result<TextSearchResult, String> {
    use rton_editor_core::{WorkerSurfaceSearchSource, WorkerTextSearchRequest};

    let source = worker_document_id.map_or_else(
        || WorkerSurfaceSearchSource::Text(buffer.materialize()),
        WorkerSurfaceSearchSource::DocumentId,
    );
    crate::platform::run_text_search_worker(WorkerTextSearchRequest {
        source,
        query,
        case_sensitive,
    })
    .await
    .map(|response| response.result)
}
