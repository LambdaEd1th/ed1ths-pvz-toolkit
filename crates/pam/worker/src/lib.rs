use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use pam_viewer_core::{
    ExportKind, ExportRequest, LoadedPamPayload, PamDocument, PamDocumentPayload, WorkerRequest,
    WorkerResponse,
};
use pam_viewer_formats::{InputFile, TextFormat};

static DOCUMENTS: OnceLock<Mutex<HashMap<u64, Arc<PamDocument>>>> = OnceLock::new();
static EXPORTS: OnceLock<Mutex<HashMap<u64, Arc<AtomicBool>>>> = OnceLock::new();

fn documents() -> &'static Mutex<HashMap<u64, Arc<PamDocument>>> {
    DOCUMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_documents<T>(operation: impl FnOnce(&mut HashMap<u64, Arc<PamDocument>>) -> T) -> T {
    let mut documents = documents()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    operation(&mut documents)
}

fn with_exports<T>(operation: impl FnOnce(&mut HashMap<u64, Arc<AtomicBool>>) -> T) -> T {
    let mut exports = EXPORTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    operation(&mut exports)
}

pub async fn perform_worker_request(request: WorkerRequest) -> WorkerResponse {
    match request {
        WorkerRequest::Batch { requests } => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                use rayon::prelude::*;

                let responses = requests
                    .into_par_iter()
                    .map(|request| {
                        if matches!(request, WorkerRequest::Batch { .. }) {
                            WorkerResponse::Error {
                                message: "Nested Worker batches are not supported".to_string(),
                            }
                        } else {
                            pollster::block_on(perform_single_request(request))
                        }
                    })
                    .collect();
                WorkerResponse::Batch { responses }
            }
            #[cfg(target_arch = "wasm32")]
            {
                let mut responses = Vec::with_capacity(requests.len());
                for request in requests {
                    if matches!(request, WorkerRequest::Batch { .. }) {
                        responses.push(WorkerResponse::Error {
                            message: "Nested Worker batches are not supported".to_string(),
                        });
                    } else {
                        responses.push(perform_single_request(request).await);
                    }
                }
                WorkerResponse::Batch { responses }
            }
        }
        request => perform_single_request(request).await,
    }
}

async fn perform_single_request(request: WorkerRequest) -> WorkerResponse {
    let result = match request {
        WorkerRequest::Ping => return WorkerResponse::Pong,
        WorkerRequest::Load { document_id, files } => {
            let files = files
                .into_iter()
                .map(|file| InputFile::new(file.path, Arc::<[u8]>::from(file.bytes)))
                .collect::<Vec<_>>();
            pam_viewer_formats::load_pam_document(&files)
                .map_err(|error| error.to_string())
                .map(|loaded| {
                    let document = Arc::new(loaded.document);
                    let payload = LoadedPamPayload {
                        document: PamDocumentPayload::from(document.as_ref()),
                        loaded_images: loaded.loaded_images,
                        missing_images: loaded.missing_images,
                    };
                    with_documents(|documents| {
                        documents.insert(document_id, document);
                    });
                    WorkerResponse::Loaded {
                        loaded: Box::new(payload),
                    }
                })
        }
        WorkerRequest::RegisterDocument {
            document_id,
            document,
        } => document
            .into_document()
            .map_err(|error| error.to_string())
            .map(|document| {
                with_documents(|documents| {
                    documents.insert(document_id, Arc::new(document));
                });
                WorkerResponse::Registered
            }),
        WorkerRequest::Export(request) => {
            let operation_id = request.operation_id;
            let cancelled = Arc::new(AtomicBool::new(false));
            with_exports(|exports| {
                exports.insert(operation_id, Arc::clone(&cancelled));
            });
            let result = export_document(request, &cancelled)
                .await
                .map(|bytes| WorkerResponse::Exported { bytes });
            with_exports(|exports| {
                if exports
                    .get(&operation_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &cancelled))
                {
                    exports.remove(&operation_id);
                }
            });
            result
        }
        WorkerRequest::CancelExport { operation_id, .. } => {
            with_exports(|exports| {
                if let Some(cancelled) = exports.get(&operation_id) {
                    cancelled.store(true, Ordering::Relaxed);
                }
            });
            Ok(WorkerResponse::Cancelled)
        }
        WorkerRequest::ReleaseDocument { document_id } => {
            with_documents(|documents| {
                documents.remove(&document_id);
            });
            Ok(WorkerResponse::Released)
        }
        WorkerRequest::Batch { .. } => unreachable!("batches are handled by the outer dispatcher"),
    };

    result.unwrap_or_else(|message| WorkerResponse::Error { message })
}

async fn export_document(
    request: ExportRequest,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let document = with_documents(|documents| documents.get(&request.document_id).cloned())
        .ok_or_else(|| format!("Worker document {} is not registered", request.document_id))?;

    ensure_not_cancelled(cancelled)?;
    match request.kind {
        ExportKind::Json => pam_viewer_core::encode_json(&document.pam)
            .map(String::into_bytes)
            .map_err(|error| error.to_string()),
        ExportKind::Yaml => pam_viewer_formats::encode_text(&document.pam, TextFormat::Yaml)
            .map(String::into_bytes)
            .map_err(|error| error.to_string()),
        ExportKind::Toml => pam_viewer_formats::encode_text(&document.pam, TextFormat::Toml)
            .map(String::into_bytes)
            .map_err(|error| error.to_string()),
        ExportKind::Pam => {
            pam_viewer_core::encode_pam_bytes(&document.pam).map_err(|error| error.to_string())
        }
        ExportKind::Fla => {
            pam_viewer_formats::export_fla_with_cancel(&document, 1200, Some(cancelled))
                .map_err(|error| error.to_string())
        }
        ExportKind::Png | ExportKind::Apng | ExportKind::Webp => {
            export_frames(document, &request, cancelled).await
        }
    }
}

async fn export_frames(
    document: Arc<PamDocument>,
    request: &ExportRequest,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let frames = if request.kind == ExportKind::Png {
        vec![request.current_frame]
    } else if request.frame_range[0] <= request.frame_range[1] {
        (request.frame_range[0]..=request.frame_range[1]).collect()
    } else {
        Vec::new()
    };
    if frames.is_empty() {
        return Err("Animation export requires at least one frame".to_string());
    }
    let width = request.size[0].max(1);
    let height = request.size[1].max(1);
    let rendered = pam_viewer_renderer::render_offscreen_frames_with_cancel(
        document,
        request.sprite,
        &frames,
        &request.image_filter,
        &request.sprite_filter,
        width,
        height,
        Some(cancelled),
    )
    .await
    .map_err(|error| error.to_string())?;

    match request.kind {
        ExportKind::Png => pam_viewer_formats::encode_png(&rendered[0], width, height)
            .map_err(|error| error.to_string()),
        ExportKind::Apng => pam_viewer_formats::encode_apng_with_cancel(
            &rendered,
            width,
            height,
            request.fps,
            Some(cancelled),
        )
        .map_err(|error| error.to_string()),
        ExportKind::Webp => pam_viewer_formats::encode_animated_webp_with_cancel(
            &rendered,
            width,
            height,
            request.fps,
            Some(cancelled),
        )
        .map_err(|error| error.to_string()),
        _ => unreachable!(),
    }
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        Err("export was cancelled".to_string())
    } else {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use serde::Serialize;
    use wasm_bindgen::prelude::*;

    use super::*;

    #[wasm_bindgen]
    pub async fn perform(request: JsValue) -> Result<JsValue, JsValue> {
        console_error_panic_hook::set_once();
        let request = serde_wasm_bindgen::from_value(request)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let response = perform_worker_request(request).await;
        let serializer =
            serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
        response
            .serialize(&serializer)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use pam_viewer_core::{ExportRequest, PamInfo, SpriteKey};

    use super::*;

    fn document_payload() -> PamDocumentPayload {
        PamDocumentPayload {
            source_name: "worker-test.pam".to_string(),
            pam: PamInfo {
                version: 6,
                frame_rate: 30,
                position: [0.0, 0.0],
                size: [64.0, 64.0],
                image: Vec::new(),
                sprite: Vec::new(),
                main_sprite: None,
            },
            images: Vec::new(),
            compiled: Default::default(),
            content_bounds: None,
        }
    }

    fn export_request(document_id: u64, kind: ExportKind) -> ExportRequest {
        ExportRequest {
            document_id,
            operation_id: document_id,
            kind,
            sprite: SpriteKey::Main,
            current_frame: 0,
            frame_range: [0, 0],
            image_filter: Vec::new(),
            sprite_filter: Vec::new(),
            size: [64, 64],
            fps: 30,
        }
    }

    #[test]
    fn registered_documents_are_exported_and_released() {
        let document_id = 0xA11CE;
        let registered =
            pollster::block_on(perform_worker_request(WorkerRequest::RegisterDocument {
                document_id,
                document: document_payload(),
            }));
        assert!(matches!(registered, WorkerResponse::Registered));

        let exported = pollster::block_on(perform_worker_request(WorkerRequest::Export(
            export_request(document_id, ExportKind::Json),
        )));
        let WorkerResponse::Exported { bytes } = exported else {
            panic!("unexpected export response: {exported:?}");
        };
        let decoded: PamInfo = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, document_payload().pam);

        let released = pollster::block_on(perform_worker_request(WorkerRequest::ReleaseDocument {
            document_id,
        }));
        assert!(matches!(released, WorkerResponse::Released));
        let missing = pollster::block_on(perform_worker_request(WorkerRequest::Export(
            export_request(document_id, ExportKind::Json),
        )));
        assert!(matches!(missing, WorkerResponse::Error { .. }));
    }

    #[test]
    fn cancellation_request_sets_the_active_export_token() {
        let operation_id = 0xCA11CE;
        let cancelled = Arc::new(AtomicBool::new(false));
        with_exports(|exports| {
            exports.insert(operation_id, Arc::clone(&cancelled));
        });
        let response = pollster::block_on(perform_worker_request(WorkerRequest::CancelExport {
            document_id: 1,
            operation_id,
        }));
        assert!(matches!(response, WorkerResponse::Cancelled));
        assert!(cancelled.load(Ordering::Relaxed));
        with_exports(|exports| {
            exports.remove(&operation_id);
        });
    }
}
