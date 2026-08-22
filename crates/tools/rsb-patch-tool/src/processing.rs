use rsb_patch_worker::{
    AppliedArchive, ApplyRequest, CreateRequest, CreatedPatch, InspectRequest, InspectedPatch,
};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
#[cfg(target_arch = "wasm32")]
use serde::{Serialize, de::DeserializeOwned};
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

pub async fn inspect_patch(request: InspectRequest) -> Result<InspectedPatch, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        run_native(move || rsb_patch_worker::inspect(request)).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        request_worker("inspect", request).await
    }
}

pub async fn create_patch(request: CreateRequest) -> Result<CreatedPatch, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        run_native(move || rsb_patch_worker::create(request)).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        request_worker("create", request).await
    }
}

pub async fn apply_patch(request: ApplyRequest) -> Result<AppliedArchive, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        run_native(move || rsb_patch_worker::apply(request)).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        request_worker("apply", request).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_native<T: Send + 'static>(
    task: impl FnOnce() -> rsb_patch_worker::Result<T> + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    worker_pool().spawn(move || {
        let result = task().map_err(|error| error.to_string());
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| "RSBP 后台任务意外终止".to_string())?
}

#[cfg(not(target_arch = "wasm32"))]
fn worker_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .thread_name(|_| "rsb-patch-worker".to_string())
            .build()
            .expect("failed to create RSBPatch worker pool")
    })
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WORKER: RefCell<Option<WorkerClient>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
async fn request_worker<I, O>(kind: &str, request: I) -> Result<O, String>
where
    I: Serialize,
    O: DeserializeOwned,
{
    let serializer = serde_wasm_bindgen::Serializer::new();
    let payload = request
        .serialize(&serializer)
        .map_err(|error| error.to_string())?;
    let promise = WORKER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(WorkerClient::new()?);
        }
        slot.as_mut()
            .expect("RSBP worker was initialized")
            .request(kind, payload)
    })?;
    decode_web_response(promise).await
}

#[cfg(target_arch = "wasm32")]
struct PendingRequest {
    resolve: Function,
    reject: Function,
}

#[cfg(target_arch = "wasm32")]
struct WorkerClient {
    worker: Worker,
    next_id: u64,
    pending: Rc<RefCell<std::collections::HashMap<u64, PendingRequest>>>,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
    _onerror: Closure<dyn FnMut(JsValue)>,
}

#[cfg(target_arch = "wasm32")]
impl WorkerClient {
    fn new() -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = Worker::new_with_options(&worker_url(), &options).map_err(js_error_string)?;
        let pending = Rc::new(RefCell::new(
            std::collections::HashMap::<u64, PendingRequest>::new(),
        ));

        let message_pending = Rc::clone(&pending);
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let envelope = event.data();
            let Some(id) = message_id(&envelope) else {
                return;
            };
            if let Some(request) = message_pending.borrow_mut().remove(&id) {
                let _ = request.resolve.call1(&JsValue::UNDEFINED, &envelope);
            }
        });
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let error_pending = Rc::clone(&pending);
        let onerror = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
            let error = JsValue::from_str(&worker_error_message(&event));
            for (_, request) in error_pending.borrow_mut().drain() {
                let _ = request.reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Ok(Self {
            worker,
            next_id: 1,
            pending,
            _onmessage: onmessage,
            _onerror: onerror,
        })
    }

    fn request(&mut self, kind: &str, payload: JsValue) -> Result<Promise, String> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let request = Object::new();
        Reflect::set(
            request.as_ref(),
            &JsValue::from_str("kind"),
            &JsValue::from_str(kind),
        )
        .map_err(js_error_string)?;
        Reflect::set(request.as_ref(), &JsValue::from_str("payload"), &payload)
            .map_err(js_error_string)?;
        let envelope = Object::new();
        Reflect::set(
            envelope.as_ref(),
            &JsValue::from_str("id"),
            &JsValue::from_f64(id as f64),
        )
        .map_err(js_error_string)?;
        Reflect::set(
            envelope.as_ref(),
            &JsValue::from_str("request"),
            request.as_ref(),
        )
        .map_err(js_error_string)?;

        let transfer = transfer_list(request.as_ref());
        let worker = self.worker.clone();
        let pending = Rc::clone(&self.pending);
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
                    worker.post_message(envelope.as_ref())
                } else {
                    worker.post_message_with_transfer(envelope.as_ref(), transfer.as_ref())
                };
                if let Err(error) = posted {
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
async fn decode_web_response<T: DeserializeOwned>(promise: Promise) -> Result<T, String> {
    let envelope = JsFuture::from(promise).await.map_err(js_error_string)?;
    let ok = Reflect::get(&envelope, &JsValue::from_str("ok"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !ok {
        return Err(Reflect::get(&envelope, &JsValue::from_str("error"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| "RSBP Worker 执行失败".to_string()));
    }
    let response =
        Reflect::get(&envelope, &JsValue::from_str("response")).map_err(js_error_string)?;
    serde_wasm_bindgen::from_value(response).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn worker_url() -> String {
    let root = crate::RSB_PATCH_ASSETS.to_string();
    format!(
        "{}/worker/rsb-patch-worker.js?v=20260822-rsb-patch-1",
        root.trim_end_matches('/')
    )
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
fn worker_error_message(event: &JsValue) -> String {
    Reflect::get(event, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "RSBP Worker 加载失败".to_string())
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
