use pam_viewer_core::{WorkerRequest, WorkerResponse};

#[cfg(not(target_arch = "wasm32"))]
use std::panic::{AssertUnwindSafe, catch_unwind};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
#[cfg(target_arch = "wasm32")]
use serde::Serialize;
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
use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

#[cfg(target_arch = "wasm32")]
const PROCESSING_WORKER_VERSION: &str = "20260723-worker-runtime-3";

#[cfg(target_arch = "wasm32")]
struct PendingRequest {
    resolve: Function,
    reject: Function,
}

#[cfg(target_arch = "wasm32")]
struct WorkerClient {
    worker: Worker,
    next_id: u64,
    pending: Rc<RefCell<HashMap<u64, PendingRequest>>>,
    failed: Rc<Cell<bool>>,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
    _onerror: Closure<dyn FnMut(JsValue)>,
}

#[cfg(target_arch = "wasm32")]
struct WorkerPool {
    workers: Vec<WorkerClient>,
    document_routes: HashMap<u64, usize>,
    next_worker: usize,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
enum Route {
    Any,
    Create(u64),
    Existing(u64),
    Release(u64),
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WORKER_POOL: RefCell<Option<WorkerPool>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
impl WorkerClient {
    fn new() -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = Worker::new_with_options(&processing_worker_url(), &options)
            .map_err(js_error_string)?;
        let pending = Rc::new(RefCell::new(HashMap::<u64, PendingRequest>::new()));
        let failed = Rc::new(Cell::new(false));

        let message_pending = Rc::clone(&pending);
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let response = event.data();
            let Some(id) = message_id(&response) else {
                return;
            };
            if let Some(request) = message_pending.borrow_mut().remove(&id) {
                let _ = request.resolve.call1(&JsValue::UNDEFINED, &response);
            }
        });
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let error_pending = Rc::clone(&pending);
        let error_failed = Rc::clone(&failed);
        let onerror = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
            error_failed.set(true);
            let message = worker_error_message(&event);
            crate::platform::log_buffer::push("ERROR", &message);
            let error = JsValue::from_str(&message);
            for (_, request) in error_pending.borrow_mut().drain() {
                let _ = request.reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Ok(Self {
            worker,
            next_id: 1,
            pending,
            failed,
            _onmessage: onmessage,
            _onerror: onerror,
        })
    }

    fn request(&mut self, request: JsValue) -> Result<Promise, String> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let message = Object::new();
        Reflect::set(
            message.as_ref(),
            &JsValue::from_str("id"),
            &JsValue::from_f64(id as f64),
        )
        .map_err(js_error_string)?;
        Reflect::set(message.as_ref(), &JsValue::from_str("request"), &request)
            .map_err(js_error_string)?;

        let worker = self.worker.clone();
        let pending = Rc::clone(&self.pending);
        let failed = Rc::clone(&self.failed);
        let transfer = transfer_list(&request);
        Ok(Promise::new(
            &mut move |resolve: Function, reject: Function| {
                pending.borrow_mut().insert(
                    id,
                    PendingRequest {
                        resolve,
                        reject: reject.clone(),
                    },
                );
                let posted = if transfer.length() == 0 {
                    worker.post_message(message.as_ref())
                } else {
                    worker.post_message_with_transfer(message.as_ref(), transfer.as_ref())
                };
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
impl Drop for WorkerClient {
    fn drop(&mut self) {
        self.worker.terminate();
    }
}

#[cfg(target_arch = "wasm32")]
impl WorkerPool {
    fn new() -> Result<Self, String> {
        let worker_count = web_worker_count();
        let workers = (0..worker_count)
            .map(|_| WorkerClient::new())
            .collect::<Result<Vec<_>, _>>()?;
        record_pool_runtime(worker_count);
        Ok(Self {
            workers,
            document_routes: HashMap::new(),
            next_worker: 0,
        })
    }

    fn has_usable_worker(&self) -> bool {
        self.workers.iter().any(|worker| !worker.failed.get())
    }

    fn least_busy_worker(&self) -> Option<usize> {
        let worker_count = self.workers.len();
        (0..worker_count)
            .map(|offset| (self.next_worker + offset) % worker_count)
            .filter(|&index| !self.workers[index].failed.get())
            .min_by_key(|&index| self.workers[index].pending.borrow().len())
    }

    fn request(&mut self, request: JsValue, route: Route) -> Result<Promise, String> {
        let selected = match route {
            Route::Existing(document_id) | Route::Release(document_id) => self
                .document_routes
                .get(&document_id)
                .copied()
                .filter(|index| !self.workers[*index].failed.get())
                .ok_or_else(|| format!("Worker document {document_id} has no active route"))?,
            Route::Create(document_id) => {
                if let Some(index) = self
                    .document_routes
                    .get(&document_id)
                    .copied()
                    .filter(|index| !self.workers[*index].failed.get())
                {
                    index
                } else {
                    let index = self
                        .least_busy_worker()
                        .ok_or_else(|| "All Processing Workers have failed".to_string())?;
                    self.document_routes.insert(document_id, index);
                    index
                }
            }
            Route::Any => self
                .least_busy_worker()
                .ok_or_else(|| "All Processing Workers have failed".to_string())?,
        };
        self.next_worker = (selected + 1) % self.workers.len();
        let promise = self.workers[selected].request(request);
        if let Route::Release(document_id) = route {
            self.document_routes.remove(&document_id);
        }
        promise
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn perform(request: WorkerRequest) -> Result<WorkerResponse, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    processing_pool().spawn(move || {
        let response = catch_unwind(AssertUnwindSafe(|| {
            pollster::block_on(pam_viewer_worker::perform_worker_request(request))
        }))
        .unwrap_or_else(|_| WorkerResponse::Error {
            message: "Native processing task panicked".to_string(),
        });
        let _ = sender.send(response);
    });
    let response = receiver
        .await
        .map_err(|_| "Native processing task was cancelled".to_string())?;
    normalize(response)
}

#[cfg(not(target_arch = "wasm32"))]
fn processing_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(2)
            .saturating_sub(1)
            .max(1);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|index| format!("pam-processing-{index}"))
            .build()
            .expect("failed to create PAM processing thread pool")
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn read_folder(
    root: std::path::PathBuf,
) -> Result<Vec<pam_viewer_core::WorkerInputFile>, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    processing_pool().spawn(move || {
        let _ = sender.send(crate::platform::input_files_in_folder(&root));
    });
    receiver
        .await
        .map_err(|_| "Native folder read was cancelled".to_string())?
}

#[cfg(target_arch = "wasm32")]
pub async fn warm_up() -> Result<(), String> {
    let result = match perform(WorkerRequest::Batch {
        requests: (0..web_worker_count())
            .map(|_| WorkerRequest::Ping)
            .collect(),
    })
    .await
    {
        Ok(WorkerResponse::Batch { responses })
            if responses
                .iter()
                .all(|response| matches!(response, WorkerResponse::Pong)) =>
        {
            Ok(())
        }
        Ok(_) => Err("Unexpected Processing Worker response".to_string()),
        Err(error) => Err(error),
    };
    record_status(if result.is_ok() { "ready" } else { "error" });
    result
}

#[cfg(target_arch = "wasm32")]
pub async fn perform(request: WorkerRequest) -> Result<WorkerResponse, String> {
    let response = match request {
        WorkerRequest::Batch { requests } => {
            let promises = requests
                .into_iter()
                .map(submit_web_request)
                .collect::<Result<Vec<_>, _>>()?;
            let mut responses = Vec::with_capacity(promises.len());
            for promise in promises {
                responses.push(decode_web_response(promise).await?);
            }
            WorkerResponse::Batch { responses }
        }
        request => decode_web_response(submit_web_request(request)?).await?,
    };
    normalize(response)
}

#[cfg(target_arch = "wasm32")]
fn submit_web_request(request: WorkerRequest) -> Result<Promise, String> {
    let route = request_route(&request);
    let serializer =
        serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
    let request = request
        .serialize(&serializer)
        .map_err(|error| error.to_string())?;
    WORKER_POOL.with(|slot| {
        let mut slot = slot.borrow_mut();
        let recreate = slot.as_ref().is_none_or(|pool| !pool.has_usable_worker());
        if recreate {
            *slot = Some(WorkerPool::new()?);
        }
        slot.as_mut()
            .expect("processing worker pool was initialized")
            .request(request, route)
    })
}

#[cfg(target_arch = "wasm32")]
fn request_route(request: &WorkerRequest) -> Route {
    match request {
        WorkerRequest::Load { document_id, .. }
        | WorkerRequest::RegisterDocument { document_id, .. } => Route::Create(*document_id),
        WorkerRequest::Export(request) => Route::Existing(request.document_id),
        WorkerRequest::CancelExport { document_id, .. } => Route::Existing(*document_id),
        WorkerRequest::ReleaseDocument { document_id } => Route::Release(*document_id),
        WorkerRequest::Ping | WorkerRequest::Batch { .. } => Route::Any,
    }
}

#[cfg(target_arch = "wasm32")]
async fn decode_web_response(promise: Promise) -> Result<WorkerResponse, String> {
    let envelope = JsFuture::from(promise).await.map_err(js_error_string)?;
    let ok = Reflect::get(&envelope, &JsValue::from_str("ok"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !ok {
        return Err(Reflect::get(&envelope, &JsValue::from_str("error"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| "Processing Worker failed".to_string()));
    }
    let response =
        Reflect::get(&envelope, &JsValue::from_str("response")).map_err(js_error_string)?;
    serde_wasm_bindgen::from_value(response).map_err(|error| error.to_string())
}

fn normalize(response: WorkerResponse) -> Result<WorkerResponse, String> {
    match response {
        WorkerResponse::Error { message } => Err(message),
        response => Ok(response),
    }
}

#[cfg(target_arch = "wasm32")]
fn processing_worker_url() -> String {
    let asset_root = crate::components::APP_ASSETS.to_string();
    format!(
        "{}/worker/pam-worker.js?v={PROCESSING_WORKER_VERSION}",
        asset_root.trim_end_matches('/')
    )
}

#[cfg(target_arch = "wasm32")]
fn worker_error_message(event: &JsValue) -> String {
    Reflect::get(event, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| "Processing Worker failed to load".to_string())
}

#[cfg(target_arch = "wasm32")]
fn message_id(value: &JsValue) -> Option<u64> {
    Reflect::get(value, &JsValue::from_str("id"))
        .ok()?
        .as_f64()
        .map(|value| value as u64)
}

#[cfg(target_arch = "wasm32")]
fn transfer_list(value: &JsValue) -> Array {
    let transfer = Array::new();
    collect_transferables(value, &transfer);
    transfer
}

#[cfg(target_arch = "wasm32")]
fn collect_transferables(value: &JsValue, transfer: &Array) {
    if value.is_instance_of::<Uint8Array>() {
        transfer.push(Uint8Array::new(value).buffer().as_ref());
        return;
    }
    if Array::is_array(value) {
        for item in Array::from(value) {
            collect_transferables(&item, transfer);
        }
        return;
    }
    if !value.is_object() {
        return;
    }
    for key in Object::keys(&Object::from(value.clone())) {
        if let Ok(item) = Reflect::get(value, &key) {
            collect_transferables(&item, transfer);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn record_status(status: &str) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("data-processing-worker", status);
    }
}

#[cfg(target_arch = "wasm32")]
fn record_pool_runtime(worker_count: usize) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let _ = root.set_attribute("data-processing-worker-backend", "worker-pool");
    let _ = root.set_attribute("data-processing-worker-threads", &worker_count.to_string());
}

#[cfg(target_arch = "wasm32")]
fn web_worker_count() -> usize {
    let hardware_concurrency = web_sys::window()
        .map(|window| window.navigator().hardware_concurrency() as usize)
        .unwrap_or(1);
    (hardware_concurrency.saturating_sub(1) / 2).clamp(1, 4)
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {error:?}"))
}
