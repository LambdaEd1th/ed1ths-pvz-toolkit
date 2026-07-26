#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
#[cfg(target_arch = "wasm32")]
use rton_editor_core::{
    TextFormat, WorkerBatchRequest, WorkerBatchResponse, WorkerHexSearchRequest,
    WorkerHexSearchResponse, WorkerLocateTextRequest, WorkerLocateTextResponse,
    WorkerModeSwitchRequest, WorkerModeSwitchResponse, WorkerOpenTextRequest,
    WorkerOpenTextResponse, WorkerParseRequest, WorkerParseResponse, WorkerReleaseDocumentRequest,
    WorkerReleaseDocumentResponse, WorkerRtonSizeRequest, WorkerRtonSizeResponse,
    WorkerTextSearchRequest, WorkerTextSearchResponse, WorkerTreeRequest, WorkerTreeResponse,
    WorkerValueSearchRequest, WorkerValueSearchResponse,
};
#[cfg(target_arch = "wasm32")]
use serde::{Serialize, de::DeserializeOwned};
#[cfg(target_arch = "wasm32")]
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

#[cfg(target_arch = "wasm32")]
const RTON_WORKER_URL: &str = "assets/rton/worker/rton-worker.js";

#[cfg(target_arch = "wasm32")]
struct PendingWorkerRequest {
    resolve: Function,
    reject: Function,
}

#[cfg(target_arch = "wasm32")]
struct WorkerClient {
    worker: Worker,
    next_id: u64,
    pending: Rc<RefCell<HashMap<u64, PendingWorkerRequest>>>,
    failed: Rc<Cell<bool>>,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
    _onerror: Closure<dyn FnMut(ErrorEvent)>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WORKER_CLIENT: RefCell<Option<WorkerClient>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
impl WorkerClient {
    fn new() -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker =
            Worker::new_with_options(RTON_WORKER_URL, &options).map_err(js_error_string)?;
        let pending = Rc::new(RefCell::new(HashMap::<u64, PendingWorkerRequest>::new()));
        let failed = Rc::new(Cell::new(false));

        let message_pending = Rc::clone(&pending);
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let response = event.data();
            record_worker_runtime(&response);
            let Some(id) = worker_message_id(&response) else {
                return;
            };
            let request = message_pending.borrow_mut().remove(&id);
            if let Some(request) = request {
                let _ = request.resolve.call1(&JsValue::UNDEFINED, &response);
            }
        });
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let error_pending = Rc::clone(&pending);
        let error_failed = Rc::clone(&failed);
        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |event: ErrorEvent| {
            error_failed.set(true);
            let message = if event.message().is_empty() {
                "Worker error".to_string()
            } else {
                event.message()
            };
            log::error!(target: "rton_editor::worker", "{message}");
            let error = JsValue::from_str(&message);
            for (_, request) in error_pending.borrow_mut().drain() {
                let _ = request.reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        log::info!(target: "rton_editor::worker", "Initialized (web)");

        Ok(Self {
            worker,
            next_id: 1,
            pending,
            failed,
            _onmessage: onmessage,
            _onerror: onerror,
        })
    }

    fn request(&mut self, message: JsValue) -> Result<Promise, String> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        Reflect::set(
            &message,
            &JsValue::from_str("id"),
            &JsValue::from_f64(id as f64),
        )
        .map_err(js_error_string)?;

        let worker = self.worker.clone();
        let pending = Rc::clone(&self.pending);
        let failed = Rc::clone(&self.failed);
        let transfer = worker_message_transfer_list(&message);
        Ok(Promise::new(
            &mut move |resolve: Function, reject: Function| {
                pending.borrow_mut().insert(
                    id,
                    PendingWorkerRequest {
                        resolve,
                        reject: reject.clone(),
                    },
                );

                let posted = transfer.as_ref().map_or_else(
                    || worker.post_message(&message),
                    |transfer| worker.post_message_with_transfer(&message, transfer.as_ref()),
                );
                if let Err(error) = posted {
                    failed.set(true);
                    pending.borrow_mut().remove(&id);
                    let _ = reject.call1(&JsValue::UNDEFINED, &error);
                }
            },
        ))
    }
}

#[cfg(target_arch = "wasm32")]
fn record_worker_runtime(response: &JsValue) {
    let backend = Reflect::get(response, &JsValue::from_str("backend"))
        .ok()
        .and_then(|value| value.as_string());
    let thread_count = Reflect::get(response, &JsValue::from_str("threadCount"))
        .ok()
        .and_then(|value| value.as_f64());
    let fallback_reason = Reflect::get(response, &JsValue::from_str("fallbackReason"))
        .ok()
        .and_then(|value| value.as_string());
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    if let Some(backend) = backend {
        let _ = root.set_attribute("data-rton-worker-backend", &backend);
    }
    if let Some(thread_count) = thread_count {
        let _ = root.set_attribute(
            "data-rton-worker-threads",
            &(thread_count as usize).to_string(),
        );
    }
    if let Some(fallback_reason) = fallback_reason {
        let _ = root.set_attribute("data-rton-worker-fallback", &fallback_reason);
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn run_mode_switch_worker(
    request: WorkerModeSwitchRequest,
) -> Result<WorkerModeSwitchResponse, String> {
    run_worker_request("mode-switch", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_parse_worker(request: WorkerParseRequest) -> Result<WorkerParseResponse, String> {
    run_worker_request("parse", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_rton_size_worker(
    request: WorkerRtonSizeRequest,
) -> Result<WorkerRtonSizeResponse, String> {
    run_worker_request("rton-size", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_open_text_worker(
    request: WorkerOpenTextRequest,
) -> Result<WorkerOpenTextResponse, String> {
    run_worker_request("open-text", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_open_text_file_worker(
    file: web_sys::File,
    format: TextFormat,
    search_query: String,
) -> Result<WorkerOpenTextResponse, String> {
    let message = open_text_file_worker_message(file, format, search_query)?;
    parse_worker_response(await_worker_value(message).await?)
}

#[cfg(target_arch = "wasm32")]
pub async fn run_tree_worker(request: WorkerTreeRequest) -> Result<WorkerTreeResponse, String> {
    run_worker_request("tree", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_value_search_worker(
    request: WorkerValueSearchRequest,
) -> Result<WorkerValueSearchResponse, String> {
    run_worker_request("value-search", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_locate_text_worker(
    request: WorkerLocateTextRequest,
) -> Result<WorkerLocateTextResponse, String> {
    run_worker_request("locate-text", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_text_search_worker(
    request: WorkerTextSearchRequest,
) -> Result<WorkerTextSearchResponse, String> {
    run_worker_request("text-search", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn run_hex_search_worker(
    request: WorkerHexSearchRequest,
) -> Result<WorkerHexSearchResponse, String> {
    run_worker_request("hex-search", &request).await
}

#[cfg(target_arch = "wasm32")]
pub async fn release_worker_document(document_id: u64) -> Result<(), String> {
    run_worker_request::<_, WorkerReleaseDocumentResponse>(
        "release-document",
        &WorkerReleaseDocumentRequest { document_id },
    )
    .await
    .map(|_| ())
}

#[cfg(target_arch = "wasm32")]
pub async fn run_batch_worker(request: WorkerBatchRequest) -> Result<WorkerBatchResponse, String> {
    run_worker_request("batch", &request).await
}

#[cfg(target_arch = "wasm32")]
async fn run_worker_request<Request, Response>(
    kind: &str,
    request: &Request,
) -> Result<Response, String>
where
    Request: Serialize + ?Sized,
    Response: DeserializeOwned,
{
    let request = serde_wasm_bindgen::to_value(request).map_err(|error| error.to_string())?;
    let message = worker_message(kind, &request)?;
    parse_worker_response(await_worker_value(message).await?)
}

#[cfg(target_arch = "wasm32")]
fn worker_message(kind: &str, request: &JsValue) -> Result<JsValue, String> {
    let message = js_sys::Object::new();
    Reflect::set(
        &message,
        &JsValue::from_str("kind"),
        &JsValue::from_str(kind),
    )
    .map_err(js_error_string)?;
    Reflect::set(&message, &JsValue::from_str("request"), request).map_err(js_error_string)?;
    Ok(message.into())
}

#[cfg(target_arch = "wasm32")]
fn open_text_file_worker_message(
    file: web_sys::File,
    format: TextFormat,
    search_query: String,
) -> Result<JsValue, String> {
    let message = js_sys::Object::new();
    Reflect::set(
        &message,
        &JsValue::from_str("kind"),
        &JsValue::from_str("open-text-file"),
    )
    .map_err(js_error_string)?;
    Reflect::set(&message, &JsValue::from_str("file"), file.as_ref()).map_err(js_error_string)?;
    Reflect::set(
        &message,
        &JsValue::from_str("format"),
        &JsValue::from_str(text_format_worker_label(format)),
    )
    .map_err(js_error_string)?;
    Reflect::set(
        &message,
        &JsValue::from_str("search_query"),
        &JsValue::from_str(&search_query),
    )
    .map_err(js_error_string)?;
    Ok(message.into())
}

#[cfg(target_arch = "wasm32")]
fn text_format_worker_label(format: TextFormat) -> &'static str {
    match format {
        TextFormat::Json => "Json",
        TextFormat::Yaml => "Yaml",
        TextFormat::Toml => "Toml",
    }
}

#[cfg(target_arch = "wasm32")]
async fn await_worker_value(message: JsValue) -> Result<JsValue, String> {
    let promise = WORKER_CLIENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        let recreate = slot.as_ref().is_none_or(|client| client.failed.get());
        if recreate {
            if let Some(client) = slot.take() {
                client.worker.terminate();
            }
            *slot = Some(WorkerClient::new()?);
        }
        slot.as_mut()
            .expect("worker client was initialized")
            .request(message)
    })?;
    JsFuture::from(promise).await.map_err(js_error_string)
}

#[cfg(target_arch = "wasm32")]
fn worker_message_id(value: &JsValue) -> Option<u64> {
    Reflect::get(value, &JsValue::from_str("id"))
        .ok()?
        .as_f64()
        .map(|value| value as u64)
}

#[cfg(target_arch = "wasm32")]
fn worker_message_transfer_list(message: &JsValue) -> Option<Array> {
    let request = Reflect::get(message, &JsValue::from_str("request")).ok()?;
    let transfer = Array::new();
    let direct_bytes = Reflect::get(&request, &JsValue::from_str("bytes")).ok();
    let source_bytes = Reflect::get(&request, &JsValue::from_str("source"))
        .ok()
        .and_then(|source| {
            Reflect::get(&source, &JsValue::from_str("RtonBytes"))
                .ok()
                .filter(|value| !value.is_undefined())
                .or_else(|| Reflect::get(&source, &JsValue::from_str("Bytes")).ok())
        });
    if let Some(bytes) = direct_bytes
        .filter(|value| value.is_instance_of::<Uint8Array>())
        .or_else(|| source_bytes.filter(|value| value.is_instance_of::<Uint8Array>()))
    {
        push_transfer_bytes(&transfer, &bytes);
    }
    if let Ok(pattern) = Reflect::get(&request, &JsValue::from_str("pattern"))
        && pattern.is_instance_of::<Uint8Array>()
    {
        push_transfer_bytes(&transfer, &pattern);
    }

    if let Ok(jobs) = Reflect::get(&request, &JsValue::from_str("jobs"))
        && Array::is_array(&jobs)
    {
        let jobs = Array::from(&jobs);
        for index in 0..jobs.length() {
            let job = jobs.get(index);
            let Ok(source) = Reflect::get(&job, &JsValue::from_str("source")) else {
                continue;
            };
            let Ok(bytes_source) = Reflect::get(&source, &JsValue::from_str("Bytes")) else {
                continue;
            };
            let Ok(bytes) = Reflect::get(&bytes_source, &JsValue::from_str("bytes")) else {
                continue;
            };
            if bytes.is_instance_of::<Uint8Array>() {
                push_transfer_bytes(&transfer, &bytes);
            }
        }
    }

    (transfer.length() > 0).then_some(transfer)
}

#[cfg(target_arch = "wasm32")]
fn push_transfer_bytes(transfer: &Array, bytes: &JsValue) {
    transfer.push(Uint8Array::new(bytes).buffer().as_ref());
}

#[cfg(target_arch = "wasm32")]
fn parse_worker_response<Response: DeserializeOwned>(value: JsValue) -> Result<Response, String> {
    let response = worker_response_value(value)?;
    serde_wasm_bindgen::from_value(response).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn worker_response_value(value: JsValue) -> Result<JsValue, String> {
    let ok = Reflect::get(&value, &JsValue::from_str("ok"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !ok {
        let error = Reflect::get(&value, &JsValue::from_str("error"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| "Worker failed".to_string());
        return Err(error);
    }

    Reflect::get(&value, &JsValue::from_str("response")).map_err(js_error_string)
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
