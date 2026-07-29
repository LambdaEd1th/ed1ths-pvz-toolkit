use crate::domain::{ArchiveDocument, PacketDocument};
use crate::editing;
#[cfg(target_arch = "wasm32")]
use rsb_archive::{Part1Extra, UnpackedFile};
#[cfg(target_arch = "wasm32")]
use rsb_preview_worker::{PacketRequest, PacketResponse};
use rsb_preview_worker::{
    PreviewResponse, PreviewSpec, TextureEncodeRequest, TextureEncodeResponse,
};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

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

#[cfg(not(target_arch = "wasm32"))]
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub struct ProcessedPreview {
    pub response: PreviewResponse,
    pub url: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn open_native(path: std::path::PathBuf) -> Result<ArchiveDocument, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    archive_pool().spawn(move || {
        let _ = sender.send(crate::loader::open_native(path));
    });
    receiver
        .await
        .map_err(|_| "RSB 索引后台任务意外终止".to_string())?
}

pub async fn source_bytes(archive: Arc<ArchiveDocument>) -> Result<Vec<u8>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = futures_channel::oneshot::channel();
        archive_pool().spawn(move || {
            let _ = sender.send(archive.source_bytes());
        });
        receiver
            .await
            .map_err(|_| "RSB 读取后台任务意外终止".to_string())?
    }

    #[cfg(target_arch = "wasm32")]
    {
        archive.source_bytes()
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WORKER: RefCell<Option<WorkerClient>> = const { RefCell::new(None) };
    static ENCODE_WORKER: RefCell<Option<WorkerClient>> = const { RefCell::new(None) };
    static EXPORT_WORKER: RefCell<Option<WorkerClient>> = const { RefCell::new(None) };
    static GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub fn begin_request() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        GENERATION.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }
    #[cfg(target_arch = "wasm32")]
    {
        WORKER.with(|slot| {
            let should_cancel = slot
                .borrow()
                .as_ref()
                .is_some_and(WorkerClient::has_pending);
            if should_cancel && let Some(mut worker) = slot.borrow_mut().take() {
                worker.cancel("PTX 预览任务已被新的选择替代");
            }
        });
        GENERATION.with(|generation| {
            let next = generation.get().wrapping_add(1).max(1);
            generation.set(next);
            next
        })
    }
}

pub fn is_current(generation: u64) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        GENERATION.load(Ordering::Relaxed) == generation
    }
    #[cfg(target_arch = "wasm32")]
    {
        GENERATION.with(|current| current.get() == generation)
    }
}

pub async fn perform(
    packet: Arc<PacketDocument>,
    file_index: usize,
    spec: PreviewSpec,
    generation: u64,
) -> Result<ProcessedPreview, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = futures_channel::oneshot::channel();
        preview_pool().spawn(move || {
            let result = packet
                .files
                .get(file_index)
                .ok_or_else(|| "PTX 文件索引已失效".to_string())
                .and_then(|file| {
                    rsb_preview_worker::perform_borrowed(&file.data, &spec, || {
                        !is_current(generation)
                    })
                    .map_err(|error| error.to_string())
                })
                .and_then(|response| {
                    let url = crate::platform::preview_url(&response.png)?;
                    Ok(ProcessedPreview { response, url })
                });
            let _ = sender.send(result);
        });
        receiver
            .await
            .map_err(|_| "PTX 后台任务意外终止".to_string())?
    }

    #[cfg(target_arch = "wasm32")]
    {
        if !is_current(generation) {
            return Err("PTX 预览任务已被新的选择替代".to_string());
        }
        let data = packet
            .files
            .get(file_index)
            .ok_or_else(|| "PTX 文件索引已失效".to_string())?
            .data
            .clone();
        let request = rsb_preview_worker::PreviewRequest { data, spec };
        let serializer = serde_wasm_bindgen::Serializer::new();
        let request = request
            .serialize(&serializer)
            .map_err(|error| error.to_string())?;
        let promise = WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(WorkerClient::new()?);
            }
            slot.as_mut()
                .expect("RSB preview worker was initialized")
                .request("preview", request)
        })?;
        let response = decode_web_response::<PreviewResponse>(promise).await?;
        if !is_current(generation) {
            return Err("PTX 预览任务已被新的选择替代".to_string());
        }
        let url = crate::platform::preview_url(&response.png)?;
        Ok(ProcessedPreview { response, url })
    }
}

pub async fn load_packet(
    archive: Arc<ArchiveDocument>,
    packet_index: usize,
) -> Result<PacketDocument, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = futures_channel::oneshot::channel();
        preview_pool().spawn(move || {
            let _ = sender.send(archive.load_packet(packet_index));
        });
        receiver
            .await
            .map_err(|_| "RSG 后台任务意外终止".to_string())?
    }

    #[cfg(target_arch = "wasm32")]
    {
        let (record, raw) = archive.read_packet_raw(packet_index)?;
        let request = PacketRequest { raw };
        let serializer = serde_wasm_bindgen::Serializer::new();
        let request = request
            .serialize(&serializer)
            .map_err(|error| error.to_string())?;
        let promise = WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(WorkerClient::new()?);
            }
            slot.as_mut()
                .expect("RSB preview worker was initialized")
                .request("unpack", request)
        })?;
        let response = decode_web_response::<PacketResponse>(promise).await?;
        let files = response
            .files
            .into_iter()
            .map(|file| UnpackedFile {
                path: file.path,
                data: file.data,
                is_part1: file.is_part1,
                part1_info: match (file.texture_id, file.width, file.height) {
                    (Some(id), Some(width), Some(height)) => Some(Part1Extra { id, width, height }),
                    _ => None,
                },
            })
            .collect();
        Ok(PacketDocument::new(record, Vec::new(), files))
    }
}

pub async fn rebuild_archive(
    archive: Arc<ArchiveDocument>,
    edit: rsb_archive::RsbArchiveEdit,
) -> Result<Vec<u8>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = futures_channel::oneshot::channel();
        preview_pool().spawn(move || {
            let _ = sender.send(editing::rebuild_archive(&archive, &edit));
        });
        receiver
            .await
            .map_err(|_| "RSB 重建任务意外终止".to_string())?
    }

    #[cfg(target_arch = "wasm32")]
    {
        editing::rebuild_archive(&archive, &edit)
    }
}

pub async fn encode_texture(
    request: TextureEncodeRequest,
) -> Result<TextureEncodeResponse, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = futures_channel::oneshot::channel();
        preview_pool().spawn(move || {
            let result =
                rsb_preview_worker::encode_texture(request).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        receiver
            .await
            .map_err(|_| "PTX 编码后台任务意外终止".to_string())?
    }

    #[cfg(target_arch = "wasm32")]
    {
        let serializer = serde_wasm_bindgen::Serializer::new();
        let request = request
            .serialize(&serializer)
            .map_err(|error| error.to_string())?;
        let promise = ENCODE_WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(WorkerClient::new()?);
            }
            slot.as_mut()
                .expect("RSB worker was initialized")
                .request("encode", request)
        })?;
        decode_web_response::<TextureEncodeResponse>(promise).await
    }
}

pub async fn decode_png(
    packet: Arc<PacketDocument>,
    file_index: usize,
    spec: PreviewSpec,
) -> Result<PreviewResponse, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = futures_channel::oneshot::channel();
        preview_pool().spawn(move || {
            let result = packet
                .files
                .get(file_index)
                .ok_or_else(|| "PTX 文件索引已失效".to_string())
                .and_then(|file| {
                    rsb_preview_worker::perform_borrowed(&file.data, &spec, || false)
                        .map_err(|error| error.to_string())
                });
            let _ = sender.send(result);
        });
        receiver
            .await
            .map_err(|_| "PTX 导出后台任务意外终止".to_string())?
    }

    #[cfg(target_arch = "wasm32")]
    {
        let data = packet
            .files
            .get(file_index)
            .ok_or_else(|| "PTX 文件索引已失效".to_string())?
            .data
            .clone();
        let request = rsb_preview_worker::PreviewRequest { data, spec };
        let serializer = serde_wasm_bindgen::Serializer::new();
        let request = request
            .serialize(&serializer)
            .map_err(|error| error.to_string())?;
        let promise = EXPORT_WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(WorkerClient::new()?);
            }
            slot.as_mut()
                .expect("RSB export worker was initialized")
                .request("preview", request)
        })?;
        decode_web_response::<PreviewResponse>(promise).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn preview_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(2)
            .saturating_sub(1)
            .clamp(1, 2);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|index| format!("rsb-preview-{index}"))
            .build()
            .expect("failed to create RSB preview thread pool")
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn archive_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .thread_name(|_| "rsb-archive-io".to_string())
            .build()
            .expect("failed to create RSB archive thread pool")
    })
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
    fn has_pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }

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
        let envelope = Object::new();
        Reflect::set(
            envelope.as_ref(),
            &JsValue::from_str("id"),
            &JsValue::from_f64(id as f64),
        )
        .map_err(js_error_string)?;
        let request = Object::new();
        Reflect::set(
            request.as_ref(),
            &JsValue::from_str("kind"),
            &JsValue::from_str(kind),
        )
        .map_err(js_error_string)?;
        Reflect::set(request.as_ref(), &JsValue::from_str("payload"), &payload)
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

    fn cancel(&mut self, message: &str) {
        self.worker.terminate();
        let error = JsValue::from_str(message);
        for (_, request) in self.pending.borrow_mut().drain() {
            let _ = request.reject.call1(&JsValue::UNDEFINED, &error);
        }
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
            .unwrap_or_else(|| "PTX Preview Worker 执行失败".to_string()));
    }
    let response =
        Reflect::get(&envelope, &JsValue::from_str("response")).map_err(js_error_string)?;
    serde_wasm_bindgen::from_value(response).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn worker_url() -> String {
    let root = crate::RSB_ASSETS.to_string();
    format!(
        "{}/worker/rsb-worker.js?v=20260728-rgba8-only-api-1",
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
        .unwrap_or_else(|| "PTX Preview Worker 加载失败".to_string())
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
