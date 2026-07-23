use dioxus_html::FileData;
#[cfg(not(target_arch = "wasm32"))]
use rton_editor_core::{DecodedDocument, decode_rton_reader, parse_text};
use rton_editor_core::{SourceFormat, TextFormat};
#[cfg(target_arch = "wasm32")]
use rton_editor_core::{WorkerOpenTextRequest, WorkerOpenTextResponse, WorkerSurface};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use crate::domain::text_surface_from_arc;
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::value_tree_rows_for_doc;
use crate::domain::{
    ByteDocument, EditorTabState, OpenTabError, create_tab_from_byte_document,
    create_text_tab_from_surface,
};
#[cfg(target_arch = "wasm32")]
use crate::domain::{TextBuffer, TextContentState};
use crate::platform;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::run_cpu_task;
#[cfg(target_arch = "wasm32")]
use crate::platform::{run_open_text_file_worker, run_open_text_worker};

use super::state::{LoadedFileDraft, LoadedFileSource, LoadedFileState};

pub(crate) fn loaded_file_draft_from_native(file: platform::NativeOpenFile) -> LoadedFileDraft {
    loaded_file_draft_from_native_path(file.display_name, file.path)
}

pub(crate) fn loaded_file_drafts_from_native(
    files: Vec<platform::NativeOpenFile>,
) -> Vec<LoadedFileDraft> {
    #[cfg(not(target_arch = "wasm32"))]
    if files.len() < 512 {
        return files
            .into_iter()
            .map(loaded_file_draft_from_native)
            .collect();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        files
            .into_par_iter()
            .map(loaded_file_draft_from_native)
            .collect()
    }

    #[cfg(target_arch = "wasm32")]
    {
        files
            .into_iter()
            .map(loaded_file_draft_from_native)
            .collect()
    }
}

fn loaded_file_draft_from_native_path(display_name: String, path: PathBuf) -> LoadedFileDraft {
    let size = std::fs::metadata(&path)
        .ok()
        .and_then(|metadata| usize::try_from(metadata.len()).ok());
    LoadedFileDraft {
        display_name,
        source: LoadedFileSource::NativePath(path),
        size,
    }
}

pub(crate) async fn loaded_file_draft_from_file_data(
    display_name: String,
    file: FileData,
) -> Result<LoadedFileDraft, String> {
    if let Some(path) = file_data_real_path(&file) {
        return Ok(loaded_file_draft_from_native_path(display_name, path));
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(web_file) = super::web_drop::file_data_web_file(&file) {
        return Ok(LoadedFileDraft {
            display_name,
            size: super::web_drop::web_file_size(&web_file),
            source: LoadedFileSource::WebFile(web_file),
        });
    }

    let bytes = file.read_bytes().await.map_err(|error| error.to_string())?;
    let size = bytes.len();
    Ok(LoadedFileDraft {
        display_name,
        source: LoadedFileSource::Bytes(Arc::<[u8]>::from(bytes.as_ref())),
        size: Some(size),
    })
}

pub(crate) async fn create_tab_from_loaded_file(
    id: usize,
    file: &LoadedFileState,
) -> Result<EditorTabState, OpenTabError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = file.clone();
        run_cpu_task(move || create_tab_from_loaded_file_sync(id, &file)).await
    }

    #[cfg(target_arch = "wasm32")]
    match &file.source {
        LoadedFileSource::NativePath(path) => {
            create_tab_from_file_path(id, file.display_name.clone(), path)
        }
        LoadedFileSource::Bytes(bytes) => {
            create_tab_from_loaded_bytes(id, file.display_name.clone(), bytes.to_vec()).await
        }
        #[cfg(target_arch = "wasm32")]
        LoadedFileSource::WebFile(web_file) => {
            if let Some(format) = text_format_for_file_name(&file.display_name) {
                let response = run_open_text_file_worker(web_file.clone(), format, String::new())
                    .await
                    .map_err(OpenTabError::Read)?;
                worker_open_text_response_to_tab(id, file.display_name.clone(), response, format)
            } else {
                let bytes = super::web_drop::read_web_file_bytes(web_file)
                    .await
                    .map_err(OpenTabError::Read)?;
                create_tab_from_loaded_bytes(id, file.display_name.clone(), bytes).await
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn create_tab_from_loaded_file_sync(
    id: usize,
    file: &LoadedFileState,
) -> Result<EditorTabState, OpenTabError> {
    match &file.source {
        LoadedFileSource::NativePath(path) => {
            create_tab_from_file_path(id, file.display_name.clone(), path)
        }
        LoadedFileSource::Bytes(bytes) => {
            create_tab_from_loaded_bytes_sync(id, file.display_name.clone(), bytes.as_ref())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn document_from_loaded_file_sync(
    file: &LoadedFileState,
) -> Result<Arc<DecodedDocument>, OpenTabError> {
    match &file.source {
        LoadedFileSource::NativePath(path) => {
            document_from_file_path(&file.display_name, path).map(Arc::new)
        }
        LoadedFileSource::Bytes(bytes) => {
            document_from_loaded_bytes_sync(&file.display_name, bytes.as_ref()).map(Arc::new)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn document_from_file_path(
    display_name: &str,
    path: &Path,
) -> Result<DecodedDocument, OpenTabError> {
    let byte_doc = ByteDocument::from_file_path(path).map_err(OpenTabError::Read)?;
    if let Some(format) = text_format_for_file_name(display_name) {
        let bytes = byte_doc.as_cow();
        let text = String::from_utf8_lossy(bytes.as_ref());
        return parse_text(text.as_ref(), format)
            .map_err(|error| OpenTabError::Decode(error.to_string()));
    }

    decode_rton_reader(byte_doc.reader()).map_err(|error| OpenTabError::Decode(error.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
fn document_from_loaded_bytes_sync(
    display_name: &str,
    bytes: &[u8],
) -> Result<DecodedDocument, OpenTabError> {
    if let Some(format) = text_format_for_file_name(display_name) {
        let text = String::from_utf8_lossy(bytes);
        return parse_text(text.as_ref(), format)
            .map_err(|error| OpenTabError::Decode(error.to_string()));
    }

    decode_rton_reader(std::io::Cursor::new(bytes))
        .map_err(|error| OpenTabError::Decode(error.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
fn create_tab_from_loaded_bytes_sync(
    id: usize,
    display_name: String,
    bytes: &[u8],
) -> Result<EditorTabState, OpenTabError> {
    if let Some(format) = text_format_for_file_name(&display_name) {
        let text = String::from_utf8_lossy(bytes).to_string();
        return Ok(create_text_tab_from_text_parallel(
            id,
            display_name,
            text,
            format,
        ));
    }

    create_tab_from_byte_document(id, display_name, ByteDocument::from_vec(bytes.to_vec()))
        .map_err(|error| OpenTabError::Decode(error.to_string()))
}

#[cfg(target_arch = "wasm32")]
async fn create_tab_from_loaded_bytes(
    id: usize,
    display_name: String,
    bytes: Vec<u8>,
) -> Result<EditorTabState, OpenTabError> {
    if let Some(format) = text_format_for_file_name(&display_name) {
        let response = run_open_text_worker(WorkerOpenTextRequest {
            bytes,
            format,
            search_query: String::new(),
        })
        .await
        .map_err(OpenTabError::Read)?;
        return worker_open_text_response_to_tab(id, display_name, response, format);
    }

    create_tab_from_byte_document(id, display_name, ByteDocument::from_vec(bytes))
        .map_err(|error| OpenTabError::Decode(error.to_string()))
}

fn create_tab_from_file_path(
    id: usize,
    name: String,
    path: &Path,
) -> Result<EditorTabState, OpenTabError> {
    let byte_doc = ByteDocument::from_file_path(path).map_err(OpenTabError::Read)?;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(format) = text_format_for_file_name(&name) {
        let text = String::from_utf8_lossy(byte_doc.as_cow().as_ref()).to_string();
        return Ok(create_text_tab_from_text_parallel(id, name, text, format));
    }

    create_tab_from_byte_document(id, name, byte_doc)
        .map_err(|error| OpenTabError::Decode(error.to_string()))
}

fn text_format_for_file_name(file_name: &str) -> Option<TextFormat> {
    match SourceFormat::from_file_name(file_name) {
        SourceFormat::Json => Some(TextFormat::Json),
        SourceFormat::Yaml => Some(TextFormat::Yaml),
        SourceFormat::Toml => Some(TextFormat::Toml),
        SourceFormat::Rton | SourceFormat::Unknown => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn create_text_tab_from_text_parallel(
    id: usize,
    display_name: String,
    text: String,
    format: TextFormat,
) -> EditorTabState {
    let text = Arc::<str>::from(text);
    let (surface, doc) = std::thread::scope(|scope| {
        let parse_text_source = text.clone();
        let doc = scope.spawn(move || parse_text(parse_text_source.as_ref(), format).ok());
        let surface = text_surface_from_arc(text, format);
        (surface, doc.join().unwrap_or(None))
    });
    text_tab_with_optional_document(id, display_name, surface, format, doc)
}

#[cfg(not(target_arch = "wasm32"))]
fn text_tab_with_optional_document(
    id: usize,
    display_name: String,
    surface: crate::domain::editor_tab::TabSurface,
    format: TextFormat,
    doc: Option<DecodedDocument>,
) -> EditorTabState {
    let mut tab = create_text_tab_from_surface(id, display_name, surface, format);
    if let Some(doc) = doc {
        let doc = Arc::new(doc);
        tab.tree_rows = value_tree_rows_for_doc(&doc);
        tab.stats = Some(doc.stats.clone());
        tab.doc = Some(doc);
    }
    tab
}

#[cfg(target_arch = "wasm32")]
fn worker_open_text_response_to_tab(
    id: usize,
    display_name: String,
    response: WorkerOpenTextResponse,
    expected_format: TextFormat,
) -> Result<EditorTabState, OpenTabError> {
    let surface = worker_surface_to_tab_surface(response.surface, expected_format)?;
    let mut tab = create_text_tab_from_surface(id, display_name, surface, expected_format);
    tab.worker_document_id = response.worker_document_id;
    tab.worker_surface_mode = response.worker_document_id.map(|_| tab.mode);
    if let Some(stats) = response.stats {
        tab.stats = Some(stats);
        tab.tree_rows = Arc::new(response.tree_rows);
        tab.search_result = response.search_result.map(Arc::new);
        tab.search_query = response.search_query;
    }
    Ok(tab)
}

#[cfg(target_arch = "wasm32")]
fn worker_surface_to_tab_surface(
    surface: WorkerSurface,
    expected_format: TextFormat,
) -> Result<crate::domain::editor_tab::TabSurface, OpenTabError> {
    let WorkerSurface::Text {
        text,
        byte_count,
        line_count,
        format,
    } = surface
    else {
        return Err(OpenTabError::Decode(
            "Worker returned non-text surface".to_string(),
        ));
    };
    if format != expected_format {
        return Err(OpenTabError::Decode(
            "Worker returned wrong text format".to_string(),
        ));
    }

    let text_buffer = Arc::new(TextBuffer::new(text));
    Ok(crate::domain::editor_tab::TabSurface {
        byte_doc: None,
        editor_text: crate::domain::empty_editor_text(),
        text_buffer: Some(text_buffer),
        text_state: TextContentState::Text {
            byte_count,
            line_count,
            format,
        },
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn file_data_display_name(file: &FileData) -> String {
    use crate::domain::normalize_display_path;

    let path = normalize_display_path(&file.path().to_string_lossy());
    if path.is_empty() { file.name() } else { path }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn file_data_display_name(file: &FileData) -> String {
    file.name()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dropped_directory_files(
    file: &FileData,
) -> Result<Option<Vec<platform::NativeOpenFile>>, String> {
    let path = file.path();
    if path.is_dir() {
        platform::files_in_folder(&path).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn dropped_directory_files(
    _file: &FileData,
) -> Result<Option<Vec<platform::NativeOpenFile>>, String> {
    Ok(None)
}

#[cfg(not(target_arch = "wasm32"))]
fn file_data_real_path(file: &FileData) -> Option<PathBuf> {
    let path = file.path();
    if path.as_os_str().is_empty() || !path.is_file() {
        return None;
    }

    Some(path)
}

#[cfg(target_arch = "wasm32")]
fn file_data_real_path(_file: &FileData) -> Option<PathBuf> {
    None
}
