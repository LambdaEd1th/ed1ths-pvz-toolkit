use rton_editor_core::{
    BinaryEncoding, CoreError, DecodedDocument, ENCRYPTED_RTON_PREFIX, EncodeOptions, SourceFormat,
    TextFormat, TreeRows, ValueSearchResult, ValueStats, decrypt_rton_bytes_if_needed,
    detect_rton_binary_encoding,
};
#[cfg(any(not(target_arch = "wasm32"), test))]
use rton_editor_core::{
    decode_hex_rton, decode_rton_reader, encode_rton_bytes, flatten_expanded_value_tree,
    parse_text, search_value_tree, value_to_text,
};
use std::collections::HashSet;
use std::sync::Arc;

use super::{ByteDocument, EditorMode, HexHistory, TextBuffer, TextHistory};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EditorTabState {
    pub(crate) id: usize,
    pub(crate) content_revision: u64,
    pub(crate) file_name: String,
    pub(crate) doc: Option<Arc<DecodedDocument>>,
    pub(crate) stats: Option<ValueStats>,
    pub(crate) worker_document_id: Option<u64>,
    pub(crate) worker_surface_mode: Option<EditorMode>,
    pub(crate) byte_doc: Option<ByteDocument>,
    pub(crate) source_encode_options: EncodeOptions,
    pub(crate) tree_rows: Arc<TreeRows>,
    pub(crate) search_result: Option<Arc<ValueSearchResult>>,
    pub(crate) editor_text: Arc<str>,
    pub(crate) text_buffer: Option<Arc<TextBuffer>>,
    pub(crate) mode: EditorMode,
    pub(crate) search_query: String,
    pub(crate) selected_path: String,
    pub(crate) text_history: TextHistory,
    pub(crate) hex_history: HexHistory,
    pub(crate) text_state: TextContentState,
    pub(crate) task_state: Option<TabTaskState>,
    pub(crate) task_generation: u64,
    pub(crate) tree_generation: u64,
    pub(crate) search_generation: u64,
    pub(crate) expanded_paths: Arc<HashSet<String>>,
    pub(crate) text_cache: Vec<TextSurfaceCache>,
    pub(crate) rton_cache: Option<RtonSurfaceCache>,
    pub(crate) dirty: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TabSurface {
    pub(crate) byte_doc: Option<ByteDocument>,
    pub(crate) editor_text: Arc<str>,
    pub(crate) text_buffer: Option<Arc<TextBuffer>>,
    pub(crate) text_state: TextContentState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextContentState {
    None,
    Text {
        byte_count: usize,
        line_count: usize,
        format: TextFormat,
    },
}

impl TextContentState {
    pub(crate) fn text_format(&self) -> Option<TextFormat> {
        match self {
            Self::Text { format, .. } => Some(*format),
            Self::None => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabTaskState {
    pub(crate) id: u64,
    pub(crate) target_mode: EditorMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextSurfaceCache {
    pub(crate) mode: EditorMode,
    pub(crate) editor_text: Arc<str>,
    pub(crate) text_buffer: Option<Arc<TextBuffer>>,
    pub(crate) text_state: TextContentState,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RtonSurfaceCache {
    pub(crate) encode_options: EncodeOptions,
    pub(crate) byte_doc: ByteDocument,
}

#[cfg(test)]
pub(crate) fn create_tab_from_bytes(
    id: usize,
    name: String,
    bytes: &[u8],
) -> Result<EditorTabState, CoreError> {
    create_tab_from_byte_document(id, name, ByteDocument::from_vec(bytes.to_vec()))
}

pub(crate) fn create_tab_from_byte_document(
    id: usize,
    name: String,
    byte_doc: ByteDocument,
) -> Result<EditorTabState, CoreError> {
    match SourceFormat::from_file_name(&name) {
        SourceFormat::Rton | SourceFormat::Unknown => {
            let display_source = rton_hex_byte_document_for_display(byte_doc);
            Ok(EditorTabState {
                id,
                content_revision: 0,
                file_name: name,
                doc: None,
                stats: None,
                worker_document_id: None,
                worker_surface_mode: None,
                byte_doc: Some(display_source.byte_doc),
                source_encode_options: display_source.encode_options,
                tree_rows: empty_tree_rows(),
                search_result: None,
                editor_text: empty_editor_text(),
                text_buffer: None,
                mode: EditorMode::RtonHex,
                search_query: String::new(),
                selected_path: "$".to_string(),
                text_history: TextHistory::default(),
                hex_history: HexHistory::default(),
                text_state: TextContentState::None,
                task_state: None,
                task_generation: 0,
                tree_generation: 0,
                search_generation: 0,
                expanded_paths: default_expanded_paths(),
                text_cache: Vec::new(),
                rton_cache: None,
                dirty: false,
            })
        }
        SourceFormat::Json => create_text_tab(
            id,
            name,
            String::from_utf8_lossy(byte_doc.as_cow().as_ref()).to_string(),
            TextFormat::Json,
        ),
        SourceFormat::Yaml => create_text_tab(
            id,
            name,
            String::from_utf8_lossy(byte_doc.as_cow().as_ref()).to_string(),
            TextFormat::Yaml,
        ),
        SourceFormat::Toml => create_text_tab(
            id,
            name,
            String::from_utf8_lossy(byte_doc.as_cow().as_ref()).to_string(),
            TextFormat::Toml,
        ),
    }
}

struct RtonDisplaySource {
    byte_doc: ByteDocument,
    encode_options: EncodeOptions,
}

fn rton_hex_byte_document_for_display(byte_doc: ByteDocument) -> RtonDisplaySource {
    let source_encoding = rton_binary_encoding_from_byte_document(&byte_doc);
    let encrypted = ENCRYPTED_RTON_PREFIX
        .iter()
        .enumerate()
        .all(|(index, byte)| byte_doc.byte_at(index) == Some(*byte));
    if !encrypted {
        return RtonDisplaySource {
            byte_doc,
            encode_options: EncodeOptions {
                encoding: source_encoding,
                encrypted: false,
            },
        };
    }

    let decrypted = {
        let bytes = byte_doc.as_cow();
        decrypt_rton_bytes_if_needed(bytes.as_ref())
    };

    match decrypted {
        Ok(Some(bytes)) => {
            let encoding = detect_rton_binary_encoding(&bytes);
            RtonDisplaySource {
                byte_doc: ByteDocument::from_vec(bytes),
                encode_options: EncodeOptions {
                    encoding,
                    encrypted: true,
                },
            }
        }
        Ok(None) | Err(_) => RtonDisplaySource {
            byte_doc,
            encode_options: EncodeOptions {
                encoding: BinaryEncoding::Standard,
                encrypted: true,
            },
        },
    }
}

fn rton_binary_encoding_from_byte_document(byte_doc: &ByteDocument) -> BinaryEncoding {
    let mut header = [0_u8; 8];
    if byte_doc.copy_range_to(0, &mut header) == Some(header.len()) {
        detect_rton_binary_encoding(&header)
    } else {
        BinaryEncoding::Standard
    }
}

pub(crate) fn create_text_tab(
    id: usize,
    file_name: String,
    text: String,
    format: TextFormat,
) -> Result<EditorTabState, CoreError> {
    let surface = text_surface_from_text(text, format);
    Ok(create_text_tab_from_surface(id, file_name, surface, format))
}

pub(crate) fn create_text_tab_from_surface(
    id: usize,
    file_name: String,
    surface: TabSurface,
    format: TextFormat,
) -> EditorTabState {
    let mode = match format {
        TextFormat::Json => EditorMode::Json,
        TextFormat::Yaml => EditorMode::Yaml,
        TextFormat::Toml => EditorMode::Toml,
    };
    EditorTabState {
        id,
        content_revision: 0,
        file_name,
        tree_rows: empty_tree_rows(),
        doc: None,
        stats: None,
        worker_document_id: None,
        worker_surface_mode: None,
        byte_doc: surface.byte_doc,
        source_encode_options: EncodeOptions::default(),
        search_result: None,
        editor_text: surface.editor_text,
        text_buffer: surface.text_buffer,
        mode,
        search_query: String::new(),
        selected_path: "$".to_string(),
        text_history: TextHistory::default(),
        hex_history: HexHistory::default(),
        text_state: surface.text_state,
        task_state: None,
        task_generation: 0,
        tree_generation: 0,
        search_generation: 0,
        expanded_paths: default_expanded_paths(),
        text_cache: Vec::new(),
        rton_cache: None,
        dirty: false,
    }
}

pub(crate) fn empty_tree_rows() -> Arc<TreeRows> {
    Arc::new(TreeRows {
        rows: Vec::new(),
        truncated: false,
    })
}

pub(crate) fn empty_editor_text() -> Arc<str> {
    Arc::<str>::from("")
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn value_tree_rows_for_doc(doc: &DecodedDocument) -> Arc<TreeRows> {
    Arc::new(flatten_expanded_value_tree(
        &doc.value,
        default_expanded_paths().as_ref(),
        usize::MAX,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn value_tree_rows_for_doc_with_expansion(
    doc: &DecodedDocument,
    expanded_paths: &HashSet<String>,
) -> Arc<TreeRows> {
    Arc::new(flatten_expanded_value_tree(
        &doc.value,
        expanded_paths,
        usize::MAX,
    ))
}

pub(crate) fn default_expanded_paths() -> Arc<HashSet<String>> {
    Arc::new(HashSet::from(["$".to_string()]))
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn value_search_result_for_doc(
    doc: &DecodedDocument,
    query: &str,
) -> Option<Arc<ValueSearchResult>> {
    if query.trim().is_empty() {
        None
    } else {
        Some(Arc::new(search_value_tree(&doc.value, query, usize::MAX)))
    }
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn parse_editor_text(
    text: &str,
    mode: EditorMode,
) -> Result<DecodedDocument, CoreError> {
    match mode {
        EditorMode::RtonHex => decode_hex_rton(text),
        EditorMode::Json => parse_text(text, TextFormat::Json),
        EditorMode::Yaml => parse_text(text, TextFormat::Yaml),
        EditorMode::Toml => parse_text(text, TextFormat::Toml),
    }
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn document_for_tab(tab: &EditorTabState) -> Result<Arc<DecodedDocument>, CoreError> {
    match tab.mode {
        EditorMode::RtonHex => tab
            .byte_doc
            .as_ref()
            .map(|bytes| decode_rton_reader(bytes.reader()).map(Arc::new))
            .unwrap_or_else(|| decode_hex_rton(tab.editor_text.as_ref()).map(Arc::new)),
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            if let Some(buffer) = tab.text_buffer.as_ref() {
                parse_editor_text(&buffer.materialize(), tab.mode).map(Arc::new)
            } else {
                parse_editor_text(tab.editor_text.as_ref(), tab.mode).map(Arc::new)
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn document_for_owned_tab(
    tab: EditorTabState,
) -> Result<Arc<DecodedDocument>, CoreError> {
    if !tab.dirty
        && let Some(doc) = tab.doc
    {
        return Ok(doc);
    }

    document_for_tab(&tab)
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn tab_surface_for_document(
    doc: &DecodedDocument,
    mode: EditorMode,
    encode_options: EncodeOptions,
) -> Result<TabSurface, CoreError> {
    match mode {
        EditorMode::RtonHex => {
            let bytes = encode_rton_bytes(&doc.value, encode_options)?;
            Ok(TabSurface {
                byte_doc: Some(ByteDocument::from_vec(bytes)),
                editor_text: empty_editor_text(),
                text_buffer: None,
                text_state: TextContentState::None,
            })
        }
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            text_surface_for_document(doc, mode)
        }
    }
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn text_surface_for_document(
    doc: &DecodedDocument,
    mode: EditorMode,
) -> Result<TabSurface, CoreError> {
    let format = mode.text_format().expect("text mode has format");
    let editor_text = value_to_text(&doc.value, format)?;
    Ok(text_surface_from_text(editor_text, format))
}

pub(crate) fn text_surface_from_text(editor_text: String, format: TextFormat) -> TabSurface {
    let text_buffer = Arc::new(TextBuffer::new(editor_text));
    text_surface_from_buffer(text_buffer, format)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn text_surface_from_arc(editor_text: Arc<str>, format: TextFormat) -> TabSurface {
    let text_buffer = Arc::new(TextBuffer::from_arc(editor_text));
    text_surface_from_buffer(text_buffer, format)
}

fn text_surface_from_buffer(text_buffer: Arc<TextBuffer>, format: TextFormat) -> TabSurface {
    let byte_count = text_buffer.byte_count();
    let line_count = text_buffer.line_count();
    TabSurface {
        byte_doc: None,
        editor_text: empty_editor_text(),
        text_buffer: Some(text_buffer),
        text_state: TextContentState::Text {
            byte_count,
            line_count,
            format,
        },
    }
}
