#[cfg(target_arch = "wasm32")]
use std::io::{Cursor, Write};

use crate::model::{ArchiveSummary, BuildRequest, BuiltArchive, MaterializeRequest, NamedBytes};
use crate::service::ArchiveService;

enum ArchiveTask {
    Open {
        main_name: String,
        main_bytes: Vec<u8>,
        auxiliary_files: Vec<NamedBytes>,
    },
    Materialize(MaterializeRequest),
    Build(BuildRequest),
    Close(u64),
}

enum ArchiveResponse {
    Opened(ArchiveSummary),
    Files(Vec<NamedBytes>),
    Built(BuiltArchive),
    Closed,
}

pub(crate) async fn open_archive(
    main_name: String,
    main_bytes: Vec<u8>,
    auxiliary_files: Vec<NamedBytes>,
) -> Result<ArchiveSummary, String> {
    match run_task(ArchiveTask::Open {
        main_name,
        main_bytes,
        auxiliary_files,
    })
    .await?
    {
        ArchiveResponse::Opened(summary) => Ok(summary),
        _ => Err("DZip 后端返回了错误的打开结果".to_string()),
    }
}

pub(crate) async fn materialize_entries(
    request: MaterializeRequest,
) -> Result<Vec<NamedBytes>, String> {
    match run_task(ArchiveTask::Materialize(request)).await? {
        ArchiveResponse::Files(files) => Ok(files),
        _ => Err("DZip 后端返回了错误的提取结果".to_string()),
    }
}

pub(crate) async fn build_archive(request: BuildRequest) -> Result<BuiltArchive, String> {
    match run_task(ArchiveTask::Build(request)).await? {
        ArchiveResponse::Built(archive) => Ok(archive),
        _ => Err("DZip 后端返回了错误的构建结果".to_string()),
    }
}

pub(crate) async fn close_archive(session_id: u64) {
    let _ = run_task(ArchiveTask::Close(session_id)).await;
}

fn execute_task(
    service: &mut ArchiveService,
    task: ArchiveTask,
) -> Result<ArchiveResponse, String> {
    match task {
        ArchiveTask::Open {
            main_name,
            main_bytes,
            auxiliary_files,
        } => service
            .open(main_name, main_bytes, auxiliary_files)
            .map(ArchiveResponse::Opened),
        ArchiveTask::Materialize(request) => {
            service.materialize(request).map(ArchiveResponse::Files)
        }
        ArchiveTask::Build(request) => service.build(request).map(ArchiveResponse::Built),
        ArchiveTask::Close(session_id) => {
            service.close(session_id);
            Ok(ArchiveResponse::Closed)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_task(task: ArchiveTask) -> Result<ArchiveResponse, String> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{OnceLock, mpsc};

    struct Message {
        task: ArchiveTask,
        reply: futures_channel::oneshot::Sender<Result<ArchiveResponse, String>>,
    }

    static WORKER: OnceLock<mpsc::Sender<Message>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Message>();
        std::thread::Builder::new()
            .name("toolkit-dzip-backend".to_string())
            .spawn(move || {
                let mut service = ArchiveService::default();
                for message in receiver {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        execute_task(&mut service, message.task)
                    }))
                    .unwrap_or_else(|_| Err("DZip 后台任务意外终止".to_string()));
                    let _ = message.reply.send(result);
                }
            })
            .expect("failed to start DZip archive worker");
        sender
    });
    let (reply, receiver) = futures_channel::oneshot::channel();
    worker
        .send(Message { task, reply })
        .map_err(|_| "DZip 后台服务不可用".to_string())?;
    receiver
        .await
        .map_err(|_| "DZip 后台服务已断开".to_string())?
}

#[cfg(target_arch = "wasm32")]
async fn run_task(task: ArchiveTask) -> Result<ArchiveResponse, String> {
    use std::cell::RefCell;

    thread_local! {
        static SERVICE: RefCell<ArchiveService> = RefCell::new(ArchiveService::default());
    }
    SERVICE.with(|service| execute_task(&mut service.borrow_mut(), task))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn save_volumes(volumes: Vec<NamedBytes>) -> Result<Option<String>, String> {
    if volumes.is_empty() {
        return Err("归档没有生成任何分卷".to_string());
    }
    if volumes.len() == 1 {
        let volume = volumes.into_iter().next().expect("checked one volume");
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_file_name(&volume.name)
            .add_filter("DZip Archive", &["dz", "dzip"])
            .save_file()
            .await
        else {
            return Ok(None);
        };
        std::fs::write(file.path(), volume.bytes)
            .map_err(|error| format!("无法保存 {}：{error}", file.path().display()))?;
        return Ok(Some(file.path().display().to_string()));
    }

    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_title("选择 DZip 分卷保存目录")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let root = folder.path();
    for volume in &volumes {
        let relative = dzip::path::resolve_relative_path(&volume.name)
            .map_err(|error| format!("不安全的分卷名称 {}：{error}", volume.name))?;
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
        }
        std::fs::write(&target, &volume.bytes)
            .map_err(|error| format!("无法保存 {}：{error}", target.display()))?;
    }
    Ok(Some(format!(
        "{} 个分卷已保存到 {}",
        volumes.len(),
        root.display()
    )))
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn save_volumes(mut volumes: Vec<NamedBytes>) -> Result<Option<String>, String> {
    if volumes.is_empty() {
        return Err("归档没有生成任何分卷".to_string());
    }
    if volumes.len() == 1 {
        let volume = volumes.pop().expect("checked one volume");
        download(&volume.name, &volume.bytes)?;
        return Ok(Some(format!("已下载 {}", volume.name)));
    }
    let main_name = volumes[0].name.clone();
    let count = volumes.len();
    let bytes = store_zip(&volumes)?;
    let stem = archive_stem(&main_name);
    let download_name = format!("{stem}-volumes.zip");
    download(&download_name, &bytes)?;
    Ok(Some(format!("已将 {count} 个分卷打包为 {download_name}")))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn export_files(files: Vec<NamedBytes>) -> Result<Option<String>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_title("选择 DZip 提取目录")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let root = folder.path();
    for file in &files {
        let relative = dzip::path::resolve_relative_path(&file.name)
            .map_err(|error| format!("不安全的归档路径 {}：{error}", file.name))?;
        let target = root.join(relative);
        if target
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(format!("拒绝覆盖符号链接：{}", target.display()));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
        }
        std::fs::write(&target, &file.bytes)
            .map_err(|error| format!("无法写入 {}：{error}", target.display()))?;
    }
    Ok(Some(format!(
        "已提取 {} 个文件到 {}",
        files.len(),
        root.display()
    )))
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn export_files(files: Vec<NamedBytes>) -> Result<Option<String>, String> {
    let count = files.len();
    let bytes = store_zip(&files)?;
    let name = "dzip-extracted.zip";
    download(name, &bytes)?;
    Ok(Some(format!("已将 {count} 个文件打包为 {name}")))
}

#[cfg(target_arch = "wasm32")]
fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "浏览器窗口不可用".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "浏览器文档不可用".to_string())?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(array.as_ref());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)
        .map_err(|error| format!("无法创建下载内容：{error:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|error| format!("无法创建下载地址：{error:?}"))?;
    let anchor = document
        .create_element("a")
        .map_err(|error| format!("无法创建下载链接：{error:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "浏览器不支持文件下载".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn archive_stem(name: &str) -> &str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".dzip") {
        &name[..name.len() - 5]
    } else if lower.ends_with(".dz") {
        &name[..name.len() - 3]
    } else {
        name
    }
}

#[cfg(target_arch = "wasm32")]
fn store_zip(files: &[NamedBytes]) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut archive = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for file in files {
            let name = file.name.replace('\\', "/");
            archive
                .start_file(name, options)
                .map_err(|error| format!("无法创建 ZIP 条目：{error}"))?;
            archive
                .write_all(&file.bytes)
                .map_err(|error| format!("无法写入 ZIP 条目：{error}"))?;
        }
        archive
            .finish()
            .map_err(|error| format!("无法完成 ZIP：{error}"))?;
    }
    Ok(cursor.into_inner())
}
