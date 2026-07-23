use dioxus::prelude::*;
use rton_editor_core::EncodeOptions;
#[cfg(target_arch = "wasm32")]
use rton_editor_core::{SourceFormat, WorkerBatchJob, WorkerBatchRequest, WorkerBatchSource};
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

use crate::components::FileListItem;
use crate::domain::{
    BatchExportMode, EditorTabState, Status, Tone, ZipArchiveBuilder, batch_archive_name,
    batch_output_path, unique_zip_path,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::{OpenTabError, document_for_owned_tab, encode_batch_export_document};
#[cfg(target_arch = "wasm32")]
use crate::file_import::LoadedFileSource;
use crate::file_import::LoadedFileState;
#[cfg(not(target_arch = "wasm32"))]
use crate::file_import::document_from_loaded_file_sync;
use crate::i18n::I18n;
use crate::platform;
#[cfg(target_arch = "wasm32")]
use crate::platform::run_batch_worker;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn batch_export_selected_files(
    selected_items: Vec<FileListItem>,
    tabs: Vec<EditorTabState>,
    loaded_files: Vec<LoadedFileState>,
    mode: BatchExportMode,
    options: EncodeOptions,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    if selected_items.is_empty() {
        status.set(Status::new(i18n.t("status-select-files-first"), Tone::Warn));
        return;
    }

    status.set(Status::new(
        i18n.t_args(
            "status-batch-converting",
            &[
                ("count", selected_items.len().to_string()),
                ("format", mode.label().to_string()),
            ],
        ),
        Tone::Warn,
    ));

    let jobs = build_batch_export_jobs(selected_items, &tabs, &loaded_files, mode, i18n);
    let mut results = run_batch_export_jobs(jobs, mode, options, status, i18n).await;
    results.sort_by_key(|item| item.index);
    let mut used_paths = HashSet::new();
    let mut archive = ZipArchiveBuilder::new();
    let mut exported_count = 0usize;
    let mut errors = Vec::new();

    for item in results {
        match item.result {
            Ok(bytes) => {
                let path = unique_zip_path(&item.output_path, &mut used_paths);
                match archive.push_entry(&path, &bytes) {
                    Ok(()) => exported_count += 1,
                    Err(error) => errors.push(format!("{}: {error}", item.source_path)),
                }
            }
            Err(error) => errors.push(format!("{}: {error}", item.source_path)),
        }
    }

    if exported_count == 0 {
        status.set(Status::new(
            errors
                .first()
                .cloned()
                .unwrap_or_else(|| i18n.t("status-no-batch-success")),
            Tone::Error,
        ));
        return;
    }

    let zip_bytes = match archive.finish() {
        Ok(bytes) => bytes,
        Err(error) => {
            status.set(Status::new(error, Tone::Error));
            return;
        }
    };
    let archive_name = batch_archive_name(mode);
    match platform::save_bytes(&archive_name, &zip_bytes) {
        Ok(true) => {
            let suffix = if errors.is_empty() {
                String::new()
            } else {
                i18n.t_args(
                    "status-batch-failure-suffix",
                    &[("count", errors.len().to_string())],
                )
            };
            status.set(Status::new(
                i18n.t_args(
                    "status-batch-exported",
                    &[
                        ("count", exported_count.to_string()),
                        ("format", mode.label().to_string()),
                        ("suffix", suffix),
                    ],
                ),
                if errors.is_empty() {
                    Tone::Ok
                } else {
                    Tone::Warn
                },
            ));
        }
        Ok(false) => status.set(Status::new(i18n.t("status-export-cancelled"), Tone::Info)),
        Err(error) => status.set(Status::new(error, Tone::Error)),
    }
}

struct BatchExportJob {
    index: usize,
    source_path: String,
    output_path: String,
    source: Result<BatchDocumentSource, String>,
}

enum BatchDocumentSource {
    Tab(Box<EditorTabState>),
    LoadedFile(LoadedFileState),
}

struct BatchExportItemResult {
    index: usize,
    source_path: String,
    output_path: String,
    result: Result<Vec<u8>, String>,
}

fn build_batch_export_jobs(
    selected_items: Vec<FileListItem>,
    tabs: &[EditorTabState],
    loaded_files: &[LoadedFileState],
    mode: BatchExportMode,
    i18n: I18n,
) -> Vec<BatchExportJob> {
    selected_items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let source_path = item.path;
            let output_path = batch_output_path(&source_path, mode);
            let source =
                resolve_batch_export_source(item.tab_id, item.file_id, tabs, loaded_files, i18n);
            BatchExportJob {
                index,
                source_path,
                output_path,
                source,
            }
        })
        .collect()
}

fn resolve_batch_export_source(
    tab_id: Option<usize>,
    file_id: Option<usize>,
    tabs: &[EditorTabState],
    loaded_files: &[LoadedFileState],
    i18n: I18n,
) -> Result<BatchDocumentSource, String> {
    if let Some(tab_id) = tab_id
        && let Some(tab) = tabs.iter().find(|tab| tab.id == tab_id).cloned()
    {
        return Ok(BatchDocumentSource::Tab(Box::new(tab)));
    }

    if let Some(file_id) = file_id {
        let file = loaded_files
            .iter()
            .find(|file| file.id == file_id)
            .ok_or_else(|| i18n.t("status-file-list-item-stale"))?;
        return Ok(BatchDocumentSource::LoadedFile(file.clone()));
    }

    Err(i18n.t("status-no-export-value"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_batch_export_jobs(
    jobs: Vec<BatchExportJob>,
    mode: BatchExportMode,
    options: EncodeOptions,
    status: Signal<Status>,
    i18n: I18n,
) -> Vec<BatchExportItemResult> {
    use rayon::prelude::*;

    let total = jobs.len();
    if total == 0 {
        return Vec::new();
    }

    let source_paths = jobs
        .iter()
        .map(|job| job.source_path.clone())
        .collect::<Vec<_>>();
    let (sender, receiver) = mpsc::channel::<BatchExportItemResult>();
    let worker = std::thread::spawn(move || {
        let thread_count = batch_export_thread_count();
        match rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build()
        {
            Ok(pool) => {
                pool.install(|| {
                    jobs.into_par_iter().for_each_with(sender, |sender, job| {
                        let _ = sender.send(process_batch_export_job_sync(job, mode, options));
                    });
                });
            }
            Err(_) => {
                for job in jobs {
                    let _ = sender.send(process_batch_export_job_sync(job, mode, options));
                }
            }
        }
    });

    let mut completed = 0usize;
    let mut last_reported = 0usize;
    let mut disconnected = false;
    let mut results = Vec::with_capacity(total);
    results.resize_with(total, || None);

    while completed < total && !disconnected {
        let mut received_any = false;
        loop {
            match receiver.try_recv() {
                Ok(result) => {
                    received_any = true;
                    completed += 1;
                    if result.index < results.len() {
                        let index = result.index;
                        results[index] = Some(result);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if completed == total || completed.saturating_sub(last_reported) >= 24 {
            last_reported = completed;
            set_batch_progress_status(status, i18n, mode, completed, total);
        }

        if completed < total && !received_any && !disconnected {
            platform::sleep_ms(16).await;
        }
    }

    let panicked = worker.join().is_err();
    for (index, result) in results.iter_mut().enumerate() {
        if result.is_none() {
            *result = Some(BatchExportItemResult {
                index,
                source_path: source_paths
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "batch export".to_string()),
                output_path: String::new(),
                result: Err(if panicked {
                    "Batch export worker stopped unexpectedly".to_string()
                } else {
                    "Batch export result was not produced".to_string()
                }),
            });
        }
    }

    results.into_iter().flatten().collect()
}

#[cfg(target_arch = "wasm32")]
async fn run_batch_export_jobs(
    jobs: Vec<BatchExportJob>,
    mode: BatchExportMode,
    options: EncodeOptions,
    status: Signal<Status>,
    i18n: I18n,
) -> Vec<BatchExportItemResult> {
    let total = jobs.len();
    let mut results = Vec::with_capacity(total);
    let mut jobs = jobs.into_iter();
    let mut completed = 0usize;

    loop {
        let chunk = jobs.by_ref().take(32).collect::<Vec<_>>();
        if chunk.is_empty() {
            break;
        }

        let chunk_len = chunk.len();
        let mut metadata = HashMap::with_capacity(chunk_len);
        let mut worker_jobs = Vec::with_capacity(chunk_len);
        for job in chunk {
            let BatchExportJob {
                index,
                source_path,
                output_path,
                source,
            } = job;
            metadata.insert(index, (source_path, output_path));
            match worker_batch_source(source).await {
                Ok(source) => worker_jobs.push(WorkerBatchJob { index, source }),
                Err(error) => {
                    let (source_path, output_path) = metadata
                        .remove(&index)
                        .expect("batch metadata was inserted");
                    results.push(BatchExportItemResult {
                        index,
                        source_path,
                        output_path,
                        result: Err(error),
                    });
                }
            }
        }

        if !worker_jobs.is_empty() {
            match run_batch_worker(WorkerBatchRequest {
                jobs: worker_jobs,
                mode,
                encode_options: options,
            })
            .await
            {
                Ok(response) => {
                    for item in response.results {
                        let Some((source_path, output_path)) = metadata.remove(&item.index) else {
                            continue;
                        };
                        results.push(BatchExportItemResult {
                            index: item.index,
                            source_path,
                            output_path,
                            result: item.error.map_or(Ok(item.bytes), Err),
                        });
                    }
                }
                Err(error) => {
                    for (index, (source_path, output_path)) in metadata.drain() {
                        results.push(BatchExportItemResult {
                            index,
                            source_path,
                            output_path,
                            result: Err(error.clone()),
                        });
                    }
                }
            }
        }

        for (index, (source_path, output_path)) in metadata.drain() {
            results.push(BatchExportItemResult {
                index,
                source_path,
                output_path,
                result: Err("Batch worker did not return a result".to_string()),
            });
        }

        completed += chunk_len;
        set_batch_progress_status(status, i18n, mode, completed, total);
    }
    results
}

#[cfg(not(target_arch = "wasm32"))]
fn process_batch_export_job_sync(
    job: BatchExportJob,
    mode: BatchExportMode,
    options: EncodeOptions,
) -> BatchExportItemResult {
    let result = (|| {
        let doc = match job.source {
            Ok(BatchDocumentSource::Tab(tab)) => {
                document_for_owned_tab(*tab).map_err(|error| error.to_string())?
            }
            Ok(BatchDocumentSource::LoadedFile(file)) => {
                document_from_loaded_file_sync(&file).map_err(OpenTabError::message)?
            }
            Err(error) => return Err(error),
        };
        encode_batch_export_document(&doc, mode, options)
    })();

    BatchExportItemResult {
        index: job.index,
        source_path: job.source_path,
        output_path: job.output_path,
        result,
    }
}

#[cfg(target_arch = "wasm32")]
async fn worker_batch_source(
    source: Result<BatchDocumentSource, String>,
) -> Result<WorkerBatchSource, String> {
    match source {
        Ok(BatchDocumentSource::Tab(tab)) => worker_batch_source_from_tab(*tab),
        Ok(BatchDocumentSource::LoadedFile(file)) => worker_batch_source_from_file(file).await,
        Err(error) => Err(error),
    }
}

#[cfg(target_arch = "wasm32")]
fn worker_batch_source_from_tab(tab: EditorTabState) -> Result<WorkerBatchSource, String> {
    if !tab.dirty
        && let Some(document_id) = tab.worker_document_id
    {
        return Ok(WorkerBatchSource::DocumentId(document_id));
    }

    let format = tab.mode.text_format();
    let bytes = if format.is_some() {
        tab.text_buffer
            .as_ref()
            .map(|buffer| buffer.materialize().into_bytes())
            .unwrap_or_else(|| tab.editor_text.as_bytes().to_vec())
    } else {
        tab.byte_doc
            .as_ref()
            .map(|document| document.as_cow().into_owned())
            .ok_or_else(|| "RTON tab has no byte surface".to_string())?
    };
    Ok(WorkerBatchSource::Bytes { bytes, format })
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn export_tab_in_web_worker(
    tab: EditorTabState,
    mode: BatchExportMode,
    options: EncodeOptions,
) -> Result<Vec<u8>, String> {
    let source = worker_batch_source_from_tab(tab)?;
    let response = run_batch_worker(WorkerBatchRequest {
        jobs: vec![WorkerBatchJob { index: 0, source }],
        mode,
        encode_options: options,
    })
    .await?;
    let item = response
        .results
        .into_iter()
        .find(|item| item.index == 0)
        .ok_or_else(|| "Worker did not return an export result".to_string())?;
    item.error.map_or(Ok(item.bytes), Err)
}

#[cfg(target_arch = "wasm32")]
async fn worker_batch_source_from_file(file: LoadedFileState) -> Result<WorkerBatchSource, String> {
    let format = match SourceFormat::from_file_name(&file.display_name) {
        SourceFormat::Json => Some(rton_editor_core::TextFormat::Json),
        SourceFormat::Yaml => Some(rton_editor_core::TextFormat::Yaml),
        SourceFormat::Toml => Some(rton_editor_core::TextFormat::Toml),
        SourceFormat::Rton | SourceFormat::Unknown => None,
    };
    let bytes = match file.source {
        LoadedFileSource::Bytes(bytes) => bytes.to_vec(),
        LoadedFileSource::WebFile(file) => crate::file_import::read_web_file_bytes(&file).await?,
        LoadedFileSource::NativePath(_) => {
            return Err("Native file paths are unavailable in the web build".to_string());
        }
    };
    Ok(WorkerBatchSource::Bytes { bytes, format })
}

#[cfg(not(target_arch = "wasm32"))]
fn batch_export_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .saturating_sub(1)
        .max(1)
}

fn set_batch_progress_status(
    mut status: Signal<Status>,
    i18n: I18n,
    mode: BatchExportMode,
    completed: usize,
    total: usize,
) {
    status.set(Status::new(
        i18n.t_args(
            "status-batch-progress",
            &[
                ("format", mode.label().to_string()),
                ("completed", completed.to_string()),
                ("total", total.to_string()),
            ],
        ),
        Tone::Warn,
    ));
}
