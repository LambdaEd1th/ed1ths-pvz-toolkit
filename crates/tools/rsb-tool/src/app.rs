use crate::domain::{
    ArchiveChannelOrderMode, ArchiveDocument, BrowserItem, BrowserLocation, PacketDocument,
    RowSelection, file_kind, format_bytes, safe_archive_path,
};
use crate::editing::{self, AddedFile, PacketEdits, RemovedPackets};
use crate::preview::{
    ArchiveChannelOrderDetection, ArchiveChannelOrderInference, PreviewCache, PreviewQuality,
    PtxPreview, PtxPreviewState, decode_prepared, detect_archive_channel_order, prepare_png_export,
    prepare_preview,
};
use crate::view_model::{
    ArchiveRowFilter, ArchiveRowSource, ArchiveTableRow, PacketItemSource, PacketTreeSource,
};
use crate::virtual_scroll::{
    TABLE_DEFAULT_VIEWPORT_HEIGHT, measured_table_viewport_height, table_row_top,
    table_virtual_window,
};
use crate::{RsbRtonOpenRequest, loader, platform, processing};
use dioxus::prelude::*;
use dioxus_html::{
    FileData, HasFileData, ScrollBehavior, geometry::PixelsVector2D, input_data::MouseButton,
};
use image::ImageReader;
use rsb_archive::{
    ChannelOrder, Part1Extra, PtxDescriptor, PtxFormatCode, PtxPayload, PtxRsbMetadata, RsbPtxInfo,
    UnpackedFile,
};
use rsb_preview_worker::{TextureEncodeRequest, TextureEncoding};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
#[cfg(target_arch = "wasm32")]
use std::io::Write;
use std::sync::Arc;
use toolkit_ui::{ContextSheet, ToolPage, ToolPageToolbar, WorkspaceCard, push_application_log};

const RSB_PAGE_CSS: Asset = asset!("/assets/rsb/page.css");
const RSB_PREVIEW_POINTER_CAPTURE: &str = r#"
const canvas = document.querySelector(".rsb-preview-modal-canvas");
if (canvas && canvas.dataset.pointerCaptureReady !== "true") {
    canvas.dataset.pointerCaptureReady = "true";
    canvas.addEventListener("pointerdown", (event) => {
        if (event.button === 0 && !event.target.closest(".rsb-preview-view-controls")) {
            canvas.setPointerCapture?.(event.pointerId);
        }
    });
    const release = (event) => {
        if (canvas.hasPointerCapture?.(event.pointerId)) {
            canvas.releasePointerCapture(event.pointerId);
        }
    };
    canvas.addEventListener("pointerup", release);
    canvas.addEventListener("pointercancel", release);
}
"#;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StatusTone {
    #[default]
    Neutral,
    Success,
    Warning,
    Error,
}

impl StatusTone {
    const fn class(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AppStatus {
    message: String,
    tone: StatusTone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EditDialog {
    CreateRsg(NewRsgDraft),
    CreateFolder(NewFolderDraft),
    ImportFiles(FileImportDraft),
    AddTexture(NewTextureDraft),
    Rename { target: RowSelection, value: String },
    Delete { target: RowSelection, label: String },
    Properties(PropertiesDraft),
}

impl EditDialog {
    const fn key(&self) -> &'static str {
        match self {
            Self::CreateRsg(_) => "create-rsg",
            Self::CreateFolder(_) => "create-folder",
            Self::ImportFiles(_) => "import-files",
            Self::AddTexture(_) => "add-texture",
            Self::Rename { .. } => "rename",
            Self::Delete { .. } => "delete",
            Self::Properties(_) => "properties",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NewRsgDraft {
    name: String,
    compression_flags: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NewFolderDraft {
    parent: Vec<String>,
    name: String,
}

type VirtualDirectories = BTreeMap<usize, BTreeSet<Vec<String>>>;

fn replace_virtual_directories(
    directories: &mut VirtualDirectories,
    packet_index: usize,
    paths: BTreeSet<Vec<String>>,
) {
    if paths.is_empty() {
        directories.remove(&packet_index);
    } else {
        directories.insert(packet_index, paths);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileImportDraft {
    files: Vec<AddedFile>,
    directory: Vec<String>,
    mode: FileImportMode,
}

impl FileImportDraft {
    fn single_kind(&self) -> Option<AddedFileKind> {
        (self.files.len() == 1).then(|| added_file_kind(&self.files[0].name))
    }

    fn texture_mode(&self) -> Option<FileImportMode> {
        match self.single_kind()? {
            AddedFileKind::EncodedPtx => Some(FileImportMode::AddPtx),
            AddedFileKind::RasterImage => Some(FileImportMode::EncodePtx),
            AddedFileKind::Ordinary => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FileImportMode {
    #[default]
    Direct,
    AddPtx,
    EncodePtx,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NewTextureDraft {
    file: AddedFile,
    source: TextureSourceKind,
    encoding: TextureEncoding,
    path: String,
    width: String,
    height: String,
    format: String,
    pitch: String,
    alpha_size: String,
    alpha_format: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextureSourceKind {
    EncodedPtx,
    RasterImage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddedFileKind {
    Ordinary,
    EncodedPtx,
    RasterImage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PropertiesTarget {
    Packet(usize),
    File(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PropertiesDraft {
    target: PropertiesTarget,
    name_or_path: String,
    compression_flags: String,
    width: String,
    height: String,
    format: String,
    pitch: String,
    alpha_size: String,
    alpha_format: String,
}

#[derive(Clone, Debug, PartialEq)]
struct ArchiveContextMenuState {
    target: Option<RowSelection>,
    directory: Vec<String>,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PtxPreviewContextMenuState {
    preview: PtxPreview,
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ArchiveContextCapabilities {
    open: bool,
    add: bool,
    replace: bool,
    rename: bool,
    delete: bool,
    properties: bool,
    extract: bool,
    export_png: bool,
}

impl AppStatus {
    fn new(message: impl Into<String>, tone: StatusTone) -> Self {
        Self {
            message: message.into(),
            tone,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NavigationSnapshot {
    location: BrowserLocation,
    selection: Option<RowSelection>,
    query: String,
    scroll_top: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ScrollRestore {
    top: f64,
    revision: u64,
}

#[derive(Clone, Copy)]
struct NavigationSignals {
    location: Signal<BrowserLocation>,
    selection: Signal<Option<RowSelection>>,
    query: Signal<String>,
    table_scroll_top: Signal<f64>,
    history: Signal<Vec<NavigationSnapshot>>,
    scroll_restore: Signal<ScrollRestore>,
}

const NAVIGATION_HISTORY_LIMIT: usize = 128;

impl NavigationSignals {
    fn snapshot(self) -> NavigationSnapshot {
        NavigationSnapshot {
            location: (self.location)(),
            selection: (self.selection)(),
            query: (self.query)(),
            scroll_top: *self.table_scroll_top.peek(),
        }
    }

    fn request_scroll(mut self, top: f64) {
        let top = if top.is_finite() && top > 0.0 {
            top
        } else {
            0.0
        };
        let revision = (self.scroll_restore)().revision.wrapping_add(1);
        self.table_scroll_top.set(top);
        self.scroll_restore.set(ScrollRestore { top, revision });
    }

    fn push_history(mut self, snapshot: NavigationSnapshot) {
        let mut history = self.history.write();
        if history.len() >= NAVIGATION_HISTORY_LIMIT {
            let remove_count = history
                .len()
                .saturating_add(1)
                .saturating_sub(NAVIGATION_HISTORY_LIMIT);
            history.drain(..remove_count);
        }
        history.push(snapshot);
    }
}

fn set_status(mut status: Signal<AppStatus>, value: AppStatus) {
    let level = match value.tone {
        StatusTone::Neutral => "INFO",
        StatusTone::Success => "INFO",
        StatusTone::Warning => "WARN",
        StatusTone::Error => "ERROR",
    };
    push_application_log("RSB", level, "ARCHIVE", &value.message);
    status.set(value);
}

fn archive_file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn is_rton_file(path: &str) -> bool {
    archive_file_name(path)
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("rton"))
}

fn archive_context_capabilities(
    target: &RowSelection,
    packet: Option<&PacketDocument>,
    packet_record: Option<&crate::domain::PacketRecord>,
    has_virtual_directory: bool,
) -> ArchiveContextCapabilities {
    match target {
        RowSelection::Packet(_) => ArchiveContextCapabilities {
            open: true,
            replace: true,
            rename: true,
            delete: packet_record.is_some(),
            properties: true,
            extract: true,
            ..Default::default()
        },
        RowSelection::Directory(path) => ArchiveContextCapabilities {
            open: true,
            add: true,
            rename: true,
            delete: has_virtual_directory
                || packet
                    .is_some_and(|packet| packet.directory_index.directory_summary(path).0 > 0),
            extract: true,
            ..Default::default()
        },
        RowSelection::File(index) => {
            let file = packet.and_then(|packet| packet.files.get(*index));
            ArchiveContextCapabilities {
                open: file.is_some_and(|file| {
                    is_rton_file(&file.path)
                        || (file.is_part1 && file.path.to_ascii_lowercase().ends_with(".ptx"))
                }),
                replace: file.is_some(),
                rename: file.is_some(),
                delete: file.is_some(),
                properties: file.is_some(),
                extract: file.is_some(),
                export_png: file.is_some_and(|file| file.is_part1 && is_ptx_file(&file.path)),
                ..Default::default()
            }
        }
    }
}

fn archive_context_open_label(
    target: &RowSelection,
    packet: Option<&PacketDocument>,
) -> &'static str {
    match target {
        RowSelection::Packet(_) | RowSelection::Directory(_) => "打开",
        RowSelection::File(index) => {
            packet
                .and_then(|packet| packet.files.get(*index))
                .map_or("打开", |file| {
                    if is_rton_file(&file.path) {
                        "在 RTON Editor 中打开"
                    } else if file.is_part1 && file.path.to_ascii_lowercase().ends_with(".ptx") {
                        "预览"
                    } else {
                        "打开"
                    }
                })
        }
    }
}

fn install_archive(
    document: ArchiveDocument,
    mut archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    mut location: Signal<BrowserLocation>,
    mut selection: Signal<Option<RowSelection>>,
    mut query: Signal<String>,
    status: Signal<AppStatus>,
) {
    let packet_count = document.packets.len();
    let warning_count = document.warnings.len();
    let name = document.display_name.clone();
    archive.set(Some(Arc::new(document)));
    packet.set(None);
    location.set(BrowserLocation::Archive);
    selection.set(None);
    query.set(String::new());
    set_status(
        status,
        AppStatus::new(
            if warning_count == 0 {
                format!("已打开 {name} · {packet_count} 个 RSG 包")
            } else {
                format!("已打开 {name} · {packet_count} 个 RSG 包 · {warning_count} 条警告")
            },
            if warning_count == 0 {
                StatusTone::Success
            } else {
                StatusTone::Warning
            },
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn install_editable_archive(
    document: ArchiveDocument,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    location: Signal<BrowserLocation>,
    selection: Signal<Option<RowSelection>>,
    query: Signal<String>,
    status: Signal<AppStatus>,
    mut edits: Signal<PacketEdits>,
    mut removed_packets: Signal<RemovedPackets>,
    mut ptx_infos: Signal<Vec<RsbPtxInfo>>,
) {
    ptx_infos.set(document.ptx_infos.as_ref().clone());
    edits.write().clear();
    removed_packets.write().clear();
    install_archive(
        document, archive, packet, location, selection, query, status,
    );
}

async fn open_file_data(file: FileData) -> Result<ArchiveDocument, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = file.path();
        if !path.as_os_str().is_empty() && path.is_file() {
            return processing::open_native(path).await;
        }
    }

    let name = file.name();
    let bytes = file.read_bytes().await.map_err(|error| error.to_string())?;
    loader::open_memory(name, bytes.as_ref().to_vec())
}

fn enter_packet(
    packet_index: usize,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    edits: Signal<PacketEdits>,
    mut navigation: NavigationSignals,
    status: Signal<AppStatus>,
) {
    let Some(document) = archive() else {
        return;
    };
    let snapshot = navigation.snapshot();
    if let Some(edited) = edits.read().get(&packet_index).cloned() {
        let loaded = edited.document;
        let file_count = loaded.files.len();
        let name = loaded.record.info.name.clone();
        navigation.push_history(snapshot);
        packet.set(Some(loaded));
        navigation.location.set(BrowserLocation::Packet {
            packet_index,
            directory: Vec::new(),
        });
        navigation.selection.set(None);
        navigation.query.set(String::new());
        navigation.request_scroll(0.0);
        set_status(
            status,
            AppStatus::new(
                format!("{name} · {file_count} 个文件 · 包含未保存修改"),
                StatusTone::Warning,
            ),
        );
        return;
    }
    processing::begin_request();
    set_status(
        status,
        AppStatus::new("正在读取并解压 RSG 包…", StatusTone::Neutral),
    );
    spawn(async move {
        match processing::load_packet(document, packet_index).await {
            Ok(loaded) => {
                let file_count = loaded.files.len();
                let name = loaded.record.info.name.clone();
                navigation.push_history(snapshot);
                packet.set(Some(Arc::new(loaded)));
                navigation.location.set(BrowserLocation::Packet {
                    packet_index,
                    directory: Vec::new(),
                });
                navigation.selection.set(None);
                navigation.query.set(String::new());
                navigation.request_scroll(0.0);
                set_status(
                    status,
                    AppStatus::new(
                        format!("{name} · 已验证并读取 {file_count} 个文件"),
                        StatusTone::Success,
                    ),
                );
            }
            Err(error) => set_status(
                status,
                AppStatus::new(format!("无法打开 RSG 包：{error}"), StatusTone::Error),
            ),
        }
    });
}

fn enter_directory(directory: Vec<String>, mut navigation: NavigationSignals) {
    let BrowserLocation::Packet {
        packet_index,
        directory: current_directory,
    } = (navigation.location)()
    else {
        return;
    };
    if directory == current_directory {
        return;
    }
    let is_direct_child =
        directory.len() == current_directory.len() + 1 && directory.starts_with(&current_directory);
    if is_direct_child {
        let snapshot = navigation.snapshot();
        navigation.push_history(snapshot);
    }
    navigation.location.set(BrowserLocation::Packet {
        packet_index,
        directory,
    });
    navigation.selection.set(None);
    navigation.query.set(String::new());
    navigation.request_scroll(0.0);
}

fn go_up(mut navigation: NavigationSignals) {
    let (parent, fallback_selection) = match (navigation.location)() {
        BrowserLocation::Archive => return,
        BrowserLocation::Packet {
            packet_index,
            mut directory,
        } => {
            if directory.is_empty() {
                (
                    BrowserLocation::Archive,
                    Some(RowSelection::Packet(packet_index)),
                )
            } else {
                let child = directory.clone();
                directory.pop();
                (
                    BrowserLocation::Packet {
                        packet_index,
                        directory,
                    },
                    Some(RowSelection::Directory(child)),
                )
            }
        }
    };

    let restored = {
        let mut history = navigation.history.write();
        let index = history
            .iter()
            .rposition(|snapshot| snapshot.location == parent);
        let snapshot = index.map(|index| history[index].clone());
        if let Some(index) = index {
            history.truncate(index);
        }
        snapshot
    };
    navigation.location.set(parent);
    if let Some(snapshot) = restored {
        navigation
            .selection
            .set(snapshot.selection.or(fallback_selection));
        navigation.query.set(snapshot.query);
        navigation.request_scroll(snapshot.scroll_top);
    } else {
        navigation.selection.set(fallback_selection);
        navigation.query.set(String::new());
        navigation.request_scroll(0.0);
    }
}

#[cfg(target_arch = "wasm32")]
fn build_zip(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (index, (path, bytes)) in entries.iter().enumerate() {
        let path = safe_archive_path(path, &format!("entry-{index}.bin"));
        writer
            .start_file(path, options)
            .map_err(|error| error.to_string())?;
        writer.write_all(bytes).map_err(|error| error.to_string())?;
    }
    writer
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|error| error.to_string())
}

fn packet_entries(
    packet: &PacketDocument,
    location: &BrowserLocation,
    selection: Option<&RowSelection>,
) -> Vec<(String, Vec<u8>)> {
    if let Some(RowSelection::File(index)) = selection {
        return packet
            .files
            .get(*index)
            .map(|file| {
                vec![(
                    safe_archive_path(&file.path, &format!("entry-{index}.bin")),
                    file.data.clone(),
                )]
            })
            .unwrap_or_default();
    }

    let directory = match selection {
        Some(RowSelection::Directory(path)) => path.clone(),
        _ => match location {
            BrowserLocation::Packet { directory, .. } => directory.clone(),
            BrowserLocation::Archive => Vec::new(),
        },
    };
    packet
        .directory_index
        .files_below(&packet.files, &directory)
        .into_iter()
        .map(|(index, file)| {
            (
                safe_archive_path(&file.path, &format!("entry-{index}.bin")),
                file.data.clone(),
            )
        })
        .collect()
}

async fn extract_current(
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    location: Signal<BrowserLocation>,
    selection: Signal<Option<RowSelection>>,
    edits: Signal<PacketEdits>,
    status: Signal<AppStatus>,
) {
    let selected = selection();
    if matches!(location(), BrowserLocation::Archive) {
        let Some(RowSelection::Packet(index)) = selected else {
            set_status(
                status,
                AppStatus::new("请先选择一个 RSG 包", StatusTone::Warning),
            );
            return;
        };
        let Some(document) = archive() else {
            return;
        };
        let loaded = edits
            .read()
            .get(&index)
            .map(|edit| edit.document.as_ref().clone())
            .map(Ok)
            .unwrap_or_else(|| document.load_packet(index));
        match loaded {
            Ok(loaded) => {
                let name = format!("{}.rsg", loaded.record.info.name);
                match platform::save_bytes(&name, &loaded.raw).await {
                    Ok(true) => set_status(
                        status,
                        AppStatus::new(format!("已导出 {name}"), StatusTone::Success),
                    ),
                    Ok(false) => {}
                    Err(error) => set_status(
                        status,
                        AppStatus::new(format!("导出失败：{error}"), StatusTone::Error),
                    ),
                }
            }
            Err(error) => set_status(
                status,
                AppStatus::new(format!("读取 RSG 失败：{error}"), StatusTone::Error),
            ),
        }
        return;
    }

    let Some(packet_document) = packet() else {
        return;
    };
    let entries = packet_entries(&packet_document, &location(), selected.as_ref());
    if entries.is_empty() {
        set_status(
            status,
            AppStatus::new("当前位置没有可提取的文件", StatusTone::Warning),
        );
        return;
    }

    if matches!(selected, Some(RowSelection::File(_))) {
        let (path, bytes) = &entries[0];
        let name = path.rsplit('/').next().unwrap_or("entry.bin");
        match platform::save_bytes(name, bytes).await {
            Ok(true) => set_status(
                status,
                AppStatus::new(format!("已导出 {name}"), StatusTone::Success),
            ),
            Ok(false) => {}
            Err(error) => set_status(
                status,
                AppStatus::new(format!("导出失败：{error}"), StatusTone::Error),
            ),
        }
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    match platform::extract_entries(&entries).await {
        Ok(Some(count)) => set_status(
            status,
            AppStatus::new(format!("已提取 {count} 个文件"), StatusTone::Success),
        ),
        Ok(None) => {}
        Err(error) => set_status(
            status,
            AppStatus::new(format!("提取失败：{error}"), StatusTone::Error),
        ),
    }

    #[cfg(target_arch = "wasm32")]
    let exported = match build_zip(&entries) {
        Ok(bytes) => {
            platform::save_bytes(&format!("{}.zip", packet_document.record.info.name), &bytes).await
        }
        Err(error) => Err(error),
    };
    #[cfg(target_arch = "wasm32")]
    match exported {
        Ok(true) => set_status(
            status,
            AppStatus::new(
                format!("已打包并导出 {} 个文件", entries.len()),
                StatusTone::Success,
            ),
        ),
        Ok(false) => {}
        Err(error) => set_status(
            status,
            AppStatus::new(format!("导出失败：{error}"), StatusTone::Error),
        ),
    }
}

async fn export_ptx_png(
    target: Option<RowSelection>,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    ptx_infos: Signal<Vec<RsbPtxInfo>>,
    status: Signal<AppStatus>,
) {
    let Some(RowSelection::File(file_index)) = target else {
        set_status(
            status,
            AppStatus::new("请先选择一个 PTX 纹理", StatusTone::Warning),
        );
        return;
    };
    let (Some(archive), Some(packet)) = (archive(), packet()) else {
        return;
    };
    let archive = archive_with_ptx_infos(archive, &ptx_infos.read());
    let prepared = match prepare_png_export(&archive, packet, file_index) {
        Ok(prepared) => prepared,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("无法导出 PNG：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    let output_name = prepared.output_name;
    set_status(
        status,
        AppStatus::new(format!("正在后台解码 {output_name}…"), StatusTone::Neutral),
    );
    let response =
        match processing::decode_png(prepared.packet, prepared.file_index, prepared.spec).await {
            Ok(response) => response,
            Err(error) => {
                set_status(
                    status,
                    AppStatus::new(format!("PNG 解码失败：{error}"), StatusTone::Error),
                );
                return;
            }
        };
    match platform::save_bytes(&output_name, &response.png).await {
        Ok(true) => set_status(
            status,
            AppStatus::new(
                format!(
                    "已导出 {output_name} · {} × {}",
                    response.width, response.height
                ),
                StatusTone::Success,
            ),
        ),
        Ok(false) => set_status(
            status,
            AppStatus::new("已取消导出 PNG", StatusTone::Neutral),
        ),
        Err(error) => set_status(
            status,
            AppStatus::new(format!("导出 PNG 失败：{error}"), StatusTone::Error),
        ),
    }
}

async fn copy_preview_png(preview: PtxPreview, status: Signal<AppStatus>) {
    let url = preview.asset.url();
    set_status(
        status,
        AppStatus::new(
            format!("正在复制 {} 的预览图像…", preview.name),
            StatusTone::Neutral,
        ),
    );
    match platform::copy_preview_image(&url).await {
        Ok(()) => set_status(
            status,
            AppStatus::new(
                format!(
                    "已复制 {} · {} × {} PNG",
                    preview.name, preview.rendered_width, preview.rendered_height
                ),
                StatusTone::Success,
            ),
        ),
        Err(error) => set_status(
            status,
            AppStatus::new(format!("复制图像失败：{error}"), StatusTone::Error),
        ),
    }
}

fn request_preview(
    archive: Arc<ArchiveDocument>,
    packet: Arc<PacketDocument>,
    file_index: usize,
    quality: PreviewQuality,
    mut preview: Signal<Option<PtxPreviewState>>,
    mut cache: Signal<PreviewCache>,
    status: Signal<AppStatus>,
) -> bool {
    let prepared = match prepare_preview(&archive, packet, file_index, quality) {
        Ok(prepared) => prepared,
        Err(error) => {
            preview.set(Some(PtxPreviewState::Error {
                file_index,
                quality,
                message: error.clone(),
            }));
            set_status(
                status,
                AppStatus::new(format!("PTX 预览失败：{error}"), StatusTone::Error),
            );
            return false;
        }
    };
    let generation = processing::begin_request();
    if let Some(cached) = cache.write().get(prepared.cache_key) {
        let message = format!(
            "{} · {} × {} · {} · 缓存",
            cached.name, cached.width, cached.height, cached.format
        );
        preview.set(Some(PtxPreviewState::Ready(cached)));
        set_status(status, AppStatus::new(message, StatusTone::Success));
        return true;
    }

    preview.set(Some(PtxPreviewState::Loading {
        file_index,
        quality,
    }));
    set_status(
        status,
        AppStatus::new(
            match quality {
                PreviewQuality::Thumbnail => "正在后台生成 PTX 缩略图…",
                PreviewQuality::Detail => "正在后台准备高分辨率 PTX 预览…",
            },
            StatusTone::Neutral,
        ),
    );
    spawn(async move {
        match decode_prepared(prepared, generation).await {
            Ok((key, decoded)) if processing::is_current(generation) => {
                let message = format!(
                    "{} · {} × {} · {}",
                    decoded.name, decoded.width, decoded.height, decoded.format
                );
                cache.write().insert(key, decoded.clone());
                preview.set(Some(PtxPreviewState::Ready(decoded)));
                set_status(status, AppStatus::new(message, StatusTone::Success));
            }
            Ok(_) => {}
            Err(_) if !processing::is_current(generation) => {}
            Err(error) => {
                preview.set(Some(PtxPreviewState::Error {
                    file_index,
                    quality,
                    message: error.clone(),
                }));
                set_status(
                    status,
                    AppStatus::new(format!("PTX 预览失败：{error}"), StatusTone::Error),
                );
            }
        }
    });
    true
}

#[derive(Clone, Copy)]
struct PreviewSignals {
    preview: Signal<Option<PtxPreviewState>>,
    detail: Signal<Option<PtxPreviewState>>,
    cache: Signal<PreviewCache>,
    status: Signal<AppStatus>,
}

fn select_item(
    next: RowSelection,
    archive: Option<Arc<ArchiveDocument>>,
    packet: Option<Arc<PacketDocument>>,
    mut selection: Signal<Option<RowSelection>>,
    mut signals: PreviewSignals,
) -> bool {
    selection.set(Some(next.clone()));
    signals.detail.set(None);
    let RowSelection::File(file_index) = next else {
        processing::begin_request();
        signals.preview.set(None);
        return false;
    };
    let Some(packet) = packet else {
        processing::begin_request();
        signals.preview.set(None);
        return false;
    };
    let Some(file) = packet.files.get(file_index) else {
        processing::begin_request();
        signals.preview.set(None);
        return false;
    };
    if !file.is_part1 || !file.path.to_ascii_lowercase().ends_with(".ptx") {
        processing::begin_request();
        signals.preview.set(None);
        return false;
    }
    let Some(archive) = archive else {
        processing::begin_request();
        signals.preview.set(None);
        return false;
    };

    request_preview(
        archive,
        packet,
        file_index,
        PreviewQuality::Thumbnail,
        signals.preview,
        signals.cache,
        signals.status,
    )
}

#[allow(clippy::too_many_arguments)]
fn open_file_item(
    index: usize,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    mut selection: Signal<Option<RowSelection>>,
    ptx_infos: Signal<Vec<RsbPtxInfo>>,
    mut ptx_preview: Signal<Option<PtxPreviewState>>,
    detail_preview: Signal<Option<PtxPreviewState>>,
    preview_cache: Signal<PreviewCache>,
    mut preview_open: Signal<bool>,
    status: Signal<AppStatus>,
    on_open_rton: Option<EventHandler<RsbRtonOpenRequest>>,
) {
    selection.set(Some(RowSelection::File(index)));
    ptx_preview.set(None);
    let (Some(archive), Some(packet)) = (archive(), packet()) else {
        return;
    };
    let archive = archive_with_ptx_infos(archive, &ptx_infos.read());
    let Some(file) = packet.files.get(index) else {
        return;
    };
    if is_rton_file(&file.path) {
        let Some(on_open_rton) = on_open_rton else {
            set_status(
                status,
                AppStatus::new("当前宿主没有可用的 RTON Editor", StatusTone::Warning),
            );
            return;
        };
        let name = archive_file_name(&file.path).to_string();
        on_open_rton.call(RsbRtonOpenRequest {
            name: name.clone(),
            bytes: Arc::from(file.data.clone()),
        });
        set_status(
            status,
            AppStatus::new(
                format!("已在 RTON Editor 中打开 {name}"),
                StatusTone::Success,
            ),
        );
        return;
    }
    if !file.is_part1 || !file.path.to_ascii_lowercase().ends_with(".ptx") {
        set_status(
            status,
            AppStatus::new("该文件没有内置预览，可使用提取或替换", StatusTone::Neutral),
        );
        return;
    }
    if request_preview(
        archive,
        packet,
        index,
        PreviewQuality::Detail,
        detail_preview,
        preview_cache,
        status,
    ) {
        preview_open.set(true);
    }
}

fn commit_packet_files(
    files: Vec<UnpackedFile>,
    name: String,
    compression_flags: u32,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    mut edits: Signal<PacketEdits>,
    status: Signal<AppStatus>,
) -> Result<(), String> {
    let current = packet().ok_or_else(|| "请先打开一个 RSG 包".to_string())?;
    let updated = editing::repack_packet(&current, name, compression_flags, files)?;
    let file_count = updated.files.len();
    let packet_name = updated.record.info.name.clone();
    let updated = editing::record_edit(&mut edits.write(), &current, updated);
    packet.set(Some(updated));
    set_status(
        status,
        AppStatus::new(
            format!("{packet_name} · 已修改 {file_count} 个文件 · 尚未保存"),
            StatusTone::Warning,
        ),
    );
    Ok(())
}

fn add_files_at(
    added: Vec<AddedFile>,
    directory: Vec<String>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    edits: Signal<PacketEdits>,
    status: Signal<AppStatus>,
) {
    if added.is_empty() {
        return;
    }
    let Some(current) = packet() else {
        return;
    };
    let mut files = current.files.as_ref().clone();
    let mut existing = files
        .iter()
        .map(|file| editing::normalize_archive_path(&file.path).to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();
    for added in added {
        let path = editing::join_archive_path(&directory, &added.name);
        if path.is_empty() {
            continue;
        }
        if !existing.insert(path.to_ascii_lowercase()) {
            set_status(
                status,
                AppStatus::new(format!("无法添加：{path} 已存在"), StatusTone::Error),
            );
            return;
        }
        files.push(UnpackedFile {
            path,
            data: added.data,
            is_part1: false,
            part1_info: None,
        });
    }
    let name = current.record.info.name.clone();
    let flags = current
        .record
        .header
        .as_ref()
        .map(|header| header.flags)
        .unwrap_or_default();
    if let Err(error) = commit_packet_files(files, name, flags, packet, edits, status) {
        set_status(
            status,
            AppStatus::new(format!("添加失败：{error}"), StatusTone::Error),
        );
    }
}

fn is_ptx_file(path: &str) -> bool {
    archive_file_name(path)
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("ptx"))
}

fn is_raster_image_file(path: &str) -> bool {
    archive_file_name(path)
        .rsplit_once('.')
        .is_some_and(|(_, extension)| {
            ["png", "webp", "jpg", "jpeg"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn added_file_kind(path: &str) -> AddedFileKind {
    if is_ptx_file(path) {
        AddedFileKind::EncodedPtx
    } else if is_raster_image_file(path) {
        AddedFileKind::RasterImage
    } else {
        AddedFileKind::Ordinary
    }
}

fn image_ptx_name(path: &str) -> String {
    let name = archive_file_name(path);
    name.rsplit_once('.')
        .map(|(stem, _)| format!("{stem}.ptx"))
        .unwrap_or_else(|| format!("{name}.ptx"))
}

fn file_import_dialog(files: Vec<AddedFile>, directory: Vec<String>) -> Option<EditDialog> {
    (!files.is_empty()).then(|| {
        EditDialog::ImportFiles(FileImportDraft {
            files,
            directory,
            mode: FileImportMode::default(),
        })
    })
}

fn raster_dimensions(data: &[u8]) -> Result<(u32, u32), String> {
    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|error| format!("无法识别图片格式：{error}"))?
        .into_dimensions()
        .map_err(|error| format!("无法读取图片尺寸：{error}"))
}

fn texture_encoding_for_template(
    archive: &ArchiveDocument,
    file: &UnpackedFile,
    info: &RsbPtxInfo,
) -> TextureEncoding {
    let Some(part1) = file.part1_info.as_ref() else {
        return TextureEncoding::Metadata;
    };
    let metadata = PtxRsbMetadata {
        format_code: PtxFormatCode::new(info.format),
        alpha_size: info.alpha_size.and_then(|value| u32::try_from(value).ok()),
        alpha_format: info.alpha_format,
        row_pitch: u32::try_from(info.pitch).ok().filter(|pitch| *pitch > 0),
        channel_order: if crate::preview::archive_uses_apple_channel_order(archive) {
            ChannelOrder::Bgra
        } else {
            ChannelOrder::Rgba
        },
    };
    match PtxDescriptor::from_rsb_payload(part1.width, part1.height, metadata, &file.data)
        .map(|descriptor| descriptor.format)
    {
        Ok(rsb_archive::PtxFormat::Etc1) => TextureEncoding::Etc1,
        Ok(rsb_archive::PtxFormat::Etc1A8) => TextureEncoding::Etc1A8,
        Ok(rsb_archive::PtxFormat::Etc1CompressedAlpha) => TextureEncoding::Etc1CompressedAlpha,
        Ok(rsb_archive::PtxFormat::Etc1Palette) => TextureEncoding::Etc1Palette,
        _ => TextureEncoding::Metadata,
    }
}

fn add_texture_dialog(
    added: AddedFile,
    directory: Vec<String>,
    archive: &ArchiveDocument,
    packet: &PacketDocument,
    ptx_infos: &[RsbPtxInfo],
) -> Result<EditDialog, String> {
    let source = if is_ptx_file(&added.name) {
        TextureSourceKind::EncodedPtx
    } else if is_raster_image_file(&added.name) {
        TextureSourceKind::RasterImage
    } else {
        return Err("新增纹理需要选择 PTX、PNG、WebP 或 JPEG 文件".into());
    };
    let image_dimensions = matches!(source, TextureSourceKind::RasterImage)
        .then(|| raster_dimensions(&added.data))
        .transpose()?;
    let archive_name = match source {
        TextureSourceKind::EncodedPtx => added.name.clone(),
        TextureSourceKind::RasterImage => image_ptx_name(&added.name),
    };
    let path = editing::join_archive_path(&directory, &archive_name);
    if path.is_empty() {
        return Err("纹理路径不能为空".into());
    }
    if packet
        .files
        .iter()
        .any(|file| editing::normalize_archive_path(&file.path).eq_ignore_ascii_case(&path))
    {
        return Err(format!("{path} 已存在"));
    }

    let template = packet
        .files
        .iter()
        .filter(|file| file.is_part1)
        .filter_map(|file| Some((file.part1_info.as_ref()?.id, file)))
        .max_by_key(|(id, _)| *id)
        .map(|(_, file)| file);
    let dimensions = template.and_then(|file| file.part1_info.as_ref());
    let metadata = template
        .and_then(|file| archive.texture_metadata(&packet.record, file))
        .and_then(|metadata| ptx_infos.get(metadata.global_index));
    let encoding = template
        .zip(metadata)
        .map(|(file, info)| texture_encoding_for_template(archive, file, info))
        .unwrap_or_default();
    let format = metadata.map(|value| value.format).unwrap_or(0);
    let pitch = match (source, image_dimensions) {
        (TextureSourceKind::RasterImage, Some((width, height))) => {
            let descriptor = PtxDescriptor::from_rsb(
                width,
                height,
                PtxRsbMetadata {
                    format_code: PtxFormatCode::new(format),
                    alpha_size: metadata
                        .and_then(|value| value.alpha_size)
                        .and_then(|value| u32::try_from(value).ok()),
                    alpha_format: metadata.and_then(|value| value.alpha_format),
                    row_pitch: None,
                    channel_order: ChannelOrder::Rgba,
                },
            );
            descriptor
                .ok()
                .and_then(|descriptor| descriptor.format.bytes_per_pixel())
                .and_then(|bytes_per_pixel| width.checked_mul(bytes_per_pixel))
                .and_then(|pitch| i32::try_from(pitch).ok())
                .unwrap_or(0)
        }
        _ => metadata.map(|value| value.pitch).unwrap_or(0),
    };

    Ok(EditDialog::AddTexture(NewTextureDraft {
        file: added,
        source,
        encoding,
        path,
        width: image_dimensions
            .map(|value| value.0.to_string())
            .or_else(|| dimensions.map(|value| value.width.to_string()))
            .unwrap_or_default(),
        height: image_dimensions
            .map(|value| value.1.to_string())
            .or_else(|| dimensions.map(|value| value.height.to_string()))
            .unwrap_or_default(),
        format: format.to_string(),
        pitch: pitch.to_string(),
        alpha_size: metadata
            .and_then(|value| value.alpha_size)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        alpha_format: metadata
            .and_then(|value| value.alpha_format)
            .map(|value| value.to_string())
            .unwrap_or_default(),
    }))
}

fn parse_positive_u32(value: &str, label: &str) -> Result<u32, String> {
    let parsed = value
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("{label}必须是正整数"))?;
    (parsed > 0)
        .then_some(parsed)
        .ok_or_else(|| format!("{label}必须大于 0"))
}

fn parse_optional_i32_checked(value: &str, label: &str) -> Result<Option<i32>, String> {
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse::<i32>()
            .map(Some)
            .map_err(|_| format!("{label}必须是整数或留空"))
    }
}

fn validate_ptx_payload(
    data: &[u8],
    width: u32,
    height: u32,
    info: &RsbPtxInfo,
) -> Result<(), String> {
    let metadata = PtxRsbMetadata {
        format_code: PtxFormatCode::new(info.format),
        alpha_size: info.alpha_size.and_then(|value| u32::try_from(value).ok()),
        alpha_format: info.alpha_format,
        row_pitch: u32::try_from(info.pitch).ok().filter(|pitch| *pitch > 0),
        channel_order: ChannelOrder::Rgba,
    };
    let descriptor = PtxDescriptor::from_rsb_payload(width, height, metadata, data)
        .map_err(|error| error.to_string())?;
    PtxPayload::parse(data, descriptor)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
async fn add_texture(
    draft: NewTextureDraft,
    mut archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    mut edits: Signal<PacketEdits>,
    mut ptx_infos: Signal<Vec<RsbPtxInfo>>,
    status: Signal<AppStatus>,
) {
    let NewTextureDraft {
        file,
        source,
        encoding,
        path,
        width,
        height,
        format,
        pitch,
        alpha_size,
        alpha_format,
    } = draft;
    let (Some(initial_document), Some(initial_packet)) = (archive(), packet()) else {
        return;
    };
    let path = editing::normalize_archive_path(&path);
    if path.is_empty() || !is_ptx_file(&path) {
        set_status(
            status,
            AppStatus::new("纹理路径必须以 .ptx 结尾", StatusTone::Error),
        );
        return;
    }
    if initial_packet
        .files
        .iter()
        .any(|file| editing::normalize_archive_path(&file.path).eq_ignore_ascii_case(&path))
    {
        set_status(
            status,
            AppStatus::new(format!("新增纹理失败：{path} 已存在"), StatusTone::Error),
        );
        return;
    }
    let width = match parse_positive_u32(&width, "宽度") {
        Ok(value) => value,
        Err(error) => {
            set_status(status, AppStatus::new(error, StatusTone::Error));
            return;
        }
    };
    let height = match parse_positive_u32(&height, "高度") {
        Ok(value) => value,
        Err(error) => {
            set_status(status, AppStatus::new(error, StatusTone::Error));
            return;
        }
    };
    let format = match format.trim().parse::<i32>() {
        Ok(value) => value,
        Err(_) => {
            set_status(
                status,
                AppStatus::new("格式代码必须是整数", StatusTone::Error),
            );
            return;
        }
    };
    let pitch = match pitch.trim().parse::<i32>() {
        Ok(value) => value,
        Err(_) => {
            set_status(
                status,
                AppStatus::new("Pitch 必须是整数", StatusTone::Error),
            );
            return;
        }
    };
    let alpha_size = match parse_optional_i32_checked(&alpha_size, "附加字节") {
        Ok(value) => value,
        Err(error) => {
            set_status(status, AppStatus::new(error, StatusTone::Error));
            return;
        }
    };
    let alpha_format = match parse_optional_i32_checked(&alpha_format, "缩放") {
        Ok(value) => value,
        Err(error) => {
            set_status(status, AppStatus::new(error, StatusTone::Error));
            return;
        }
    };
    let (Ok(width_i32), Ok(height_i32)) = (i32::try_from(width), i32::try_from(height)) else {
        set_status(
            status,
            AppStatus::new("纹理尺寸超过 RSB 可表示范围", StatusTone::Error),
        );
        return;
    };
    let mut info = RsbPtxInfo {
        ptx_index: 0,
        width: width_i32,
        height: height_i32,
        pitch,
        format,
        alpha_size,
        alpha_format,
    };
    let channel_order_archive = archive_with_ptx_infos(initial_document.clone(), &ptx_infos.read());
    let apple_channel_order =
        crate::preview::archive_uses_apple_channel_order(&channel_order_archive);
    let data = match source {
        TextureSourceKind::EncodedPtx => file.data,
        TextureSourceKind::RasterImage => {
            set_status(
                status,
                AppStatus::new(
                    format!("正在将 {} 编码为 PTX…", file.name),
                    StatusTone::Neutral,
                ),
            );
            let response = match processing::encode_texture(TextureEncodeRequest {
                source: file.data,
                width,
                height,
                format,
                alpha_size,
                alpha_format,
                pitch,
                apple_channel_order,
                encoding,
            })
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    set_status(
                        status,
                        AppStatus::new(format!("图片编码为 PTX 失败：{error}"), StatusTone::Error),
                    );
                    return;
                }
            };
            info.format = response.format;
            info.alpha_size = response.alpha_size;
            info.alpha_format = response.alpha_format;
            response.data
        }
    };
    let (Some(document), Some(current)) = (archive(), packet()) else {
        return;
    };
    if !Arc::ptr_eq(&initial_document, &document) || !Arc::ptr_eq(&initial_packet, &current) {
        set_status(
            status,
            AppStatus::new(
                "编码期间当前 RSB 或 RSG 已改变，请重新添加纹理",
                StatusTone::Warning,
            ),
        );
        return;
    }
    if let Err(error) = validate_ptx_payload(&data, width, height, &info) {
        set_status(
            status,
            AppStatus::new(format!("PTX 数据与属性不匹配：{error}"), StatusTone::Error),
        );
        return;
    }

    let local_id = current.record.info.ptx_number;
    let global_index = match usize::try_from(current.record.info.ptx_before_number)
        .ok()
        .zip(usize::try_from(local_id).ok())
        .and_then(|(begin, local_id)| begin.checked_add(local_id))
    {
        Some(value) => value,
        None => {
            set_status(
                status,
                AppStatus::new("新增纹理失败：PTX 序号溢出", StatusTone::Error),
            );
            return;
        }
    };
    let mut infos = ptx_infos.read().clone();
    if global_index > infos.len() {
        set_status(
            status,
            AppStatus::new("新增纹理失败：全局 PTX 表不连续", StatusTone::Error),
        );
        return;
    }
    info.ptx_index = i32::try_from(global_index).unwrap_or(i32::MAX);
    infos.insert(global_index, info);
    reindex_ptx_infos(&mut infos);

    let mut files = current.files.as_ref().clone();
    files.push(UnpackedFile {
        path: path.clone(),
        data,
        is_part1: true,
        part1_info: Some(Part1Extra {
            id: local_id,
            width,
            height,
        }),
    });
    let name = current.record.info.name.clone();
    let flags = current
        .record
        .header
        .as_ref()
        .map(|header| header.flags)
        .unwrap_or_default();
    let rebuilt = match editing::repack_packet(&current, name, flags, files) {
        Ok(value) => value,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("新增纹理失败：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    let mut edit_guard = edits.write();
    let rebuilt = editing::record_edit(&mut edit_guard, &current, rebuilt);
    let shifted = match editing::shift_texture_ranges_after(
        &document,
        &mut edit_guard,
        current.record.index,
        1,
    ) {
        Ok(value) => value,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("新增纹理失败：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    drop(edit_guard);
    ptx_infos.set(infos);
    archive.set(Some(Arc::new(shifted)));
    packet.set(Some(rebuilt));
    set_status(
        status,
        AppStatus::new(format!("已新增纹理 {path} · 尚未保存"), StatusTone::Warning),
    );
}

fn reindex_ptx_infos(infos: &mut [RsbPtxInfo]) {
    for (index, info) in infos.iter_mut().enumerate() {
        info.ptx_index = i32::try_from(index).unwrap_or(i32::MAX);
    }
}

fn open_file_import(
    files: Vec<AddedFile>,
    directory: Vec<String>,
    mut edit_dialog: Signal<Option<EditDialog>>,
) {
    if let Some(dialog) = file_import_dialog(files, directory) {
        edit_dialog.set(Some(dialog));
    }
}

#[allow(clippy::too_many_arguments)]
fn confirm_file_import(
    draft: FileImportDraft,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    edits: Signal<PacketEdits>,
    ptx_infos: Signal<Vec<RsbPtxInfo>>,
    mut edit_dialog: Signal<Option<EditDialog>>,
    status: Signal<AppStatus>,
) {
    match draft.mode {
        FileImportMode::Direct => {
            edit_dialog.set(None);
            add_files_at(draft.files, draft.directory, packet, edits, status);
        }
        FileImportMode::AddPtx | FileImportMode::EncodePtx => {
            if draft.texture_mode() != Some(draft.mode) {
                edit_dialog.set(None);
                set_status(
                    status,
                    AppStatus::new("所选文件不支持该纹理导入方式", StatusTone::Error),
                );
                return;
            }
            let file = draft
                .files
                .into_iter()
                .next()
                .expect("texture import mode requires one file");
            let (Some(document), Some(current)) = (archive(), packet()) else {
                edit_dialog.set(None);
                set_status(
                    status,
                    AppStatus::new("请先进入目标 RSG", StatusTone::Error),
                );
                return;
            };
            match add_texture_dialog(
                file,
                draft.directory,
                &document,
                &current,
                &ptx_infos.read(),
            ) {
                Ok(dialog) => edit_dialog.set(Some(dialog)),
                Err(error) => {
                    edit_dialog.set(None);
                    set_status(
                        status,
                        AppStatus::new(format!("准备纹理导入失败：{error}"), StatusTone::Error),
                    );
                }
            }
        }
    }
}

fn replace_selected_file(
    replacement: AddedFile,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    selection: Signal<Option<RowSelection>>,
    edits: Signal<PacketEdits>,
    ptx_infos: Signal<Vec<RsbPtxInfo>>,
    status: Signal<AppStatus>,
) {
    let (Some(current), Some(RowSelection::File(file_index))) = (packet(), selection()) else {
        return;
    };
    let Some(file) = current.files.get(file_index) else {
        return;
    };
    if file.is_part1 {
        let Some(document) = archive() else {
            return;
        };
        let Some(part1) = file.part1_info.as_ref() else {
            set_status(
                status,
                AppStatus::new("替换失败：纹理缺少 Part 1 属性", StatusTone::Error),
            );
            return;
        };
        let Some(metadata) = document.texture_metadata(&current.record, file) else {
            set_status(
                status,
                AppStatus::new("替换失败：找不到全局 PTX 属性", StatusTone::Error),
            );
            return;
        };
        let infos = ptx_infos.read();
        let Some(info) = infos.get(metadata.global_index) else {
            set_status(
                status,
                AppStatus::new("替换失败：PTX 序号超出全局表", StatusTone::Error),
            );
            return;
        };
        if let Err(error) = validate_ptx_payload(&replacement.data, part1.width, part1.height, info)
        {
            set_status(
                status,
                AppStatus::new(
                    format!("替换 PTX 与当前尺寸/格式不匹配：{error}"),
                    StatusTone::Error,
                ),
            );
            return;
        }
    }
    let mut files = current.files.as_ref().clone();
    files[file_index].data = replacement.data;
    let path = file.path.clone();
    let name = current.record.info.name.clone();
    let flags = current
        .record
        .header
        .as_ref()
        .map(|header| header.flags)
        .unwrap_or_default();
    match commit_packet_files(files, name, flags, packet, edits, status) {
        Ok(()) => set_status(
            status,
            AppStatus::new(format!("已替换 {path} · 尚未保存"), StatusTone::Warning),
        ),
        Err(error) => set_status(
            status,
            AppStatus::new(format!("替换失败：{error}"), StatusTone::Error),
        ),
    }
}

fn create_rsg_packet(
    draft: NewRsgDraft,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut edits: Signal<PacketEdits>,
    removed: Signal<RemovedPackets>,
    mut selection: Signal<Option<RowSelection>>,
    status: Signal<AppStatus>,
) {
    let Some(archive) = archive() else {
        return;
    };
    let packet_name = draft.name.trim();
    if packet_name.is_empty()
        || packet_name.len() >= 128
        || packet_name.contains('/')
        || packet_name.contains('\\')
    {
        set_status(
            status,
            AppStatus::new(
                "RSG 名称必须是少于 128 字节且不含路径分隔符的名称",
                StatusTone::Error,
            ),
        );
        return;
    }
    let Ok(compression_flags) = draft.compression_flags.parse::<u32>() else {
        set_status(
            status,
            AppStatus::new("空 RSG 只支持 Raw 或 Part 0 · zlib", StatusTone::Error),
        );
        return;
    };
    if !matches!(compression_flags, 0 | 2) {
        set_status(
            status,
            AppStatus::new("空 RSG 只支持 Raw 或 Part 0 · zlib", StatusTone::Error),
        );
        return;
    }
    if editing::visible_packet_records(&archive, &edits.read(), &removed.read())
        .iter()
        .any(|record| record.info.name.eq_ignore_ascii_case(packet_name))
    {
        set_status(
            status,
            AppStatus::new(
                format!("新增 RSG 失败：{packet_name} 已存在"),
                StatusTone::Error,
            ),
        );
        return;
    }

    let packet_index = editing::next_added_packet_index(archive.packets.len(), &edits.read());
    let version = archive
        .packets
        .iter()
        .filter_map(|record| record.header.as_ref())
        .map(|header| header.version)
        .find(|version| matches!(version, 3 | 4))
        .unwrap_or_else(|| match archive.header.version {
            3 | 4 => archive.header.version,
            _ => 4,
        });
    let mut document = match editing::create_empty_packet(
        packet_index,
        packet_name.to_string(),
        version,
        compression_flags,
    ) {
        Ok(document) => document,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("新增 RSG 失败：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    document.record.info.ptx_before_number = archive.header.ptx_number;
    let packet_name = document.record.info.name.clone();
    editing::record_addition(&mut edits.write(), document);
    selection.set(Some(RowSelection::Packet(packet_index)));
    set_status(
        status,
        AppStatus::new(
            format!("已创建空 RSG {packet_name} · 尚未保存"),
            StatusTone::Warning,
        ),
    );
}

fn create_rsg_dialog(
    archive: &ArchiveDocument,
    edits: &PacketEdits,
    removed: &RemovedPackets,
) -> EditDialog {
    let records = editing::visible_packet_records(archive, edits, removed);
    let mut suffix = 1_u32;
    let name = loop {
        let candidate = if suffix == 1 {
            "NewRsg".to_string()
        } else {
            format!("NewRsg_{suffix}")
        };
        if !records
            .iter()
            .any(|record| record.info.name.eq_ignore_ascii_case(&candidate))
        {
            break candidate;
        }
        suffix = suffix.saturating_add(1);
    };
    EditDialog::CreateRsg(NewRsgDraft {
        name,
        compression_flags: "2".into(),
    })
}

fn sibling_name_exists(
    files: &[UnpackedFile],
    virtual_directories: &BTreeSet<Vec<String>>,
    parent: &[String],
    name: &str,
    excluded_directory: Option<&[String]>,
) -> bool {
    let file_matches = files.iter().any(|file| {
        let components = crate::domain::archive_path_components(&file.path);
        if excluded_directory.is_some_and(|excluded| components.starts_with(excluded)) {
            return false;
        }
        components.len() > parent.len()
            && components.starts_with(parent)
            && components[parent.len()].eq_ignore_ascii_case(name)
    });
    file_matches
        || virtual_directories.iter().any(|path| {
            if excluded_directory.is_some_and(|excluded| path.starts_with(excluded)) {
                return false;
            }
            path.len() > parent.len()
                && path.starts_with(parent)
                && path[parent.len()].eq_ignore_ascii_case(name)
        })
}

fn create_folder_dialog(
    parent: Vec<String>,
    packet: &PacketDocument,
    virtual_directories: &BTreeSet<Vec<String>>,
) -> EditDialog {
    let mut suffix = 1_u32;
    let name = loop {
        let candidate = if suffix == 1 {
            "新建文件夹".to_string()
        } else {
            format!("新建文件夹 {suffix}")
        };
        if !sibling_name_exists(
            &packet.files,
            virtual_directories,
            &parent,
            &candidate,
            None,
        ) {
            break candidate;
        }
        suffix = suffix.saturating_add(1);
    };
    EditDialog::CreateFolder(NewFolderDraft { parent, name })
}

fn create_folder(
    draft: NewFolderDraft,
    packet: Signal<Option<Arc<PacketDocument>>>,
    mut virtual_directories: Signal<VirtualDirectories>,
    mut selection: Signal<Option<RowSelection>>,
    status: Signal<AppStatus>,
) {
    let Some(packet) = packet() else {
        return;
    };
    let name = draft.name.trim();
    if name.is_empty()
        || name.len() >= 128
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
    {
        set_status(
            status,
            AppStatus::new(
                "文件夹名称必须少于 128 字节，且不能是路径或包含路径分隔符",
                StatusTone::Error,
            ),
        );
        return;
    }

    let packet_index = packet.record.index;
    let existing = virtual_directories
        .read()
        .get(&packet_index)
        .cloned()
        .unwrap_or_default();
    if sibling_name_exists(&packet.files, &existing, &draft.parent, name, None) {
        set_status(
            status,
            AppStatus::new(format!("新建文件夹失败：{name} 已存在"), StatusTone::Error),
        );
        return;
    }

    let mut path = draft.parent;
    path.push(name.to_string());
    virtual_directories
        .write()
        .entry(packet_index)
        .or_default()
        .insert(path.clone());
    selection.set(Some(RowSelection::Directory(path)));
    set_status(
        status,
        AppStatus::new(
            format!("已创建文件夹 {name} · 添加文件后会写入 RSG"),
            StatusTone::Success,
        ),
    );
}

fn replace_selected_rsg(
    imported: AddedFile,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    selection: Signal<Option<RowSelection>>,
    mut edits: Signal<PacketEdits>,
    status: Signal<AppStatus>,
) {
    let (Some(archive), Some(RowSelection::Packet(packet_index))) = (archive(), selection()) else {
        return;
    };
    let current = edits
        .read()
        .get(&packet_index)
        .map(|edit| edit.document.clone())
        .or_else(|| archive.load_packet(packet_index).ok().map(Arc::new));
    let Some(current) = current else {
        set_status(
            status,
            AppStatus::new("替换 RSG 失败：无法读取原数据包", StatusTone::Error),
        );
        return;
    };
    let updated = match editing::import_replacement_packet(&current, imported.data) {
        Ok(updated) => updated,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("替换 RSG 失败：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    let name = updated.record.info.name.clone();
    let file_count = updated.files.len();
    let updated = editing::record_edit(&mut edits.write(), &current, updated);
    if packet()
        .as_ref()
        .is_some_and(|packet| packet.record.index == packet_index)
    {
        packet.set(Some(updated));
    }
    set_status(
        status,
        AppStatus::new(
            format!("已替换 {name} · {file_count} 个文件 · 尚未保存"),
            StatusTone::Warning,
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn delete_selected(
    target: RowSelection,
    mut archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    mut selection: Signal<Option<RowSelection>>,
    mut edits: Signal<PacketEdits>,
    mut removed_packets: Signal<RemovedPackets>,
    mut location: Signal<BrowserLocation>,
    mut ptx_infos: Signal<Vec<RsbPtxInfo>>,
    mut virtual_directories: Signal<VirtualDirectories>,
    status: Signal<AppStatus>,
) {
    if let RowSelection::Packet(packet_index) = target {
        let Some(document) = archive() else {
            return;
        };
        let record = editing::packet_record(&document, &edits.read(), packet_index);
        let Some(record) = record else {
            return;
        };

        let texture_start = match usize::try_from(record.info.ptx_before_number) {
            Ok(value) => value,
            Err(_) => {
                set_status(
                    status,
                    AppStatus::new("删除 RSG 失败：PTX 起始序号溢出", StatusTone::Error),
                );
                return;
            }
        };
        let texture_count = match usize::try_from(record.info.ptx_number) {
            Ok(value) => value,
            Err(_) => {
                set_status(
                    status,
                    AppStatus::new("删除 RSG 失败：PTX 数量溢出", StatusTone::Error),
                );
                return;
            }
        };
        let Some(texture_end) = texture_start.checked_add(texture_count) else {
            set_status(
                status,
                AppStatus::new("删除 RSG 失败：PTX 序号范围溢出", StatusTone::Error),
            );
            return;
        };
        let mut infos = ptx_infos.read().clone();
        if texture_end > infos.len() {
            set_status(
                status,
                AppStatus::new(
                    "删除 RSG 失败：对应的全局 PTX 元数据范围无效",
                    StatusTone::Error,
                ),
            );
            return;
        }

        let shifted = if texture_count == 0 {
            None
        } else {
            infos.drain(texture_start..texture_end);
            reindex_ptx_infos(&mut infos);
            let delta = match i64::try_from(texture_count) {
                Ok(value) => -value,
                Err(_) => {
                    set_status(
                        status,
                        AppStatus::new("删除 RSG 失败：PTX 数量超出范围", StatusTone::Error),
                    );
                    return;
                }
            };
            let shifted = match editing::shift_texture_ranges_after(
                &document,
                &mut edits.write(),
                packet_index,
                delta,
            ) {
                Ok(value) => value,
                Err(error) => {
                    set_status(
                        status,
                        AppStatus::new(format!("删除 RSG 失败：{error}"), StatusTone::Error),
                    );
                    return;
                }
            };
            Some(shifted)
        };

        edits.write().remove(&packet_index);
        virtual_directories.write().remove(&packet_index);
        if packet_index < document.packets.len() {
            removed_packets.write().insert(packet_index);
        }
        if let Some(shifted) = shifted {
            ptx_infos.set(infos);
            archive.set(Some(Arc::new(shifted)));
        }
        if packet()
            .as_ref()
            .is_some_and(|packet| packet.record.index == packet_index)
        {
            packet.set(None);
            location.set(BrowserLocation::Archive);
        }
        selection.set(None);
        set_status(
            status,
            AppStatus::new(
                if texture_count == 0 {
                    format!("已删除 RSG {} · 尚未保存", record.info.name)
                } else {
                    format!(
                        "已删除 RSG {} 及 {texture_count} 个 PTX · 索引已重排 · 尚未保存",
                        record.info.name
                    )
                },
                StatusTone::Warning,
            ),
        );
        return;
    }

    let Some(current) = packet() else {
        return;
    };
    let directory_target = match &target {
        RowSelection::Directory(path) => Some(path.clone()),
        _ => None,
    };
    let remove_indices = match &target {
        RowSelection::File(index) => vec![*index],
        RowSelection::Directory(path) => current
            .directory_index
            .files_below(&current.files, path)
            .into_iter()
            .map(|(index, _)| index)
            .collect(),
        RowSelection::Packet(_) => unreachable!(),
    };
    let packet_index = current.record.index;
    let previous_virtual = virtual_directories
        .read()
        .get(&packet_index)
        .cloned()
        .unwrap_or_default();
    let updated_virtual = if let Some(path) = directory_target.as_ref() {
        previous_virtual
            .iter()
            .filter(|candidate| !candidate.starts_with(path))
            .cloned()
            .collect::<BTreeSet<_>>()
    } else {
        previous_virtual.clone()
    };
    let removed_virtual_count = previous_virtual.len().saturating_sub(updated_virtual.len());
    let remove = remove_indices
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    if remove.is_empty() && removed_virtual_count > 0 {
        replace_virtual_directories(
            &mut virtual_directories.write(),
            packet_index,
            updated_virtual,
        );
        selection.set(None);
        set_status(
            status,
            AppStatus::new("已删除空文件夹", StatusTone::Success),
        );
        return;
    }
    let mut removed_texture_ids = remove
        .iter()
        .filter_map(|index| current.files.get(*index))
        .filter(|file| file.is_part1)
        .filter_map(|file| file.part1_info.as_ref().map(|info| info.id))
        .collect::<Vec<_>>();
    removed_texture_ids.sort_unstable();
    removed_texture_ids.dedup();

    let mut files = current
        .files
        .iter()
        .enumerate()
        .filter(|(index, _)| !remove.contains(index))
        .map(|(_, file)| file.clone())
        .collect::<Vec<_>>();
    if removed_texture_ids.is_empty() {
        let name = current.record.info.name.clone();
        let flags = current
            .record
            .header
            .as_ref()
            .map(|header| header.flags)
            .unwrap_or_default();
        match commit_packet_files(files, name, flags, packet, edits, status) {
            Ok(()) => {
                replace_virtual_directories(
                    &mut virtual_directories.write(),
                    packet_index,
                    updated_virtual,
                );
                selection.set(None);
            }
            Err(error) => set_status(
                status,
                AppStatus::new(format!("删除失败：{error}"), StatusTone::Error),
            ),
        }
        return;
    }

    let Some(document) = archive() else {
        return;
    };
    for file in &mut files {
        let Some(info) = file.part1_info.as_mut() else {
            continue;
        };
        let removed_before = removed_texture_ids.partition_point(|id| *id < info.id);
        info.id = info
            .id
            .checked_sub(u32::try_from(removed_before).unwrap_or(u32::MAX))
            .unwrap_or_default();
    }
    let begin = match usize::try_from(current.record.info.ptx_before_number) {
        Ok(value) => value,
        Err(_) => {
            set_status(
                status,
                AppStatus::new("删除纹理失败：PTX 起始序号溢出", StatusTone::Error),
            );
            return;
        }
    };
    let mut infos = ptx_infos.read().clone();
    let mut global_indices = removed_texture_ids
        .iter()
        .filter_map(|id| usize::try_from(*id).ok())
        .filter_map(|id| begin.checked_add(id))
        .collect::<Vec<_>>();
    if global_indices.len() != removed_texture_ids.len()
        || global_indices.iter().any(|index| *index >= infos.len())
    {
        set_status(
            status,
            AppStatus::new("删除纹理失败：全局 PTX 序号无效", StatusTone::Error),
        );
        return;
    }
    global_indices.sort_unstable_by(|left, right| right.cmp(left));
    for index in global_indices {
        infos.remove(index);
    }
    reindex_ptx_infos(&mut infos);

    let name = current.record.info.name.clone();
    let flags = current
        .record
        .header
        .as_ref()
        .map(|header| header.flags)
        .unwrap_or_default();
    let rebuilt = match editing::repack_packet(&current, name, flags, files) {
        Ok(value) => value,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("删除失败：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    let delta = -(i64::try_from(removed_texture_ids.len()).unwrap_or(i64::MAX));
    let mut edit_guard = edits.write();
    let rebuilt = editing::record_edit(&mut edit_guard, &current, rebuilt);
    let shifted = match editing::shift_texture_ranges_after(
        &document,
        &mut edit_guard,
        current.record.index,
        delta,
    ) {
        Ok(value) => value,
        Err(error) => {
            set_status(
                status,
                AppStatus::new(format!("删除纹理失败：{error}"), StatusTone::Error),
            );
            return;
        }
    };
    drop(edit_guard);
    ptx_infos.set(infos);
    archive.set(Some(Arc::new(shifted)));
    packet.set(Some(rebuilt));
    replace_virtual_directories(
        &mut virtual_directories.write(),
        packet_index,
        updated_virtual,
    );
    selection.set(None);
    set_status(
        status,
        AppStatus::new(
            format!(
                "已删除 {} 个纹理 · 后续 PTX 序号已重排 · 尚未保存",
                removed_texture_ids.len()
            ),
            StatusTone::Warning,
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn rename_selected(
    target: RowSelection,
    value: String,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    mut selection: Signal<Option<RowSelection>>,
    mut edits: Signal<PacketEdits>,
    removed_packets: Signal<RemovedPackets>,
    mut virtual_directories: Signal<VirtualDirectories>,
    status: Signal<AppStatus>,
) {
    if let RowSelection::Packet(packet_index) = target {
        let value = value.trim();
        if value.is_empty() || value.len() >= 128 || value.contains('/') || value.contains('\\') {
            set_status(
                status,
                AppStatus::new(
                    "RSG 名称必须是少于 128 字节且不含路径分隔符的名称",
                    StatusTone::Error,
                ),
            );
            return;
        }
        let Some(archive) = archive() else {
            return;
        };
        if editing::visible_packet_records(&archive, &edits.read(), &removed_packets.read())
            .iter()
            .any(|record| {
                record.index != packet_index && record.info.name.eq_ignore_ascii_case(value)
            })
        {
            set_status(
                status,
                AppStatus::new("RSG 名称与现有数据包重复", StatusTone::Error),
            );
            return;
        }
        let current = edits
            .read()
            .get(&packet_index)
            .map(|edit| edit.document.clone())
            .or_else(|| archive.load_packet(packet_index).ok().map(Arc::new));
        let Some(current) = current else {
            set_status(
                status,
                AppStatus::new("重命名失败：无法读取 RSG", StatusTone::Error),
            );
            return;
        };
        let flags = current
            .record
            .header
            .as_ref()
            .map(|header| header.flags)
            .unwrap_or_default();
        let updated = match editing::repack_packet(
            &current,
            value.to_string(),
            flags,
            current.files.as_ref().clone(),
        ) {
            Ok(updated) => updated,
            Err(error) => {
                set_status(
                    status,
                    AppStatus::new(format!("重命名失败：{error}"), StatusTone::Error),
                );
                return;
            }
        };
        let updated = editing::record_edit(&mut edits.write(), &current, updated);
        if packet()
            .as_ref()
            .is_some_and(|packet| packet.record.index == packet_index)
        {
            packet.set(Some(updated));
        }
        selection.set(None);
        set_status(
            status,
            AppStatus::new(
                format!("已将 RSG 重命名为 {value} · 尚未保存"),
                StatusTone::Warning,
            ),
        );
        return;
    }

    let Some(current) = packet() else {
        return;
    };
    let value = match &target {
        RowSelection::Directory(_) => {
            let value = value.trim();
            if value.is_empty()
                || value.len() >= 128
                || value == "."
                || value == ".."
                || value.contains('/')
                || value.contains('\\')
            {
                set_status(
                    status,
                    AppStatus::new(
                        "文件夹名称必须少于 128 字节，且不能是路径或包含路径分隔符",
                        StatusTone::Error,
                    ),
                );
                return;
            }
            value.to_string()
        }
        _ => {
            let value = editing::normalize_archive_path(&value);
            if value.is_empty() {
                set_status(status, AppStatus::new("名称不能为空", StatusTone::Error));
                return;
            }
            value
        }
    };
    let packet_index = current.record.index;
    let previous_virtual = virtual_directories
        .read()
        .get(&packet_index)
        .cloned()
        .unwrap_or_default();
    let mut updated_virtual = previous_virtual.clone();
    let mut files = current.files.as_ref().clone();
    let mut files_changed = false;
    match &target {
        RowSelection::File(index) => {
            let Some(file) = files.get_mut(*index) else {
                return;
            };
            let mut components = crate::domain::archive_path_components(&file.path);
            let Some(last) = components.last_mut() else {
                return;
            };
            files_changed = *last != value;
            *last = value.clone();
            file.path = components.join("/");
        }
        RowSelection::Directory(path) => {
            let Some(old_name) = path.last() else {
                return;
            };
            let parent = &path[..path.len().saturating_sub(1)];
            if !old_name.eq_ignore_ascii_case(&value)
                && sibling_name_exists(
                    &current.files,
                    &previous_virtual,
                    parent,
                    &value,
                    Some(path),
                )
            {
                set_status(
                    status,
                    AppStatus::new("重命名后会与现有项目重名", StatusTone::Error),
                );
                return;
            }
            for file in &mut files {
                let mut components = crate::domain::archive_path_components(&file.path);
                if components.len() > parent.len()
                    && components[..parent.len()] == *parent
                    && components[parent.len()] == *old_name
                {
                    components[parent.len()] = value.clone();
                    file.path = components.join("/");
                    files_changed = true;
                }
            }
            updated_virtual = previous_virtual
                .iter()
                .map(|candidate| {
                    if candidate.starts_with(path) {
                        let mut renamed = parent.to_vec();
                        renamed.push(value.clone());
                        renamed.extend_from_slice(&candidate[path.len()..]);
                        renamed
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
        }
        RowSelection::Packet(_) => unreachable!(),
    }
    let mut paths = std::collections::HashSet::new();
    if files
        .iter()
        .any(|file| !paths.insert(editing::normalize_archive_path(&file.path).to_ascii_lowercase()))
    {
        set_status(
            status,
            AppStatus::new("重命名后会产生重复路径", StatusTone::Error),
        );
        return;
    }
    if !files_changed {
        let is_directory = matches!(target, RowSelection::Directory(_));
        if is_directory {
            replace_virtual_directories(
                &mut virtual_directories.write(),
                packet_index,
                updated_virtual,
            );
        }
        selection.set(None);
        set_status(
            status,
            AppStatus::new(
                if is_directory {
                    format!("已将文件夹重命名为 {value}")
                } else {
                    "名称未发生变化".to_string()
                },
                StatusTone::Success,
            ),
        );
        return;
    }
    let name = current.record.info.name.clone();
    let flags = current
        .record
        .header
        .as_ref()
        .map(|header| header.flags)
        .unwrap_or_default();
    match commit_packet_files(files, name, flags, packet, edits, status) {
        Ok(()) => {
            replace_virtual_directories(
                &mut virtual_directories.write(),
                packet_index,
                updated_virtual,
            );
            selection.set(None);
        }
        Err(error) => set_status(
            status,
            AppStatus::new(format!("重命名失败：{error}"), StatusTone::Error),
        ),
    }
}

fn rename_dialog(
    target: RowSelection,
    archive: Option<Arc<ArchiveDocument>>,
    packet: Option<Arc<PacketDocument>>,
    edits: &PacketEdits,
) -> Option<EditDialog> {
    let value = match &target {
        RowSelection::File(index) => packet
            .and_then(|packet| packet.files.get(*index).cloned())
            .map(|file| archive_file_name(&file.path).to_string()),
        RowSelection::Directory(path) => path.last().cloned(),
        RowSelection::Packet(index) => archive
            .and_then(|archive| editing::packet_record(&archive, edits, *index))
            .map(|record| record.info.name),
    }?;
    Some(EditDialog::Rename { target, value })
}

fn delete_dialog(
    target: RowSelection,
    archive: Option<Arc<ArchiveDocument>>,
    packet: Option<Arc<PacketDocument>>,
    edits: &PacketEdits,
) -> Option<EditDialog> {
    let label = match &target {
        RowSelection::File(index) => packet
            .and_then(|packet| packet.files.get(*index).cloned())
            .map(|file| archive_file_name(&file.path).to_string()),
        RowSelection::Directory(path) => path.last().cloned(),
        RowSelection::Packet(index) => archive
            .and_then(|archive| editing::packet_record(&archive, edits, *index))
            .map(|record| record.info.name),
    }?;
    Some(EditDialog::Delete { target, label })
}

fn property_draft(
    target: RowSelection,
    archive: Option<Arc<ArchiveDocument>>,
    packet: Option<Arc<PacketDocument>>,
    edits: &PacketEdits,
    ptx_infos: &[RsbPtxInfo],
) -> Option<PropertiesDraft> {
    match target {
        RowSelection::Packet(index) => {
            let record = packet
                .filter(|packet| packet.record.index == index)
                .map(|packet| packet.record.clone())
                .or_else(|| {
                    archive
                        .as_ref()
                        .and_then(|archive| editing::packet_record(archive, edits, index))
                })?;
            Some(PropertiesDraft {
                target: PropertiesTarget::Packet(index),
                name_or_path: record.info.name,
                compression_flags: record
                    .header
                    .as_ref()
                    .map(|header| header.flags.to_string())
                    .unwrap_or_default(),
                width: String::new(),
                height: String::new(),
                format: String::new(),
                pitch: String::new(),
                alpha_size: String::new(),
                alpha_format: String::new(),
            })
        }
        RowSelection::File(index) => {
            let archive = archive?;
            let packet = packet?;
            let file = packet.files.get(index)?;
            let texture = file.part1_info.as_ref();
            let metadata = archive
                .texture_metadata(&packet.record, file)
                .and_then(|metadata| ptx_infos.get(metadata.global_index));
            Some(PropertiesDraft {
                target: PropertiesTarget::File(index),
                name_or_path: file.path.clone(),
                compression_flags: String::new(),
                width: texture
                    .map(|info| info.width.to_string())
                    .unwrap_or_default(),
                height: texture
                    .map(|info| info.height.to_string())
                    .unwrap_or_default(),
                format: metadata
                    .map(|info| info.format.to_string())
                    .unwrap_or_default(),
                pitch: metadata
                    .map(|info| info.pitch.to_string())
                    .unwrap_or_default(),
                alpha_size: metadata
                    .and_then(|info| info.alpha_size)
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                alpha_format: metadata
                    .and_then(|info| info.alpha_format)
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            })
        }
        RowSelection::Directory(_) => None,
    }
}

fn apply_properties(
    draft: PropertiesDraft,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    mut packet: Signal<Option<Arc<PacketDocument>>>,
    mut edits: Signal<PacketEdits>,
    removed_packets: Signal<RemovedPackets>,
    mut ptx_infos: Signal<Vec<RsbPtxInfo>>,
    status: Signal<AppStatus>,
) {
    match draft.target {
        PropertiesTarget::Packet(index) => {
            let Some(archive) = archive() else {
                return;
            };
            let current = packet()
                .filter(|packet| packet.record.index == index)
                .or_else(|| edits.read().get(&index).map(|edit| edit.document.clone()))
                .or_else(|| archive.load_packet(index).ok().map(Arc::new));
            let Some(current) = current else {
                set_status(
                    status,
                    AppStatus::new("无法读取该 RSG 包", StatusTone::Error),
                );
                return;
            };
            let name = draft.name_or_path.trim();
            if name.is_empty() || name.len() >= 128 || name.contains('/') || name.contains('\\') {
                set_status(
                    status,
                    AppStatus::new(
                        "RSG 名称必须是少于 128 字节且不含路径分隔符的名称",
                        StatusTone::Error,
                    ),
                );
                return;
            }
            if editing::visible_packet_records(&archive, &edits.read(), &removed_packets.read())
                .iter()
                .any(|record| record.index != index && record.info.name.eq_ignore_ascii_case(name))
            {
                set_status(
                    status,
                    AppStatus::new("RSG 名称与现有数据包重复", StatusTone::Error),
                );
                return;
            }
            let Ok(flags) = draft.compression_flags.parse::<u32>() else {
                set_status(
                    status,
                    AppStatus::new("压缩标志必须是 0 到 3", StatusTone::Error),
                );
                return;
            };
            if flags > 3 {
                set_status(
                    status,
                    AppStatus::new("压缩标志必须是 0 到 3", StatusTone::Error),
                );
                return;
            }
            match editing::repack_packet(
                &current,
                name.to_string(),
                flags,
                current.files.as_ref().clone(),
            ) {
                Ok(updated) => {
                    let updated = editing::record_edit(&mut edits.write(), &current, updated);
                    if packet()
                        .as_ref()
                        .is_some_and(|packet| packet.record.index == index)
                    {
                        packet.set(Some(updated));
                    }
                    set_status(
                        status,
                        AppStatus::new(
                            format!("已修改 RSG {name} 的属性 · 尚未保存"),
                            StatusTone::Warning,
                        ),
                    );
                }
                Err(error) => set_status(
                    status,
                    AppStatus::new(format!("属性修改失败：{error}"), StatusTone::Error),
                ),
            }
        }
        PropertiesTarget::File(index) => {
            let (Some(archive), Some(current)) = (archive(), packet()) else {
                return;
            };
            let Some(original_file) = current.files.get(index) else {
                return;
            };
            let mut files = current.files.as_ref().clone();
            let path = editing::normalize_archive_path(&draft.name_or_path);
            if path.is_empty() {
                set_status(
                    status,
                    AppStatus::new("文件路径不能为空", StatusTone::Error),
                );
                return;
            }
            if files.iter().enumerate().any(|(other_index, file)| {
                other_index != index
                    && editing::normalize_archive_path(&file.path).eq_ignore_ascii_case(&path)
            }) {
                set_status(
                    status,
                    AppStatus::new("文件路径与现有项目重复", StatusTone::Error),
                );
                return;
            }
            files[index].path = path;
            if original_file.is_part1 {
                let parse_positive = |value: &str, label: &str| -> Result<u32, String> {
                    let parsed = value
                        .parse::<u32>()
                        .map_err(|_| format!("{label}必须是正整数"))?;
                    (parsed > 0)
                        .then_some(parsed)
                        .ok_or_else(|| format!("{label}必须大于 0"))
                };
                let width = match parse_positive(&draft.width, "宽度") {
                    Ok(value) => value,
                    Err(error) => {
                        set_status(status, AppStatus::new(error, StatusTone::Error));
                        return;
                    }
                };
                let height = match parse_positive(&draft.height, "高度") {
                    Ok(value) => value,
                    Err(error) => {
                        set_status(status, AppStatus::new(error, StatusTone::Error));
                        return;
                    }
                };
                if let Some(info) = files[index].part1_info.as_mut() {
                    info.width = width;
                    info.height = height;
                }
                if let Some(metadata) = archive.texture_metadata(&current.record, original_file) {
                    let mut infos = ptx_infos.write();
                    if let Some(info) = infos.get_mut(metadata.global_index) {
                        let Ok(format) = draft.format.parse::<i32>() else {
                            set_status(
                                status,
                                AppStatus::new("格式代码必须是整数", StatusTone::Error),
                            );
                            return;
                        };
                        let Ok(pitch) = draft.pitch.parse::<i32>() else {
                            set_status(
                                status,
                                AppStatus::new("Pitch 必须是整数", StatusTone::Error),
                            );
                            return;
                        };
                        info.width = width as i32;
                        info.height = height as i32;
                        info.format = format;
                        info.pitch = pitch;
                        info.alpha_size = parse_optional_i32(&draft.alpha_size);
                        info.alpha_format = parse_optional_i32(&draft.alpha_format);
                    }
                }
            }
            let name = current.record.info.name.clone();
            let flags = current
                .record
                .header
                .as_ref()
                .map(|header| header.flags)
                .unwrap_or_default();
            if let Err(error) = commit_packet_files(files, name, flags, packet, edits, status) {
                set_status(
                    status,
                    AppStatus::new(format!("属性修改失败：{error}"), StatusTone::Error),
                );
            }
        }
    }
}

fn parse_optional_i32(value: &str) -> Option<i32> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.parse().ok()).flatten()
}

fn archive_with_ptx_infos(
    archive: Arc<ArchiveDocument>,
    ptx_infos: &[RsbPtxInfo],
) -> Arc<ArchiveDocument> {
    if archive.ptx_infos.as_slice() == ptx_infos {
        return archive;
    }
    let mut edited = archive.as_ref().clone();
    edited.ptx_infos = Arc::new(ptx_infos.to_vec());
    Arc::new(edited)
}

#[allow(clippy::too_many_arguments)]
fn save_edited_archive(
    save_as: bool,
    archive: Signal<Option<Arc<ArchiveDocument>>>,
    packet: Signal<Option<Arc<PacketDocument>>>,
    location: Signal<BrowserLocation>,
    selection: Signal<Option<RowSelection>>,
    query: Signal<String>,
    edits: Signal<PacketEdits>,
    removed_packets: Signal<RemovedPackets>,
    ptx_infos: Signal<Vec<RsbPtxInfo>>,
    status: Signal<AppStatus>,
) {
    let Some(document) = archive() else {
        return;
    };
    let edit = editing::archive_edit(
        &edits.read(),
        &removed_packets.read(),
        document.packets.len(),
        &ptx_infos.read(),
        document.ptx_infos.as_ref(),
    );
    if edit.packets.is_empty()
        && edit.added_packets.is_empty()
        && edit.removed_packets.is_empty()
        && edit.ptx_infos.is_none()
        && !save_as
    {
        return;
    }
    let has_edits = !edit.packets.is_empty()
        || !edit.added_packets.is_empty()
        || !edit.removed_packets.is_empty()
        || edit.ptx_infos.is_some();
    let default_name = document.display_name.clone();
    let channel_order_mode = document.channel_order_mode;
    #[cfg(not(target_arch = "wasm32"))]
    let overwrite = (!save_as)
        .then(|| document.source_path().map(std::path::Path::to_path_buf))
        .flatten();
    #[cfg(target_arch = "wasm32")]
    let overwrite = None::<std::path::PathBuf>;
    set_status(
        status,
        AppStatus::new("正在重建 RSB 索引与数据包…", StatusTone::Neutral),
    );
    spawn(async move {
        let bytes = match if has_edits {
            processing::rebuild_archive(document, edit).await
        } else {
            processing::source_bytes(document).await
        } {
            Ok(bytes) => bytes,
            Err(error) => {
                set_status(
                    status,
                    AppStatus::new(format!("保存失败：{error}"), StatusTone::Error),
                );
                return;
            }
        };
        let saved = match platform::save_archive(&default_name, &bytes, overwrite.as_deref()).await
        {
            Ok(Some(saved)) => saved,
            Ok(None) => {
                set_status(status, AppStatus::new("已取消保存", StatusTone::Neutral));
                return;
            }
            Err(error) => {
                set_status(
                    status,
                    AppStatus::new(format!("写入失败：{error}"), StatusTone::Error),
                );
                return;
            }
        };
        let loaded = match saved {
            #[cfg(not(target_arch = "wasm32"))]
            platform::SavedArchive::Native(path) => processing::open_native(path).await,
            #[cfg(target_arch = "wasm32")]
            platform::SavedArchive::Downloaded => loader::open_memory(default_name, bytes),
        };
        match loaded {
            Ok(mut document) => {
                document.channel_order_mode = channel_order_mode;
                install_editable_archive(
                    document,
                    archive,
                    packet,
                    location,
                    selection,
                    query,
                    status,
                    edits,
                    removed_packets,
                    ptx_infos,
                );
                set_status(
                    status,
                    AppStatus::new("RSB 归档已保存并重新验证", StatusTone::Success),
                );
            }
            Err(error) => set_status(
                status,
                AppStatus::new(
                    format!("文件已写入，但重新打开验证失败：{error}"),
                    StatusTone::Warning,
                ),
            ),
        }
    });
}

#[component]
pub fn RsbArchivePage(on_open_rton: Option<EventHandler<RsbRtonOpenRequest>>) -> Element {
    let mut archive = use_signal(|| None::<Arc<ArchiveDocument>>);
    let packet = use_signal(|| None::<Arc<PacketDocument>>);
    let packet_edits = use_signal(PacketEdits::new);
    let removed_packets = use_signal(RemovedPackets::new);
    let mut virtual_directories = use_signal(VirtualDirectories::new);
    let ptx_infos = use_signal(Vec::<RsbPtxInfo>::new);
    let mut location = use_signal(BrowserLocation::default);
    let mut selection = use_signal(|| None::<RowSelection>);
    let mut query = use_signal(String::new);
    let status = use_signal(|| AppStatus::new("打开或拖入一个 RSB 归档", StatusTone::Neutral));
    let mut dragging = use_signal(|| false);
    let mut tree_visible = use_signal(|| false);
    let mut inspector_visible = use_signal(|| false);
    let mut ptx_preview = use_signal(|| None::<PtxPreviewState>);
    let mut detail_preview = use_signal(|| None::<PtxPreviewState>);
    let mut preview_open = use_signal(|| false);
    let mut preview_cache = use_signal(PreviewCache::default);
    let mut archive_identity = use_signal(|| 0_usize);
    let mut table_scroll_top = use_signal(|| 0_f64);
    let mut scroll_restore = use_signal(ScrollRestore::default);
    let mut navigation_history = use_signal(Vec::<NavigationSnapshot>::new);
    let mut edit_dialog = use_signal(|| None::<EditDialog>);
    let mut context_menu = use_signal(|| None::<ArchiveContextMenuState>);
    let mut preview_context_menu = use_signal(|| None::<PtxPreviewContextMenuState>);
    let mut pending_archive = use_signal(|| None::<ArchiveDocument>);
    let navigation = NavigationSignals {
        location,
        selection,
        query,
        table_scroll_top,
        history: navigation_history,
        scroll_restore,
    };

    use_effect(move || {
        if selection().is_none() {
            processing::begin_request();
            ptx_preview.set(None);
            detail_preview.set(None);
            preview_open.set(false);
            preview_context_menu.set(None);
        }
    });
    use_effect(move || {
        let next = archive()
            .as_ref()
            .map(|document| document.identity())
            .unwrap_or_default();
        if archive_identity() != next {
            archive_identity.set(next);
            processing::begin_request();
            preview_cache.write().clear();
            ptx_preview.set(None);
            detail_preview.set(None);
            preview_open.set(false);
            table_scroll_top.set(0.0);
            scroll_restore.set(ScrollRestore::default());
            navigation_history.write().clear();
            context_menu.set(None);
            preview_context_menu.set(None);
            virtual_directories.write().clear();
        }
    });

    let archive_snapshot = archive();
    let packet_snapshot = packet();
    let location_snapshot = location();
    let selection_snapshot = selection();
    let query_snapshot = query();
    let status_snapshot = status();
    let ptx_preview_snapshot = ptx_preview();
    let detail_preview_snapshot = detail_preview();
    let context_menu_snapshot = context_menu();
    let preview_context_menu_snapshot = preview_context_menu();
    let virtual_directories_snapshot = packet_snapshot
        .as_ref()
        .and_then(|packet| {
            virtual_directories
                .read()
                .get(&packet.record.index)
                .cloned()
        })
        .unwrap_or_default();
    let edit_dialog_key = edit_dialog()
        .as_ref()
        .map(EditDialog::key)
        .unwrap_or("none");
    let has_unsaved_changes = !packet_edits.read().is_empty()
        || !removed_packets.read().is_empty()
        || archive_snapshot
            .as_ref()
            .is_some_and(|document| ptx_infos.read().as_slice() != document.ptx_infos.as_slice());
    let can_export_png = selection_snapshot.as_ref().is_some_and(|target| {
        archive_context_capabilities(target, packet_snapshot.as_deref(), None, false).export_png
    });
    let preview_signals = PreviewSignals {
        preview: ptx_preview,
        detail: detail_preview,
        cache: preview_cache,
        status,
    };

    rsx! {
        document::Stylesheet { href: RSB_PAGE_CSS }
        div {
            class: if dragging() { "rsb-page-host is-dragging" } else { "rsb-page-host" },
            ondragenter: move |event| {
                    event.prevent_default();
                    if !event.files().is_empty() {
                        dragging.set(true);
                    }
                },
            ondragover: move |event| {
                    event.prevent_default();
                    if !event.files().is_empty() {
                        dragging.set(true);
                    }
                },
            ondragleave: move |_| dragging.set(false),
            ondrop: move |event| async move {
                    event.prevent_default();
                    dragging.set(false);
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };
                    set_status(
                        status,
                        AppStatus::new("正在读取归档索引…", StatusTone::Neutral),
                    );
                    match open_file_data(file).await {
                        Ok(document) => {
                            if has_unsaved_changes {
                                pending_archive.set(Some(document));
                            } else {
                                install_editable_archive(
                                    document,
                                    archive,
                                    packet,
                                    location,
                                    selection,
                                    query,
                                    status,
                                    packet_edits,
                                    removed_packets,
                                    ptx_infos,
                                );
                            }
                        }
                        Err(error) => set_status(
                            status,
                            AppStatus::new(format!("打开失败：{error}"), StatusTone::Error),
                        ),
                    }
            },
            ToolPage { namespace: "rsb", class: "rsb-app",
                ToolPageToolbar {
                    class: "rsb-page-toolbar",
                    actions: rsx! { ArchiveToolbar {
                    archive_name: archive_snapshot
                        .as_ref()
                        .map(|archive| archive.display_name.clone())
                        .unwrap_or_else(|| "未打开归档".to_string()),
                    has_archive: archive_snapshot.is_some(),
                    has_unsaved_changes,
                    in_packet: matches!(location_snapshot, BrowserLocation::Packet { .. }),
                    can_properties: matches!(
                        selection_snapshot,
                        Some(RowSelection::Packet(_) | RowSelection::File(_))
                    ) || (matches!(location_snapshot, BrowserLocation::Packet { .. })
                        && selection_snapshot.is_none()),
                    can_replace: matches!(selection_snapshot, Some(RowSelection::File(_))),
                    can_replace_rsg: matches!(selection_snapshot, Some(RowSelection::Packet(_))),
                    can_rename: matches!(
                        selection_snapshot,
                        Some(
                            RowSelection::Packet(_)
                                | RowSelection::File(_)
                                | RowSelection::Directory(_)
                        )
                    ),
                    can_delete: match selection_snapshot {
                        Some(RowSelection::Packet(index)) => archive_snapshot
                            .as_ref()
                            .and_then(|archive| {
                                editing::packet_record(archive, &packet_edits.read(), index)
                            })
                            .is_some(),
                        Some(RowSelection::File(_) | RowSelection::Directory(_)) => true,
                        None => false,
                    },
                    can_export_png,
                    can_go_up: !matches!(location_snapshot, BrowserLocation::Archive),
                    tree_visible: tree_visible(),
                    inspector_visible: inspector_visible(),
                    on_open: move |document| {
                        if has_unsaved_changes {
                            pending_archive.set(Some(document));
                        } else {
                            install_editable_archive(
                                document,
                                archive,
                                packet,
                                location,
                                selection,
                                query,
                                status,
                                packet_edits,
                                removed_packets,
                                ptx_infos,
                            );
                        }
                    },
                    on_error: move |error| set_status(
                        status,
                        AppStatus::new(format!("打开失败：{error}"), StatusTone::Error),
                    ),
                    on_up: move |_| go_up(navigation),
                    on_extract: move |_| {
                        spawn(async move {
                            extract_current(
                                archive,
                                packet,
                                location,
                                selection,
                                packet_edits,
                                status,
                            )
                            .await;
                        });
                    },
                    on_export_png: move |_| {
                        let target = selection();
                        spawn(async move {
                            export_ptx_png(target, archive, packet, ptx_infos, status).await;
                        });
                    },
                    on_save: move |_| save_edited_archive(
                        false,
                        archive,
                        packet,
                        location,
                        selection,
                        query,
                        packet_edits,
                        removed_packets,
                        ptx_infos,
                        status,
                    ),
                    on_save_as: move |_| save_edited_archive(
                        true,
                        archive,
                        packet,
                        location,
                        selection,
                        query,
                        packet_edits,
                        removed_packets,
                        ptx_infos,
                        status,
                    ),
                    on_add: move |files| {
                        let directory = match location() {
                            BrowserLocation::Packet { directory, .. } => directory,
                            BrowserLocation::Archive => return,
                        };
                        open_file_import(files, directory, edit_dialog);
                        processing::begin_request();
                        ptx_preview.set(None);
                        detail_preview.set(None);
                    },
                    on_create_folder: move |_| {
                        let (Some(packet), BrowserLocation::Packet { directory, .. }) =
                            (packet(), location())
                        else {
                            return;
                        };
                        let directories = virtual_directories
                            .read()
                            .get(&packet.record.index)
                            .cloned()
                            .unwrap_or_default();
                        edit_dialog.set(Some(create_folder_dialog(
                            directory,
                            &packet,
                            &directories,
                        )));
                    },
                    on_add_rsg: move |_| {
                        let Some(archive) = archive() else {
                            return;
                        };
                        edit_dialog.set(Some(create_rsg_dialog(
                            &archive,
                            &packet_edits.read(),
                            &removed_packets.read(),
                        )));
                    },
                    on_replace: move |file| {
                        replace_selected_file(
                            file,
                            archive,
                            packet,
                            selection,
                            packet_edits,
                            ptx_infos,
                            status,
                        );
                        processing::begin_request();
                        ptx_preview.set(None);
                        detail_preview.set(None);
                    },
                    on_replace_rsg: move |file| {
                        replace_selected_rsg(
                            file,
                            archive,
                            packet,
                            selection,
                            packet_edits,
                            status,
                        );
                    },
                    on_rename: move |_| {
                        let Some(target) = selection() else {
                            return;
                        };
                        if let Some(dialog) =
                            rename_dialog(target, archive(), packet(), &packet_edits.read())
                        {
                            edit_dialog.set(Some(dialog));
                        }
                    },
                    on_delete: move |_| {
                        let Some(target) = selection() else {
                            return;
                        };
                        if let Some(dialog) =
                            delete_dialog(target, archive(), packet(), &packet_edits.read())
                        {
                            edit_dialog.set(Some(dialog));
                        }
                    },
                    on_properties: move |_| {
                        let target = selection().or_else(|| match location() {
                            BrowserLocation::Packet { packet_index, .. } => {
                                Some(RowSelection::Packet(packet_index))
                            }
                            BrowserLocation::Archive => None,
                        });
                        let Some(target) = target else { return };
                        if let Some(draft) =
                            property_draft(
                                target,
                                archive(),
                                packet(),
                                &packet_edits.read(),
                                &ptx_infos.read(),
                            )
                        {
                            edit_dialog.set(Some(EditDialog::Properties(draft)));
                        }
                    },
                    on_toggle_tree: move |_| {
                        let open = !tree_visible();
                        tree_visible.set(open);
                        if open {
                            inspector_visible.set(false);
                        }
                    },
                    on_toggle_inspector: move |_| {
                        let open = !inspector_visible();
                        inspector_visible.set(open);
                        if open {
                            tree_visible.set(false);
                        }
                    },
                } },
                }

                WorkspaceCard { class: "rsb-browser-card", aria_label: "RSB Archive",
                    if let Some(document) = archive_snapshot.as_ref() {
                        AddressBar {
                            archive: document.clone(),
                            packet: packet_snapshot.clone(),
                            location: location_snapshot.clone(),
                            on_archive: move |_| {
                                location.set(BrowserLocation::Archive);
                                selection.set(None);
                                query.set(String::new());
                                navigation.request_scroll(0.0);
                            },
                            on_directory: move |directory: Vec<String>| {
                                enter_directory(directory, navigation);
                            },
                            query: query_snapshot.clone(),
                            on_query: move |value| {
                                query.set(value);
                                navigation.request_scroll(0.0);
                            },
                        }

                        div { class: "rsb-browser-grid",
                            ArchiveTable {
                                archive: document.clone(),
                                edits: packet_edits.read().clone(),
                                removed_packets: removed_packets.read().clone(),
                                virtual_directories: virtual_directories_snapshot.clone(),
                                packet: packet_snapshot.clone(),
                                location: location_snapshot.clone(),
                                selection: selection_snapshot.clone(),
                                query: query_snapshot.clone(),
                                restore_scroll: scroll_restore(),
                                on_scroll: move |top| table_scroll_top.set(top),
                                on_select: move |next| {
                                    let archive = archive().map(|archive| {
                                        archive_with_ptx_infos(archive, &ptx_infos.read())
                                    });
                                    select_item(
                                        next,
                                        archive,
                                        packet(),
                                        selection,
                                        preview_signals,
                                    );
                                },
                                on_context_menu: move |(target, x, y): (RowSelection, f64, f64)| {
                                    let archive = archive().map(|archive| {
                                        archive_with_ptx_infos(archive, &ptx_infos.read())
                                    });
                                    select_item(
                                        target.clone(),
                                        archive,
                                        packet(),
                                        selection,
                                        preview_signals,
                                    );
                                    context_menu.set(Some(ArchiveContextMenuState {
                                        target: Some(target),
                                        directory: Vec::new(),
                                        x,
                                        y,
                                    }));
                                },
                                on_blank_context_menu: move |(directory, x, y)| {
                                    selection.set(None);
                                    context_menu.set(Some(ArchiveContextMenuState {
                                        target: None,
                                        directory,
                                        x,
                                        y,
                                    }));
                                },
                                on_packet: move |index| enter_packet(
                                    index,
                                    archive,
                                    packet,
                                    packet_edits,
                                    navigation,
                                    status,
                                ),
                                on_directory: move |directory| {
                                    enter_directory(directory, navigation);
                                },
                                on_file: move |index| open_file_item(
                                    index,
                                    archive,
                                    packet,
                                    selection,
                                    ptx_infos,
                                    ptx_preview,
                                    detail_preview,
                                    preview_cache,
                                    preview_open,
                                    status,
                                    on_open_rton,
                                ),
                            }
                        }

                        StatusBar {
                            archive: document.clone(),
                            edits: packet_edits.read().clone(),
                            removed_packets: removed_packets.read().clone(),
                            virtual_directories: virtual_directories_snapshot.clone(),
                            packet: packet_snapshot.clone(),
                            location: location_snapshot.clone(),
                            status: status_snapshot.clone(),
                        }
                    } else {
                        EmptyArchive {
                            on_open: move |document| install_editable_archive(
                                document,
                                archive,
                                packet,
                                location,
                                selection,
                                query,
                                status,
                                packet_edits,
                                removed_packets,
                                ptx_infos,
                            ),
                            on_error: move |error| set_status(
                                status,
                                AppStatus::new(format!("打开失败：{error}"), StatusTone::Error),
                            ),
                        }
                    }
                }

                if let Some(document) = archive_snapshot.as_ref() {
                    ContextSheet {
                        open: tree_visible(),
                        side: "left",
                        title: "归档目录",
                        close_label: "关闭归档目录",
                        class: "rsb-context-sheet-layer",
                        on_close: move |_| tree_visible.set(false),
                        ArchiveTree {
                            archive: document.clone(),
                            edits: packet_edits.read().clone(),
                            removed_packets: removed_packets.read().clone(),
                            virtual_directories: virtual_directories_snapshot.clone(),
                            packet: packet_snapshot.clone(),
                            location: location_snapshot.clone(),
                            on_archive: move |_| {
                                location.set(BrowserLocation::Archive);
                                selection.set(None);
                                query.set(String::new());
                                navigation.request_scroll(0.0);
                                tree_visible.set(false);
                            },
                            on_packet: move |index| {
                                enter_packet(
                                    index,
                                    archive,
                                    packet,
                                    packet_edits,
                                    navigation,
                                    status,
                                );
                                tree_visible.set(false);
                            },
                            on_directory: move |directory| {
                                enter_directory(directory, navigation);
                                tree_visible.set(false);
                            },
                        }
                    }

                    ContextSheet {
                        open: inspector_visible(),
                        title: "详情",
                        close_label: "关闭详情",
                        class: "rsb-context-sheet-layer",
                        on_close: move |_| inspector_visible.set(false),
                        Inspector {
                            archive: document.clone(),
                            edits: packet_edits.read().clone(),
                            removed_packets: removed_packets.read().clone(),
                            ptx_infos,
                            packet: packet_snapshot.clone(),
                            location: location_snapshot.clone(),
                            selection: selection_snapshot.clone(),
                            preview: ptx_preview_snapshot.clone(),
                            on_channel_order_mode: move |mode| {
                                let Some(current) = archive() else {
                                    return;
                                };
                                if current.channel_order_mode == mode {
                                    return;
                                }
                                let mut updated = current.as_ref().clone();
                                updated.channel_order_mode = mode;
                                let updated = Arc::new(updated);
                                archive.set(Some(updated.clone()));
                                processing::begin_request();
                                preview_cache.write().clear();
                                ptx_preview.set(None);
                                detail_preview.set(None);
                                preview_open.set(false);

                                if let (Some(RowSelection::File(index)), Some(packet)) =
                                    (selection(), packet())
                                {
                                    let updated =
                                        archive_with_ptx_infos(updated, &ptx_infos.read());
                                    request_preview(
                                        updated,
                                        packet,
                                        index,
                                        PreviewQuality::Thumbnail,
                                        ptx_preview,
                                        preview_cache,
                                        status,
                                    );
                                } else {
                                    let label = match mode {
                                        ArchiveChannelOrderMode::Auto => "自动识别",
                                        ArchiveChannelOrderMode::Rgba => "RGBA",
                                        ArchiveChannelOrderMode::Apple => "Apple BGRA",
                                    };
                                    set_status(
                                        status,
                                        AppStatus::new(
                                            format!("纹理通道顺序已切换为 {label}"),
                                            StatusTone::Success,
                                        ),
                                    );
                                }
                            },
                            on_open_preview: move |_| {
                                let Some(RowSelection::File(index)) = selection() else {
                                    return;
                                };
                                let (Some(archive), Some(packet)) = (archive(), packet()) else {
                                    return;
                                };
                                let archive =
                                    archive_with_ptx_infos(archive, &ptx_infos.read());
                                preview_open.set(true);
                                request_preview(
                                    archive,
                                    packet,
                                    index,
                                    PreviewQuality::Detail,
                                    detail_preview,
                                    preview_cache,
                                    status,
                                );
                            },
                            on_preview_context_menu: move |(preview, x, y)| {
                                context_menu.set(None);
                                preview_context_menu.set(Some(PtxPreviewContextMenuState {
                                    preview,
                                    x,
                                    y,
                                }));
                            },
                        }
                    }
                }

                if let (Some(menu), Some(document)) =
                    (context_menu_snapshot, archive_snapshot.as_ref())
                {
                    ArchiveContextMenu {
                        state: menu,
                        archive: document.clone(),
                        edits: packet_edits.read().clone(),
                        virtual_directories: virtual_directories_snapshot.clone(),
                        packet: packet_snapshot.clone(),
                        on_close: move |_| context_menu.set(None),
                        on_open: move |target| {
                            context_menu.set(None);
                            match target {
                                RowSelection::Packet(index) => enter_packet(
                                    index,
                                    archive,
                                    packet,
                                    packet_edits,
                                    navigation,
                                    status,
                                ),
                                RowSelection::Directory(directory) => {
                                    enter_directory(directory, navigation);
                                }
                                RowSelection::File(index) => open_file_item(
                                    index,
                                    archive,
                                    packet,
                                    selection,
                                    ptx_infos,
                                    ptx_preview,
                                    detail_preview,
                                    preview_cache,
                                    preview_open,
                                    status,
                                    on_open_rton,
                                ),
                            }
                        },
                        on_add: move |(files, directory)| {
                            context_menu.set(None);
                            open_file_import(files, directory, edit_dialog);
                            processing::begin_request();
                            ptx_preview.set(None);
                            detail_preview.set(None);
                        },
                        on_create_folder: move |parent| {
                            context_menu.set(None);
                            let Some(packet) = packet() else {
                                return;
                            };
                            let directories = virtual_directories
                                .read()
                                .get(&packet.record.index)
                                .cloned()
                                .unwrap_or_default();
                            edit_dialog.set(Some(create_folder_dialog(
                                parent,
                                &packet,
                                &directories,
                            )));
                        },
                        on_replace: move |file| {
                            context_menu.set(None);
                            replace_selected_file(
                                file,
                                archive,
                                packet,
                                selection,
                                packet_edits,
                                ptx_infos,
                                status,
                            );
                            processing::begin_request();
                            ptx_preview.set(None);
                            detail_preview.set(None);
                        },
                        on_replace_rsg: move |file| {
                            context_menu.set(None);
                            replace_selected_rsg(
                                file,
                                archive,
                                packet,
                                selection,
                                packet_edits,
                                status,
                            );
                        },
                        on_rename: move |target| {
                            context_menu.set(None);
                            if let Some(dialog) =
                                rename_dialog(target, archive(), packet(), &packet_edits.read())
                            {
                                edit_dialog.set(Some(dialog));
                            }
                        },
                        on_delete: move |target| {
                            context_menu.set(None);
                            if let Some(dialog) =
                                delete_dialog(target, archive(), packet(), &packet_edits.read())
                            {
                                edit_dialog.set(Some(dialog));
                            }
                        },
                        on_properties: move |target| {
                            context_menu.set(None);
                            if let Some(draft) =
                                property_draft(
                                    target,
                                    archive(),
                                    packet(),
                                    &packet_edits.read(),
                                    &ptx_infos.read(),
                                )
                            {
                                edit_dialog.set(Some(EditDialog::Properties(draft)));
                            }
                        },
                        on_extract: move |target| {
                            selection.set(Some(target));
                            context_menu.set(None);
                            spawn(async move {
                                extract_current(
                                    archive,
                                    packet,
                                    location,
                                    selection,
                                    packet_edits,
                                    status,
                                )
                                .await;
                            });
                        },
                        on_export_png: move |target| {
                            context_menu.set(None);
                            spawn(async move {
                                export_ptx_png(
                                    Some(target),
                                    archive,
                                    packet,
                                    ptx_infos,
                                    status,
                                )
                                .await;
                            });
                        },
                        on_error: move |error| {
                            context_menu.set(None);
                            set_status(
                                status,
                                AppStatus::new(
                                    format!("编辑操作失败：{error}"),
                                    StatusTone::Error,
                                ),
                            );
                        },
                    }
                }

                if dragging() {
                    div { class: "rsb-drop-overlay",
                        Glyph { name: "archive" }
                        strong { "释放以打开 RSB 归档" }
                        span { "文件只在本地处理" }
                    }
                }

                if preview_open() {
                    match detail_preview_snapshot.clone() {
                        Some(PtxPreviewState::Ready(decoded)) => rsx! {
                            PreviewModal {
                                preview: decoded,
                                loading: false,
                                on_close: move |_| {
                                    preview_context_menu.set(None);
                                    preview_open.set(false);
                                },
                                on_context_menu: move |(preview, x, y)| {
                                    context_menu.set(None);
                                    preview_context_menu.set(Some(PtxPreviewContextMenuState {
                                        preview,
                                        x,
                                        y,
                                    }));
                                },
                                on_export: move |file_index| {
                                    spawn(export_ptx_png(
                                        Some(RowSelection::File(file_index)),
                                        archive,
                                        packet,
                                        ptx_infos,
                                        status,
                                    ));
                                },
                            }
                        },
                        Some(PtxPreviewState::Loading { .. }) => {
                            let fallback = match ptx_preview() {
                                Some(PtxPreviewState::Ready(preview)) => Some(preview),
                                _ => None,
                            };
                            rsx! {
                                PreviewLoadingModal {
                                    preview: fallback,
                                    on_close: move |_| {
                                        processing::begin_request();
                                        detail_preview.set(None);
                                        preview_context_menu.set(None);
                                        preview_open.set(false);
                                    },
                                    on_context_menu: move |(preview, x, y)| {
                                        context_menu.set(None);
                                        preview_context_menu.set(Some(
                                            PtxPreviewContextMenuState {
                                                preview,
                                                x,
                                                y,
                                            },
                                        ));
                                    },
                                    on_export: move |file_index| {
                                        spawn(export_ptx_png(
                                            Some(RowSelection::File(file_index)),
                                            archive,
                                            packet,
                                            ptx_infos,
                                            status,
                                        ));
                                    },
                                }
                            }
                        },
                        Some(PtxPreviewState::Error { message, .. }) => rsx! {
                            PreviewErrorModal {
                                message,
                                on_close: move |_| preview_open.set(false),
                            }
                        },
                        None => rsx! {
                            PreviewLoadingModal {
                                preview: None,
                                on_close: move |_| {
                                    preview_context_menu.set(None);
                                    preview_open.set(false);
                                },
                                on_context_menu: move |(preview, x, y)| {
                                    context_menu.set(None);
                                    preview_context_menu.set(Some(PtxPreviewContextMenuState {
                                        preview,
                                        x,
                                        y,
                                    }));
                                },
                                on_export: move |file_index| {
                                    spawn(export_ptx_png(
                                        Some(RowSelection::File(file_index)),
                                        archive,
                                        packet,
                                        ptx_infos,
                                        status,
                                    ));
                                },
                            }
                        }
                    }
                }

                if let Some(menu) = preview_context_menu_snapshot.clone() {
                    PtxPreviewContextMenu {
                        state: menu,
                        on_close: move |_| preview_context_menu.set(None),
                        on_copy: move |preview| {
                            preview_context_menu.set(None);
                            spawn(copy_preview_png(preview, status));
                        },
                        on_export: move |file_index| {
                            preview_context_menu.set(None);
                            spawn(export_ptx_png(
                                Some(RowSelection::File(file_index)),
                                archive,
                                packet,
                                ptx_infos,
                                status,
                            ));
                        },
                    }
                }

                if let Some(dialog) = edit_dialog() {
                    EditArchiveDialog {
                        key: "{edit_dialog_key}",
                        dialog,
                        on_cancel: move |_| edit_dialog.set(None),
                        on_confirm: move |dialog| {
                            match dialog {
                                EditDialog::ImportFiles(draft) => confirm_file_import(
                                    draft,
                                    archive,
                                    packet,
                                    packet_edits,
                                    ptx_infos,
                                    edit_dialog,
                                    status,
                                ),
                                dialog => {
                                    edit_dialog.set(None);
                                    match dialog {
                                        EditDialog::CreateRsg(draft) => create_rsg_packet(
                                            draft,
                                            archive,
                                            packet_edits,
                                            removed_packets,
                                            selection,
                                            status,
                                        ),
                                        EditDialog::CreateFolder(draft) => create_folder(
                                            draft,
                                            packet,
                                            virtual_directories,
                                            selection,
                                            status,
                                        ),
                                        EditDialog::AddTexture(draft) => {
                                            spawn(async move {
                                                add_texture(
                                                    draft,
                                                    archive,
                                                    packet,
                                                    packet_edits,
                                                    ptx_infos,
                                                    status,
                                                )
                                                .await;
                                            });
                                        }
                                        EditDialog::Rename { target, value } => rename_selected(
                                            target,
                                            value,
                                            archive,
                                            packet,
                                            selection,
                                            packet_edits,
                                            removed_packets,
                                            virtual_directories,
                                            status,
                                        ),
                                        EditDialog::Delete { target, .. } => delete_selected(
                                            target,
                                            archive,
                                            packet,
                                            selection,
                                            packet_edits,
                                            removed_packets,
                                            location,
                                            ptx_infos,
                                            virtual_directories,
                                            status,
                                        ),
                                        EditDialog::Properties(draft) => apply_properties(
                                            draft,
                                            archive,
                                            packet,
                                            packet_edits,
                                            removed_packets,
                                            ptx_infos,
                                            status,
                                        ),
                                        EditDialog::ImportFiles(_) => unreachable!(),
                                    }
                                }
                            }
                            processing::begin_request();
                            ptx_preview.set(None);
                            detail_preview.set(None);
                        },
                    }
                }

                if let Some(document) = pending_archive() {
                    UnsavedChangesDialog {
                        name: document.display_name.clone(),
                        on_cancel: move |_| pending_archive.set(None),
                        on_discard: move |_| {
                            let Some(document) = pending_archive.take() else {
                                return;
                            };
                            install_editable_archive(
                                document,
                                archive,
                                packet,
                                location,
                                selection,
                                query,
                                status,
                                packet_edits,
                                removed_packets,
                                ptx_infos,
                            );
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn ArchiveToolbar(
    archive_name: String,
    has_archive: bool,
    has_unsaved_changes: bool,
    in_packet: bool,
    can_properties: bool,
    can_replace: bool,
    can_replace_rsg: bool,
    can_rename: bool,
    can_delete: bool,
    can_export_png: bool,
    can_go_up: bool,
    tree_visible: bool,
    inspector_visible: bool,
    on_open: EventHandler<ArchiveDocument>,
    on_error: EventHandler<String>,
    on_up: EventHandler<()>,
    on_extract: EventHandler<()>,
    on_export_png: EventHandler<()>,
    on_save: EventHandler<()>,
    on_save_as: EventHandler<()>,
    on_add: EventHandler<Vec<AddedFile>>,
    on_create_folder: EventHandler<()>,
    on_add_rsg: EventHandler<()>,
    on_replace: EventHandler<AddedFile>,
    on_replace_rsg: EventHandler<AddedFile>,
    on_rename: EventHandler<()>,
    on_delete: EventHandler<()>,
    on_properties: EventHandler<()>,
    on_toggle_tree: EventHandler<()>,
    on_toggle_inspector: EventHandler<()>,
) -> Element {
    let mut more_open = use_signal(|| false);
    let more_open_snapshot = more_open();

    rsx! {
        div { class: "ui-island ui-tool-page-actions rsb-page-actions",
            button {
                r#type: "button",
                class: if tree_visible { "rsb-tool-button rsb-tool-button--icon is-active" } else { "rsb-tool-button rsb-tool-button--icon" },
                disabled: !has_archive,
                title: "归档目录",
                aria_label: "显示或隐藏归档目录",
                aria_pressed: tree_visible,
                onclick: move |_| on_toggle_tree.call(()),
                Glyph { name: "tree" }
            }
            div { class: "rsb-toolbar-group",
                OpenArchiveButton {
                    on_open,
                    on_error,
                    primary: true,
                    icon_only: true,
                }
                button {
                    r#type: "button",
                    class: if has_unsaved_changes { "rsb-tool-button rsb-tool-button--icon is-dirty" } else { "rsb-tool-button rsb-tool-button--icon" },
                    disabled: !has_archive || !has_unsaved_changes,
                    title: "保存",
                    aria_label: "保存",
                    onclick: move |_| on_save.call(()),
                    Glyph { name: "save" }
                }
            }
            div { class: "rsb-document-pill", title: "{archive_name}",
                span { class: "rsb-document-dot" }
                span { class: "rsb-document-name", "{archive_name}" }
            }
            if has_archive {
                div { class: "rsb-toolbar-spacer" }
                button {
                    r#type: "button",
                    class: "rsb-tool-button rsb-tool-button--icon",
                    disabled: !can_go_up,
                    title: "上一级",
                    aria_label: "上一级",
                    onclick: move |_| on_up.call(()),
                    Glyph { name: "up" }
                }
                if in_packet {
                    button {
                        r#type: "button",
                        class: "rsb-tool-button rsb-tool-button--icon",
                        title: "新建文件夹",
                        aria_label: "新建文件夹",
                        onclick: move |_| on_create_folder.call(()),
                        Glyph { name: "folder-add" }
                    }
                    AddFilesButton { enabled: true, on_add, on_error }
                } else {
                    button {
                        r#type: "button",
                        class: "rsb-tool-button rsb-tool-button--icon",
                        disabled: !has_archive,
                        title: "添加 RSG",
                        aria_label: "添加 RSG",
                        onclick: move |_| on_add_rsg.call(()),
                        Glyph { name: "add" }
                    }
                }
                if can_replace_rsg {
                    ReplaceRsgButton {
                        enabled: true,
                        on_pick: on_replace_rsg,
                        on_error,
                    }
                } else {
                    ReplaceFileButton { enabled: can_replace, on_replace, on_error }
                }
                div { class: "rsb-toolbar-spacer" }
                button {
                    r#type: "button",
                    class: "rsb-tool-button rsb-tool-button--icon",
                    title: "提取",
                    aria_label: "提取所选项目",
                    onclick: move |_| on_extract.call(()),
                    Glyph { name: "extract" }
                }
                button {
                    r#type: "button",
                    class: if more_open_snapshot { "rsb-tool-button rsb-tool-button--icon is-active" } else { "rsb-tool-button rsb-tool-button--icon" },
                    title: "更多",
                    aria_label: "更多",
                    aria_expanded: more_open_snapshot,
                    onclick: move |_| more_open.toggle(),
                    Glyph { name: "more" }
                }
                button {
                    r#type: "button",
                    class: if inspector_visible { "rsb-tool-button rsb-tool-button--icon is-active" } else { "rsb-tool-button rsb-tool-button--icon" },
                    title: "详情",
                    aria_label: "显示或隐藏详情",
                    aria_pressed: inspector_visible,
                    onclick: move |_| on_toggle_inspector.call(()),
                    Glyph { name: "panel" }
                }
            }
        }
        if more_open_snapshot {
            div {
                class: "rsb-action-sheet-layer",
                tabindex: "-1",
                onmounted: move |event| async move {
                    let _ = event.set_focus(true).await;
                },
                onkeydown: move |event| {
                    if event.key() == Key::Escape {
                        event.prevent_default();
                        more_open.set(false);
                    }
                },
                onclick: move |_| more_open.set(false),
                div { class: "rsb-action-sheet-backdrop", aria_hidden: "true" }
                section {
                    class: "rsb-action-sheet",
                    role: "dialog",
                    aria_modal: "true",
                    aria_labelledby: "rsb-more-title",
                    onclick: move |event| event.stop_propagation(),
                    header { class: "rsb-action-sheet-header",
                        strong { id: "rsb-more-title", "更多" }
                        button {
                            r#type: "button",
                            class: "rsb-tool-button rsb-tool-button--icon",
                            title: "关闭更多操作",
                            aria_label: "关闭更多操作",
                            onclick: move |_| more_open.set(false),
                            Glyph { name: "close" }
                        }
                    }
                    div { class: "rsb-action-sheet-groups",
                        section { class: "rsb-action-group",
                            div { class: "rsb-action-group-heading",
                                strong { "归档" }
                                span { "保存与副本" }
                            }
                            div { class: "rsb-action-row",
                                button {
                                    r#type: "button",
                                    class: "rsb-action-button",
                                    onclick: move |_| {
                                        more_open.set(false);
                                        on_save_as.call(());
                                    },
                                    Glyph { name: "save-as" }
                                    span { "另存为" }
                                }
                            }
                        }
                        section { class: "rsb-action-group",
                            div { class: "rsb-action-group-heading",
                                strong { "所选项目" }
                                span { "编辑名称、属性或内容" }
                            }
                            div { class: "rsb-action-row",
                                button {
                                    r#type: "button",
                                    class: "rsb-action-button",
                                    disabled: !can_rename,
                                    onclick: move |_| {
                                        more_open.set(false);
                                        on_rename.call(());
                                    },
                                    Glyph { name: "rename" }
                                    span { "重命名" }
                                }
                                button {
                                    r#type: "button",
                                    class: "rsb-action-button",
                                    disabled: !can_properties,
                                    onclick: move |_| {
                                        more_open.set(false);
                                        on_properties.call(());
                                    },
                                    Glyph { name: "properties" }
                                    span { "属性" }
                                }
                                button {
                                    r#type: "button",
                                    class: "rsb-action-button rsb-action-button--danger",
                                    disabled: !can_delete,
                                    onclick: move |_| {
                                        more_open.set(false);
                                        on_delete.call(());
                                    },
                                    Glyph { name: "delete" }
                                    span { "删除" }
                                }
                            }
                        }
                        section { class: "rsb-action-group",
                            div { class: "rsb-action-group-heading",
                                strong { "纹理输出" }
                                span { "将所选 PTX 解码为原始尺寸图片" }
                            }
                            div { class: "rsb-action-row",
                                button {
                                    r#type: "button",
                                    class: "rsb-action-button",
                                    disabled: !can_export_png,
                                    onclick: move |_| {
                                        more_open.set(false);
                                        on_export_png.call(());
                                    },
                                    Glyph { name: "image" }
                                    span { "导出 PNG" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn OpenArchiveButton(
    on_open: EventHandler<ArchiveDocument>,
    on_error: EventHandler<String>,
    #[props(default)] primary: bool,
    #[props(default)] icon_only: bool,
) -> Element {
    let class = match (primary, icon_only) {
        (true, true) => "rsb-tool-button rsb-tool-button--primary rsb-tool-button--icon",
        (true, false) => "rsb-tool-button rsb-tool-button--primary",
        (false, true) => "rsb-tool-button rsb-tool-button--icon",
        (false, false) => "rsb-tool-button",
    };
    rsx! {
        button {
            r#type: "button",
            class,
            title: "打开 RSB",
            aria_label: "打开 RSB 归档",
            onclick: move |_| async move {
                let Some(path) = platform::pick_archive().await else {
                    return;
                };
                match processing::open_native(path).await {
                    Ok(document) => on_open.call(document),
                    Err(error) => on_error.call(error),
                }
            },
            Glyph { name: "open" }
            if !icon_only {
                span { "打开" }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn OpenArchiveButton(
    on_open: EventHandler<ArchiveDocument>,
    on_error: EventHandler<String>,
    #[props(default)] primary: bool,
    #[props(default)] icon_only: bool,
) -> Element {
    let class = match (primary, icon_only) {
        (true, true) => "rsb-tool-button rsb-tool-button--primary rsb-tool-button--icon",
        (true, false) => "rsb-tool-button rsb-tool-button--primary",
        (false, true) => "rsb-tool-button rsb-tool-button--icon",
        (false, false) => "rsb-tool-button",
    };
    rsx! {
        label {
            class,
            title: "打开 RSB",
            aria_label: "打开 RSB 归档",
            input {
                class: "rsb-file-input",
                r#type: "file",
                accept: ".rsb,application/octet-stream",
                onchange: move |event| async move {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };
                    match open_file_data(file).await {
                        Ok(document) => on_open.call(document),
                        Err(error) => on_error.call(error),
                    }
                },
            }
            Glyph { name: "open" }
            if !icon_only {
                span { "打开" }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn AddFilesButton(
    enabled: bool,
    on_add: EventHandler<Vec<AddedFile>>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "rsb-tool-button rsb-tool-button--icon",
            disabled: !enabled,
            title: "添加文件或纹理",
            aria_label: "添加文件或纹理",
            onclick: move |_| async move { match platform::pick_files().await {
                Ok(files) => on_add.call(files),
                Err(error) => on_error.call(error),
            }},
            Glyph { name: "add" }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn AddFilesButton(
    enabled: bool,
    on_add: EventHandler<Vec<AddedFile>>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: if enabled { "rsb-tool-button rsb-tool-button--icon" } else { "rsb-tool-button rsb-tool-button--icon is-disabled" },
            title: "添加文件或纹理",
            aria_label: "添加文件或纹理",
            input {
                class: "rsb-file-input",
                r#type: "file",
                multiple: true,
                disabled: !enabled,
                onchange: move |event| async move {
                    let mut added = Vec::new();
                    for file in event.files() {
                        match file.read_bytes().await {
                            Ok(bytes) => added.push(AddedFile {
                                name: file.name(),
                                data: bytes.as_ref().to_vec(),
                            }),
                            Err(error) => {
                                on_error.call(error.to_string());
                                return;
                            }
                        }
                    }
                    on_add.call(added);
                },
            }
            Glyph { name: "add" }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ReplaceFileButton(
    enabled: bool,
    on_replace: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "rsb-tool-button rsb-tool-button--icon",
            disabled: !enabled,
            title: "替换内容",
            aria_label: "替换内容",
            onclick: move |_| async move { match platform::pick_replacement().await {
                Ok(Some(file)) => on_replace.call(file),
                Ok(None) => {}
                Err(error) => on_error.call(error),
            }},
            Glyph { name: "replace" }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ReplaceFileButton(
    enabled: bool,
    on_replace: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: if enabled { "rsb-tool-button rsb-tool-button--icon" } else { "rsb-tool-button rsb-tool-button--icon is-disabled" },
            title: "替换内容",
            aria_label: "替换内容",
            input {
                class: "rsb-file-input",
                r#type: "file",
                disabled: !enabled,
                onchange: move |event| async move {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };
                    match file.read_bytes().await {
                        Ok(bytes) => on_replace.call(AddedFile {
                            name: file.name(),
                            data: bytes.as_ref().to_vec(),
                        }),
                        Err(error) => on_error.call(error.to_string()),
                    }
                },
            }
            Glyph { name: "replace" }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ReplaceRsgButton(
    enabled: bool,
    on_pick: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "rsb-tool-button rsb-tool-button--icon",
            disabled: !enabled,
            title: "替换 RSG",
            aria_label: "替换 RSG",
            onclick: move |_| async move { match platform::pick_rsg_replacement().await {
                Ok(Some(file)) => on_pick.call(file),
                Ok(None) => {}
                Err(error) => on_error.call(error),
            }},
            Glyph { name: "replace" }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ReplaceRsgButton(
    enabled: bool,
    on_pick: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: if !enabled {
                "rsb-tool-button rsb-tool-button--icon is-disabled"
            } else {
                "rsb-tool-button rsb-tool-button--icon"
            },
            title: "替换 RSG",
            aria_label: "替换 RSG",
            input {
                class: "rsb-file-input",
                r#type: "file",
                accept: ".rsg,application/octet-stream",
                disabled: !enabled,
                onchange: move |event| async move {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };
                    match file.read_bytes().await {
                        Ok(bytes) => on_pick.call(AddedFile {
                            name: file.name(),
                            data: bytes.as_ref().to_vec(),
                        }),
                        Err(error) => on_error.call(error.to_string()),
                    }
                },
            }
            Glyph { name: "replace" }
        }
    }
}

#[component]
fn AddressBar(
    archive: Arc<ArchiveDocument>,
    packet: Option<Arc<PacketDocument>>,
    location: BrowserLocation,
    query: String,
    on_archive: EventHandler<()>,
    on_directory: EventHandler<Vec<String>>,
    on_query: EventHandler<String>,
) -> Element {
    let packet_name = packet.as_ref().map(|value| value.record.info.name.clone());
    let directory = match &location {
        BrowserLocation::Archive => Vec::new(),
        BrowserLocation::Packet { directory, .. } => directory.clone(),
    };
    rsx! {
        div { class: "rsb-address-row",
            div { class: "rsb-address-bar", aria_label: "当前位置",
                Glyph { name: "archive" }
                button {
                    r#type: "button",
                    onclick: move |_| on_archive.call(()),
                    "{archive.display_name}"
                }
                if let Some(name) = packet_name {
                    span { class: "rsb-address-separator", "›" }
                    button {
                        r#type: "button",
                        onclick: move |_| on_directory.call(Vec::new()),
                        "{name}"
                    }
                }
                for (index, component) in directory.iter().enumerate() {
                    span { class: "rsb-address-separator", "›" }
                    button {
                        r#type: "button",
                        onclick: {
                            let path = directory[..=index].to_vec();
                            move |_| on_directory.call(path.clone())
                        },
                        "{component}"
                    }
                }
            }
            label { class: "rsb-search",
                Glyph { name: "search" }
                input {
                    r#type: "search",
                    value: "{query}",
                    placeholder: "搜索当前位置",
                    aria_label: "搜索当前位置",
                    oninput: move |event| on_query.call(event.value()),
                }
                if !query.is_empty() {
                    button {
                        r#type: "button",
                        aria_label: "清除搜索",
                        onclick: move |_| on_query.call(String::new()),
                        "×"
                    }
                }
            }
        }
    }
}

#[component]
fn ArchiveTree(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed_packets: RemovedPackets,
    virtual_directories: BTreeSet<Vec<String>>,
    packet: Option<Arc<PacketDocument>>,
    location: BrowserLocation,
    on_archive: EventHandler<()>,
    on_packet: EventHandler<usize>,
    on_directory: EventHandler<Vec<String>>,
) -> Element {
    let visible_packet_count = editing::visible_packet_count(&archive, &edits, &removed_packets);
    let in_archive = matches!(location, BrowserLocation::Archive);
    let current_directory = match &location {
        BrowserLocation::Packet { directory, .. } => directory.clone(),
        BrowserLocation::Archive => Vec::new(),
    };
    let packet_index = packet.as_ref().map(|value| value.record.index);
    let tree_rows = use_memo(use_reactive(
        &PacketTreeSource {
            index: packet
                .as_ref()
                .map(|document| document.directory_index.clone()),
            current_directory: current_directory.clone(),
            virtual_directories,
        },
        |source| source.rows(),
    ));
    let tree_rows = tree_rows.read().clone();
    rsx! {
        div { class: "rsb-tree-sheet",
            header { class: "rsb-panel-header",
                div { class: "rsb-panel-header-title",
                    span { class: "rsb-panel-header-icon", Glyph { name: "archive" } }
                    h2 { "归档目录" }
                }
                p { "{archive.display_name} · {visible_packet_count} 个 RSG 包" }
            }
            nav { class: "rsb-tree-list",
                button {
                    r#type: "button",
                    class: if in_archive { "rsb-tree-item is-active" } else { "rsb-tree-item" },
                    onclick: move |_| on_archive.call(()),
                    Glyph { name: "archive" }
                    span { "{archive.display_name}" }
                    small { "{visible_packet_count}" }
                }
                if let Some(document) = packet {
                    button {
                        r#type: "button",
                        class: if !in_archive && current_directory.is_empty() { "rsb-tree-item rsb-tree-item--child is-active" } else { "rsb-tree-item rsb-tree-item--child" },
                        onclick: move |_| on_packet.call(document.record.index),
                        Glyph { name: "package" }
                        span { "{document.record.info.name}" }
                    }
                    for row in tree_rows.iter() {
                        {
                            let path = row.path.clone();
                            let key = path.join("/");
                            let title = key.clone();
                            let class = if row.is_current {
                                "rsb-tree-item rsb-tree-item--directory is-active"
                            } else if row.is_expanded {
                                "rsb-tree-item rsb-tree-item--directory is-expanded"
                            } else {
                                "rsb-tree-item rsb-tree-item--directory"
                            };
                            let padding = 9 + row.depth.min(9) * 12;
                            rsx! {
                                button {
                                    key: "{key}",
                                    r#type: "button",
                                    class,
                                    style: "padding-left: {padding}px",
                                    aria_current: row.is_current.then_some("page"),
                                    title,
                                    onclick: move |_| on_directory.call(path.clone()),
                                    Glyph { name: "folder" }
                                    span { "{row.name}" }
                                    small { "{row.file_count}" }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "rsb-tree-placeholder",
                        span { "选择并打开一个 RSG 包后，目录会显示在这里。" }
                    }
                }
                if packet_index.is_none() && visible_packet_count > 0 {
                    div { class: "rsb-tree-summary",
                        strong { "{visible_packet_count}" }
                        span { "RSG packets" }
                    }
                }
            }
        }
    }
}

#[component]
fn ArchiveTable(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed_packets: RemovedPackets,
    virtual_directories: BTreeSet<Vec<String>>,
    packet: Option<Arc<PacketDocument>>,
    location: BrowserLocation,
    selection: Option<RowSelection>,
    query: String,
    restore_scroll: ScrollRestore,
    on_scroll: EventHandler<f64>,
    on_select: EventHandler<RowSelection>,
    on_context_menu: EventHandler<(RowSelection, f64, f64)>,
    on_blank_context_menu: EventHandler<(Vec<String>, f64, f64)>,
    on_packet: EventHandler<usize>,
    on_directory: EventHandler<Vec<String>>,
    on_file: EventHandler<usize>,
) -> Element {
    let mut mounted = use_signal(|| None::<MountedEvent>);
    let mut scroll_top = use_signal(|| 0_f64);
    let mut viewport_height = use_signal(|| TABLE_DEFAULT_VIEWPORT_HEIGHT);
    let blank_context_directory = match &location {
        BrowserLocation::Packet { directory, .. } => Some(directory.clone()),
        BrowserLocation::Archive => None,
    };

    let archive_rows = use_memo(use_reactive(
        &ArchiveRowSource::new(&archive, edits, removed_packets),
        |source| source.build_rows(),
    ));
    let archive_rows = archive_rows.read().clone();
    let archive_indices = use_memo(use_reactive(
        &ArchiveRowFilter {
            rows: archive_rows.clone(),
            query: query.clone(),
        },
        |filter| filter.indices(),
    ));
    let archive_indices = archive_indices.read().clone();

    let directory = match &location {
        BrowserLocation::Packet { directory, .. } => directory.clone(),
        BrowserLocation::Archive => Vec::new(),
    };
    let packet_items = use_memo(use_reactive(
        &PacketItemSource {
            index: packet
                .as_ref()
                .map(|document| document.directory_index.clone()),
            directory,
            virtual_directories,
            query,
        },
        |source| source.items(),
    ));
    let packet_items = packet_items.read().clone();
    let row_count = match location {
        BrowserLocation::Archive => archive_indices.len(),
        BrowserLocation::Packet { .. } => packet_items.len(),
    };
    let scroll_top_snapshot = *scroll_top.read();
    let virtual_window =
        table_virtual_window(row_count, scroll_top_snapshot, *viewport_height.read());

    use_effect(use_reactive(&restore_scroll, move |restore| {
        if restore.revision == 0 {
            return;
        }
        scroll_top.set(restore.top);
        let Some(event) = mounted.peek().clone() else {
            return;
        };
        spawn(async move {
            let _ = event
                .scroll(
                    PixelsVector2D::new(0.0, restore.top),
                    ScrollBehavior::Instant,
                )
                .await;
        });
    }));
    rsx! {
        main {
            class: "rsb-table-pane",
            onmounted: move |event| {
                mounted.set(Some(event.clone()));
                async move {
                    if let Ok(rect) = event.get_client_rect().await {
                        viewport_height.set(measured_table_viewport_height(rect.height()));
                    }
                }
            },
            onresize: move |event| {
                if let Ok(size) = event.get_content_box_size() {
                    viewport_height.set(measured_table_viewport_height(size.height));
                }
            },
            onscroll: move |event| {
                let top = event.scroll_top();
                scroll_top.set(top);
                on_scroll.call(top);
            },
            div { class: "rsb-table-head",
                span { class: "rsb-col-name", "名称" }
                span { class: "rsb-col-type", "类型" }
                span { class: "rsb-col-size", "原始大小" }
                span { class: "rsb-col-packed", "压缩后" }
                span { class: "rsb-col-ratio", "压缩率" }
            }
            div {
                class: "rsb-table-body",
                oncontextmenu: move |event| {
                    let Some(directory) = blank_context_directory.clone() else {
                        return;
                    };
                    event.prevent_default();
                    let point = event.client_coordinates();
                    on_blank_context_menu.call((directory, point.x, point.y));
                },
                div {
                    class: "rsb-table-virtual-space",
                    style: "height: {virtual_window.content_height}px",
                    match location {
                        BrowserLocation::Archive => rsx! {
                            for visual_index in virtual_window.start..virtual_window.end {
                                if let Some(row) = archive_indices
                                    .get(visual_index)
                                    .and_then(|row_index| archive_rows.get(*row_index))
                                    .cloned()
                                {
                                    div {
                                        key: "packet-{row.index}",
                                        class: "rsb-table-virtual-row",
                                        style: "transform: translateY({table_row_top(visual_index)}px)",
                                        ArchivePacketRow {
                                            row: row.clone(),
                                            selected: selection == Some(RowSelection::Packet(row.index)),
                                            on_select: move |_| on_select.call(RowSelection::Packet(row.index)),
                                            on_open: move |_| on_packet.call(row.index),
                                            on_context_menu: move |(x, y)| {
                                                on_context_menu.call((RowSelection::Packet(row.index), x, y))
                                            },
                                        }
                                    }
                                }
                            }
                        },
                        BrowserLocation::Packet { .. } => {
                            let files = packet.as_ref().map(|value| value.files.clone());
                            rsx! {
                                for visual_index in virtual_window.start..virtual_window.end {
                                    if let Some(item) = packet_items.get(visual_index).cloned() {
                                        match item {
                                        BrowserItem::Directory { name, path, file_count, byte_len } => {
                                            let is_selected = selection == Some(RowSelection::Directory(path.clone()));
                                            rsx! {
                                                div {
                                                    key: "directory-{path:?}",
                                                    class: "rsb-table-virtual-row",
                                                    style: "transform: translateY({table_row_top(visual_index)}px)",
                                            button {
                                                r#type: "button",
                                                class: if is_selected { "rsb-table-row is-selected" } else { "rsb-table-row" },
                                                onclick: {
                                                    let path = path.clone();
                                                    move |_| on_select.call(RowSelection::Directory(path.clone()))
                                                },
                                                oncontextmenu: {
                                                    let path = path.clone();
                                                    move |event| {
                                                        event.prevent_default();
                                                        event.stop_propagation();
                                                        let point = event.client_coordinates();
                                                        on_context_menu.call((
                                                            RowSelection::Directory(path.clone()),
                                                            point.x,
                                                            point.y,
                                                        ));
                                                    }
                                                },
                                                ondoubleclick: move |_| on_directory.call(path.clone()),
                                                span { class: "rsb-name-cell",
                                                    span { class: "rsb-file-icon rsb-file-icon--folder", Glyph { name: "folder" } }
                                                    span {
                                                        strong { "{name}" }
                                                        small { "{file_count} 个文件" }
                                                    }
                                                }
                                                span { class: "rsb-col-type", "文件夹" }
                                                span { class: "rsb-col-size rsb-number", "{format_bytes(byte_len)}" }
                                                span { class: "rsb-col-packed rsb-number", "—" }
                                                span { class: "rsb-col-ratio rsb-number", "—" }
                                            }
                                                }
                                            }
                                        },
                                        BrowserItem::File { name, file_index } => {
                                            let file = files.as_ref().and_then(|values| values.get(file_index));
                                            if let Some(file) = file {
                                                let kind = file_kind(&file.path, file.is_part1);
                                                let size = file.data.len() as u64;
                                                rsx! {
                                                    div {
                                                        key: "file-{file_index}",
                                                        class: "rsb-table-virtual-row",
                                                        style: "transform: translateY({table_row_top(visual_index)}px)",
                                                        button {
                                                            r#type: "button",
                                                            class: if selection == Some(RowSelection::File(file_index)) { "rsb-table-row is-selected" } else { "rsb-table-row" },
                                                            onclick: move |_| on_select.call(RowSelection::File(file_index)),
                                                            oncontextmenu: move |event| {
                                                                event.prevent_default();
                                                                event.stop_propagation();
                                                                let point = event.client_coordinates();
                                                                on_context_menu.call((
                                                                    RowSelection::File(file_index),
                                                                    point.x,
                                                                    point.y,
                                                                ));
                                                            },
                                                            ondoubleclick: move |_| on_file.call(file_index),
                                                            span { class: "rsb-name-cell",
                                                                span { class: "rsb-file-icon", Glyph { name: "file" } }
                                                                span {
                                                                    strong { "{name}" }
                                                                    small { if file.is_part1 { "Part 1 texture" } else { "Part 0 resource" } }
                                                                }
                                                            }
                                                            span { class: "rsb-col-type", "{kind}" }
                                                            span { class: "rsb-col-size rsb-number", "{format_bytes(size)}" }
                                                            span { class: "rsb-col-packed rsb-number", "—" }
                                                            span { class: "rsb-col-ratio rsb-number", "—" }
                                                        }
                                                    }
                                                }
                                            } else {
                                                rsx! {}
                                            }
                                        },
                                        }
                                }
                            }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn ArchivePacketRow(
    row: ArchiveTableRow,
    selected: bool,
    on_select: EventHandler<()>,
    on_open: EventHandler<()>,
    on_context_menu: EventHandler<(f64, f64)>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if selected { "rsb-table-row is-selected" } else { "rsb-table-row" },
            onclick: move |_| on_select.call(()),
            oncontextmenu: move |event| {
                event.prevent_default();
                event.stop_propagation();
                let point = event.client_coordinates();
                on_context_menu.call((point.x, point.y));
            },
            ondoubleclick: move |_| on_open.call(()),
            span { class: "rsb-name-cell",
                span {
                    class: if row.error { "rsb-file-icon rsb-file-icon--error" } else { "rsb-file-icon rsb-file-icon--package" },
                    Glyph { name: "package" }
                }
                span {
                    strong { "{row.name}" }
                    small { "{row.subtitle}" }
                }
            }
            span { class: "rsb-col-type", "{row.compression}" }
            span { class: "rsb-col-size rsb-number", "{row.unpacked_size}" }
            span { class: "rsb-col-packed rsb-number", "{row.stored_size}" }
            span { class: "rsb-col-ratio rsb-number", "{row.ratio}" }
        }
    }
}

#[component]
fn PtxPreviewContextMenu(
    state: PtxPreviewContextMenuState,
    on_close: EventHandler<()>,
    on_copy: EventHandler<PtxPreview>,
    on_export: EventHandler<usize>,
) -> Element {
    let style = format!(
        "--rsb-context-x: {}px; --rsb-context-y: {}px",
        state.x.round(),
        state.y.round()
    );
    let copy_preview = state.preview.clone();
    let export_index = state.preview.file_index;
    rsx! {
        div {
            class: "rsb-context-backdrop rsb-preview-context-backdrop",
            onmousedown: move |event| {
                event.prevent_default();
                on_close.call(());
            },
            oncontextmenu: move |event| {
                event.prevent_default();
                on_close.call(());
            },
        }
        div {
            class: "rsb-context-menu rsb-preview-context-menu",
            role: "menu",
            aria_label: "纹理预览操作",
            tabindex: "0",
            style: "{style}",
            onmousedown: move |event| event.stop_propagation(),
            oncontextmenu: move |event| {
                event.prevent_default();
                event.stop_propagation();
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            button {
                r#type: "button",
                role: "menuitem",
                onclick: move |_| on_copy.call(copy_preview.clone()),
                span { "复制图像" }
            }
            button {
                r#type: "button",
                role: "menuitem",
                onclick: move |_| on_export.call(export_index),
                span { "导出 PNG" }
            }
        }
    }
}

#[component]
fn ArchiveContextMenu(
    state: ArchiveContextMenuState,
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    virtual_directories: BTreeSet<Vec<String>>,
    packet: Option<Arc<PacketDocument>>,
    on_close: EventHandler<()>,
    on_open: EventHandler<RowSelection>,
    on_add: EventHandler<(Vec<AddedFile>, Vec<String>)>,
    on_create_folder: EventHandler<Vec<String>>,
    on_replace: EventHandler<AddedFile>,
    on_replace_rsg: EventHandler<AddedFile>,
    on_rename: EventHandler<RowSelection>,
    on_delete: EventHandler<RowSelection>,
    on_properties: EventHandler<RowSelection>,
    on_extract: EventHandler<RowSelection>,
    on_export_png: EventHandler<RowSelection>,
    on_error: EventHandler<String>,
) -> Element {
    let packet_record = match state.target.as_ref() {
        Some(RowSelection::Packet(index)) => editing::packet_record(&archive, &edits, *index),
        _ => None,
    };
    let has_virtual_directory = match state.target.as_ref() {
        Some(RowSelection::Directory(path)) => virtual_directories
            .iter()
            .any(|candidate| candidate.starts_with(path)),
        _ => false,
    };
    let capabilities = state.target.as_ref().map_or(
        ArchiveContextCapabilities {
            add: packet.is_some(),
            ..Default::default()
        },
        |target| {
            archive_context_capabilities(
                target,
                packet.as_deref(),
                packet_record.as_ref(),
                has_virtual_directory,
            )
        },
    );
    let open_label = state
        .target
        .as_ref()
        .map(|target| archive_context_open_label(target, packet.as_deref()))
        .unwrap_or("打开");
    let directory = match state.target.as_ref() {
        Some(RowSelection::Directory(path)) => path.clone(),
        _ => state.directory.clone(),
    };
    let can_show_delete = matches!(
        state.target.as_ref(),
        Some(RowSelection::Packet(_) | RowSelection::File(_) | RowSelection::Directory(_))
    );
    let style = format!(
        "--rsb-context-x: {}px; --rsb-context-y: {}px",
        state.x.round(),
        state.y.round()
    );
    rsx! {
        div {
            class: "rsb-context-backdrop",
            onmousedown: move |event| {
                event.prevent_default();
                on_close.call(());
            },
            oncontextmenu: move |event| {
                event.prevent_default();
                on_close.call(());
            },
        }
        div {
            class: "rsb-context-menu",
            role: "menu",
            aria_label: "归档项目操作",
            tabindex: "0",
            style: "{style}",
            onmousedown: move |event| {
                event.stop_propagation();
            },
            oncontextmenu: move |event| {
                event.prevent_default();
                event.stop_propagation();
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            if capabilities.open {
                button {
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let target = state.target.clone();
                        move |_| {
                            if let Some(target) = target.clone() {
                                on_open.call(target);
                            }
                        }
                    },
                    span { "{open_label}" }
                }
            }
            if capabilities.add || capabilities.replace {
                div { class: "rsb-context-separator", role: "separator" }
            }
            if capabilities.add {
                button {
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let directory = directory.clone();
                        move |_| on_create_folder.call(directory.clone())
                    },
                    span { "新建文件夹" }
                }
                ContextAddFilesItem {
                    directory: directory.clone(),
                    on_add,
                    on_error,
                }
            }
            if capabilities.replace {
                if matches!(state.target.as_ref(), Some(RowSelection::Packet(_))) {
                    ContextReplaceRsgItem {
                        on_pick: on_replace_rsg,
                        on_error,
                    }
                } else {
                    ContextReplaceFileItem { on_replace, on_error }
                }
            }
            if capabilities.rename || can_show_delete || capabilities.properties {
                div { class: "rsb-context-separator", role: "separator" }
            }
            if capabilities.rename {
                button {
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let target = state.target.clone();
                        move |_| {
                            if let Some(target) = target.clone() {
                                on_rename.call(target);
                            }
                        }
                    },
                    span { "重命名" }
                }
            }
            if can_show_delete {
                button {
                    r#type: "button",
                    role: "menuitem",
                    class: "is-danger",
                    disabled: !capabilities.delete,
                    title: if capabilities.delete { "删除" } else { "该项目当前无法删除" },
                    onclick: {
                        let target = state.target.clone();
                        move |_| {
                            if let Some(target) = target.clone() {
                                on_delete.call(target);
                            }
                        }
                    },
                    span { "删除" }
                }
            }
            if capabilities.properties {
                button {
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let target = state.target.clone();
                        move |_| {
                            if let Some(target) = target.clone() {
                                on_properties.call(target);
                            }
                        }
                    },
                    span { "属性" }
                }
            }
            if capabilities.extract || capabilities.export_png {
                div { class: "rsb-context-separator", role: "separator" }
                if capabilities.export_png {
                    button {
                        r#type: "button",
                        role: "menuitem",
                        onclick: {
                            let target = state.target.clone();
                            move |_| {
                                if let Some(target) = target.clone() {
                                    on_export_png.call(target);
                                }
                            }
                        },
                        span { "导出为 PNG" }
                    }
                }
            }
            if capabilities.extract {
                button {
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let target = state.target.clone();
                        move |_| {
                            if let Some(target) = target.clone() {
                                on_extract.call(target);
                            }
                        }
                    },
                    span { "提取" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ContextAddFilesItem(
    directory: Vec<String>,
    on_add: EventHandler<(Vec<AddedFile>, Vec<String>)>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            role: "menuitem",
            onclick: move |_| {
                let directory = directory.clone();
                async move { match platform::pick_files().await {
                Ok(files) if !files.is_empty() => on_add.call((files, directory.clone())),
                Ok(_) => {}
                Err(error) => on_error.call(error),
                }}
            },
            span { "添加文件或纹理到此处" }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ContextAddFilesItem(
    directory: Vec<String>,
    on_add: EventHandler<(Vec<AddedFile>, Vec<String>)>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "rsb-context-file-item",
            role: "menuitem",
            input {
                class: "rsb-context-file-input",
                r#type: "file",
                multiple: true,
                onchange: move |event| {
                    let directory = directory.clone();
                    async move {
                        let mut added = Vec::new();
                        for file in event.files() {
                            match file.read_bytes().await {
                                Ok(bytes) => added.push(AddedFile {
                                    name: file.name(),
                                    data: bytes.as_ref().to_vec(),
                                }),
                                Err(error) => {
                                    on_error.call(error.to_string());
                                    return;
                                }
                            }
                        }
                        if !added.is_empty() {
                            on_add.call((added, directory));
                        }
                    }
                },
            }
            span { "添加文件或纹理到此处" }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ContextReplaceFileItem(
    on_replace: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            role: "menuitem",
            onclick: move |_| async move { match platform::pick_replacement().await {
                Ok(Some(file)) => on_replace.call(file),
                Ok(None) => {}
                Err(error) => on_error.call(error),
            }},
            span { "替换内容" }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ContextReplaceFileItem(
    on_replace: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "rsb-context-file-item",
            role: "menuitem",
            input {
                class: "rsb-context-file-input",
                r#type: "file",
                onchange: move |event| async move {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };
                    match file.read_bytes().await {
                        Ok(bytes) => on_replace.call(AddedFile {
                            name: file.name(),
                            data: bytes.as_ref().to_vec(),
                        }),
                        Err(error) => on_error.call(error.to_string()),
                    }
                },
            }
            span { "替换内容" }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ContextReplaceRsgItem(
    on_pick: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            role: "menuitem",
            onclick: move |_| async move { match platform::pick_rsg_replacement().await {
                Ok(Some(file)) => on_pick.call(file),
                Ok(None) => {}
                Err(error) => on_error.call(error),
            }},
            span { "替换 RSG" }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ContextReplaceRsgItem(
    on_pick: EventHandler<AddedFile>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            class: "rsb-context-file-item",
            role: "menuitem",
            input {
                class: "rsb-context-file-input",
                r#type: "file",
                accept: ".rsg,application/octet-stream",
                onchange: move |event| async move {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };
                    match file.read_bytes().await {
                        Ok(bytes) => on_pick.call(AddedFile {
                            name: file.name(),
                            data: bytes.as_ref().to_vec(),
                        }),
                        Err(error) => on_error.call(error.to_string()),
                    }
                },
            }
            span { "替换 RSG" }
        }
    }
}

#[component]
fn ChannelOrderControl(
    mode: ArchiveChannelOrderMode,
    detection: ArchiveChannelOrderDetection,
    on_change: EventHandler<ArchiveChannelOrderMode>,
) -> Element {
    let (result, detail, tone) = match detection.inference {
        ArchiveChannelOrderInference::Apple => (
            format!(
                "自动识别为 Apple · {} 个 PVRTC 标记",
                detection.apple_evidence
            ),
            "自动模式将 RGBA8888 按 BGRA 存储顺序处理。".to_string(),
            "apple",
        ),
        ArchiveChannelOrderInference::Rgba => {
            let palette = if detection.disambiguated_palettes > 0 {
                format!(
                    "，其中 {} 个代码 30 已识别为 ETC1 Palette",
                    detection.disambiguated_palettes
                )
            } else {
                String::new()
            };
            (
                format!(
                    "自动识别为 RGBA · {} 个 ETC 标记{palette}",
                    detection.rgba_evidence
                ),
                "自动模式将普通纹理保持为 RGBA 顺序。".to_string(),
                "rgba",
            )
        }
        ArchiveChannelOrderInference::Conflicting => (
            "检测到混合平台格式 · 自动使用 RGBA".to_string(),
            format!(
                "Apple {} 个 · ETC {} 个；可在此手动覆盖。",
                detection.apple_evidence, detection.rgba_evidence
            ),
            "warning",
        ),
        ArchiveChannelOrderInference::Unknown => (
            "没有可用于判断平台的纹理 · 自动使用 RGBA".to_string(),
            "仅有 RGBA8888 等通用格式时无法从数据本身可靠判断。".to_string(),
            "warning",
        ),
    };
    rsx! {
        section { class: "rsb-channel-order-card",
            div { class: "rsb-channel-order-heading",
                div {
                    strong { "纹理通道顺序" }
                    span { class: "rsb-channel-order-result is-{tone}", "{result}" }
                }
            }
            div {
                class: "rsb-channel-order-options",
                role: "group",
                aria_label: "纹理通道顺序",
                button {
                    r#type: "button",
                    class: if mode == ArchiveChannelOrderMode::Auto { "is-selected" } else { "" },
                    aria_pressed: mode == ArchiveChannelOrderMode::Auto,
                    onclick: move |_| on_change.call(ArchiveChannelOrderMode::Auto),
                    "自动"
                }
                button {
                    r#type: "button",
                    class: if mode == ArchiveChannelOrderMode::Rgba { "is-selected" } else { "" },
                    aria_pressed: mode == ArchiveChannelOrderMode::Rgba,
                    onclick: move |_| on_change.call(ArchiveChannelOrderMode::Rgba),
                    "RGBA"
                }
                button {
                    r#type: "button",
                    class: if mode == ArchiveChannelOrderMode::Apple { "is-selected" } else { "" },
                    aria_pressed: mode == ArchiveChannelOrderMode::Apple,
                    onclick: move |_| on_change.call(ArchiveChannelOrderMode::Apple),
                    "Apple"
                }
            }
            p { "{detail}" }
        }
    }
}

#[component]
fn Inspector(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed_packets: RemovedPackets,
    ptx_infos: Signal<Vec<RsbPtxInfo>>,
    packet: Option<Arc<PacketDocument>>,
    location: BrowserLocation,
    selection: Option<RowSelection>,
    preview: Option<PtxPreviewState>,
    on_channel_order_mode: EventHandler<ArchiveChannelOrderMode>,
    on_open_preview: EventHandler<()>,
    on_preview_context_menu: EventHandler<(PtxPreview, f64, f64)>,
) -> Element {
    let visible_packet_count = editing::visible_packet_count(&archive, &edits, &removed_packets);
    let channel_order_detection = use_memo(move || detect_archive_channel_order(&ptx_infos.read()));
    let channel_order_detection = *channel_order_detection.read();
    let ptx_infos = ptx_infos.read();
    rsx! {
        div { class: "rsb-inspector-sheet",
            header { class: "rsb-panel-header",
                div { class: "rsb-panel-header-title",
                    span { class: "rsb-panel-header-icon", Glyph { name: "properties" } }
                    h2 { "详情" }
                }
                p { "当前归档与所选项目" }
            }
            div { class: "rsb-inspector-scroll",
                ChannelOrderControl {
                    mode: archive.channel_order_mode,
                    detection: channel_order_detection,
                    on_change: move |mode| on_channel_order_mode.call(mode),
                }
                if let Some(selected) = selection {
                    match selected {
                    RowSelection::Packet(index) => {
                        if let Some(record) = editing::packet_record(&archive, &edits, index) {
                            rsx! {
                                PacketInspector { record }
                            }
                        } else {
                            rsx! { InspectorEmpty {} }
                        }
                    },
                    RowSelection::Directory(path) => {
                        if let Some(document) = packet.as_ref() {
                            let (file_count, bytes) =
                                document.directory_index.directory_summary(&path);
                            let name = path.last().cloned().unwrap_or_else(|| document.record.info.name.clone());
                            rsx! {
                                InspectorTitle { icon: "folder", name, kind: "文件夹".to_string() }
                                InspectorProperties {
                                    rows: vec![
                                        ("包含文件".into(), file_count.to_string()),
                                        ("总大小".into(), format_bytes(bytes)),
                                        ("路径".into(), path.join("/")),
                                    ]
                                }
                            }
                        } else {
                            rsx! { InspectorEmpty {} }
                        }
                    },
                    RowSelection::File(index) => {
                        if let Some(file) = packet.as_ref().and_then(|value| value.files.get(index)) {
                            let name = file.path.replace('\\', "/").rsplit('/').next().unwrap_or(&file.path).to_string();
                            let mut rows = vec![
                                ("大小".into(), format_bytes(file.data.len() as u64)),
                                ("分区".into(), if file.is_part1 { "Part 1".into() } else { "Part 0".into() }),
                                ("路径".into(), file.path.clone()),
                            ];
                            if let Some(info) = &file.part1_info {
                                rows.push(("纹理 ID".into(), info.id.to_string()));
                                rows.push(("尺寸".into(), format!("{} × {}", info.width, info.height)));
                            }
                            if let Some(metadata) = packet
                                .as_ref()
                                .and_then(|value| archive.texture_metadata(&value.record, file))
                            {
                                let info = ptx_infos
                                    .get(metadata.global_index)
                                    .unwrap_or(&metadata.info);
                                rows.push(("全局 PTX".into(), format!("#{}", metadata.global_index)));
                                rows.push(("格式代码".into(), info.format.to_string()));
                                rows.push(("Pitch".into(), info.pitch.to_string()));
                            }
                            rsx! {
                                InspectorTitle {
                                    icon: "file",
                                    name,
                                    kind: file_kind(&file.path, file.is_part1).to_string(),
                                }
                                InspectorProperties { rows }
                                if let Some(state) = preview
                                    .filter(|state| state.file_index() == index)
                                {
                                    match state {
                                        PtxPreviewState::Loading { .. } => rsx! {
                                            PtxPreviewLoadingCard {}
                                        },
                                        PtxPreviewState::Ready(decoded) => rsx! {
                                            PtxPreviewCard {
                                                preview: decoded,
                                                on_open: move |_| on_open_preview.call(()),
                                                on_context_menu: move |value| {
                                                    on_preview_context_menu.call(value)
                                                },
                                            }
                                        },
                                        PtxPreviewState::Error { message, .. } => rsx! {
                                            div { class: "rsb-preview-error",
                                                Glyph { name: "image" }
                                                strong { "无法预览此纹理" }
                                                p { "{message}" }
                                            }
                                        },
                                    }
                                }
                            }
                        } else {
                            rsx! { InspectorEmpty {} }
                        }
                    },
                    }
                } else {
                    match location {
                        BrowserLocation::Archive => rsx! {
                            InspectorTitle {
                                icon: "archive",
                                name: archive.display_name.clone(),
                                kind: "RSB 归档".to_string(),
                            }
                            InspectorProperties {
                                rows: vec![
                                    ("版本".into(), format!("v{}", archive.header.version)),
                                    ("归档大小".into(), format_bytes(archive.byte_len)),
                                    ("RSG 包".into(), visible_packet_count.to_string()),
                                    ("资源索引".into(), archive.resource_count.to_string()),
                                    ("PTX 纹理".into(), archive.header.ptx_number.to_string()),
                                ]
                            }
                            if !archive.warnings.is_empty() {
                                div { class: "rsb-warning-card",
                                    strong { "索引警告" }
                                    for warning in archive.warnings.iter() {
                                        p { "{warning}" }
                                    }
                                }
                            }
                        },
                        BrowserLocation::Packet { .. } => {
                            if let Some(document) = packet {
                                rsx! { PacketInspector { record: document.record.clone() } }
                            } else {
                                rsx! { InspectorEmpty {} }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn PtxPreviewCard(
    preview: PtxPreview,
    on_open: EventHandler<()>,
    on_context_menu: EventHandler<(PtxPreview, f64, f64)>,
) -> Element {
    let preview_url = preview.asset.url().to_string();
    let context_preview = preview.clone();
    rsx! {
        section { class: "rsb-preview-section",
            div { class: "rsb-preview-heading",
                div {
                    strong { "纹理预览" }
                    span { "{preview.format}" }
                }
                button {
                    r#type: "button",
                    onclick: move |_| on_open.call(()),
                    "放大"
                }
            }
            button {
                r#type: "button",
                class: "rsb-preview-card",
                title: "打开大图预览",
                onclick: move |_| on_open.call(()),
                oncontextmenu: move |event| {
                    event.prevent_default();
                    event.stop_propagation();
                    let point = event.client_coordinates();
                    on_context_menu.call((context_preview.clone(), point.x, point.y));
                },
                img {
                    src: preview_url,
                    alt: "{preview.name} 纹理预览",
                    decoding: "async",
                    draggable: "false",
                }
            }
            div { class: "rsb-preview-meta",
                span { "{preview.width} × {preview.height}" }
                if preview.rendered_width != preview.width || preview.rendered_height != preview.height {
                    span { "预览 {preview.rendered_width} × {preview.rendered_height}" }
                }
                span { "{format_bytes(preview.payload_size as u64)} PTX" }
                span { "Global #{preview.global_index}" }
            }
        }
    }
}

#[component]
fn PtxPreviewLoadingCard() -> Element {
    rsx! {
        section { class: "rsb-preview-section",
            div { class: "rsb-preview-heading",
                div {
                    strong { "纹理预览" }
                    span { "后台解码中" }
                }
            }
            div { class: "rsb-preview-card rsb-preview-card--loading",
                span { class: "rsb-preview-spinner" }
                strong { "正在生成缩略图" }
                small { "解码不会阻塞界面" }
            }
        }
    }
}

#[component]
fn EditArchiveDialog(
    dialog: EditDialog,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<EditDialog>,
) -> Element {
    let mut draft = use_signal(|| dialog.clone());
    let current = draft();
    let (title, description, confirm_label, danger) = match &current {
        EditDialog::CreateRsg(_) => (
            "新建 RSG",
            "在当前 RSB 内创建空的 Part 0 数据包",
            "创建",
            false,
        ),
        EditDialog::CreateFolder(_) => ("新建文件夹", "在当前 RSG 位置创建目录", "创建", false),
        EditDialog::ImportFiles(import) => (
            "导入文件",
            "为所选文件选择统一、明确的导入方式",
            if import.mode == FileImportMode::Direct {
                "添加"
            } else {
                "继续"
            },
            false,
        ),
        EditDialog::AddTexture(_) => (
            "添加纹理",
            "确认无文件头 PTX 的尺寸、格式与存储属性",
            "添加",
            false,
        ),
        EditDialog::Rename { .. } => ("重命名", "修改会在保存 RSB 时统一写入", "应用", false),
        EditDialog::Delete { .. } => ("删除项目", "删除会在保存 RSB 时生效", "删除", true),
        EditDialog::Properties(_) => ("属性", "修改会在保存 RSB 时统一写入", "应用", false),
    };
    rsx! {
        div { class: "rsb-dialog-layer",
            button {
                r#type: "button",
                class: "rsb-dialog-backdrop",
                aria_label: "关闭",
                onclick: move |_| on_cancel.call(()),
            }
            section {
                class: "rsb-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: title,
                header {
                    div {
                        strong { "{title}" }
                        span { "{description}" }
                    }
                    button {
                        r#type: "button",
                        aria_label: "关闭",
                        onclick: move |_| on_cancel.call(()),
                        "×"
                    }
                }
                div { class: "rsb-dialog-body",
                    match current {
                        EditDialog::CreateRsg(new_rsg) => rsx! {
                            label { class: "rsb-field",
                                span { "RSG 名称" }
                                input {
                                    value: new_rsg.name,
                                    autofocus: true,
                                    oninput: move |event| {
                                        if let EditDialog::CreateRsg(new_rsg) = &mut *draft.write() {
                                            new_rsg.name = event.value();
                                        }
                                    },
                                }
                            }
                            label { class: "rsb-field",
                                span { "压缩方式" }
                                select {
                                    value: new_rsg.compression_flags,
                                    oninput: move |event| {
                                        if let EditDialog::CreateRsg(new_rsg) = &mut *draft.write() {
                                            new_rsg.compression_flags = event.value();
                                        }
                                    },
                                    option { value: "2", "Part 0 · zlib" }
                                    option { value: "0", "Raw" }
                                }
                            }
                            p { class: "rsb-field-hint",
                                "创建后可进入该 RSG 添加普通文件或 Part 1 纹理。"
                            }
                        },
                        EditDialog::CreateFolder(folder) => {
                            let parent = if folder.parent.is_empty() {
                                "RSG 根目录".to_string()
                            } else {
                                folder.parent.join("/")
                            };
                            rsx! {
                                label { class: "rsb-field",
                                    span { "文件夹名称" }
                                    input {
                                        value: folder.name,
                                        autofocus: true,
                                        oninput: move |event| {
                                            if let EditDialog::CreateFolder(folder) =
                                                &mut *draft.write()
                                            {
                                                folder.name = event.value();
                                            }
                                        },
                                    }
                                }
                                p { class: "rsb-field-hint",
                                    "创建位置：{parent}。空目录只在当前编辑会话中保留；向其中添加文件后会随 RSG 保存。"
                                }
                            }
                        },
                        EditDialog::ImportFiles(import) => {
                            let file_count = import.files.len();
                            let source_label = if file_count == 1 {
                                import.files[0].name.clone()
                            } else {
                                format!("{file_count} 个文件")
                            };
                            let directory_label = if import.directory.is_empty() {
                                "RSG 根目录".to_string()
                            } else {
                                import.directory.join("/")
                            };
                            let single_kind = import.single_kind();
                            let texture_mode = import.texture_mode();
                            let summary_icon = if single_kind == Some(AddedFileKind::RasterImage) {
                                "image"
                            } else {
                                "file"
                            };
                            rsx! {
                                div { class: "rsb-import-summary",
                                    span { class: "rsb-import-summary-icon",
                                        Glyph { name: summary_icon }
                                    }
                                    div {
                                        strong { "{source_label}" }
                                        span { "目标：{directory_label}" }
                                    }
                                }
                                div { class: "rsb-import-options",
                                    button {
                                        r#type: "button",
                                        class: if import.mode == FileImportMode::Direct {
                                            "rsb-import-option is-selected"
                                        } else {
                                            "rsb-import-option"
                                        },
                                        aria_pressed: import.mode == FileImportMode::Direct,
                                        onclick: move |_| {
                                            if let EditDialog::ImportFiles(import) = &mut *draft.write() {
                                                import.mode = FileImportMode::Direct;
                                            }
                                        },
                                        span { class: "rsb-import-option-icon", Glyph { name: "file" } }
                                        div {
                                            strong { "作为普通文件添加" }
                                            span { "保留原始名称和字节内容，写入 RSG Part 0" }
                                        }
                                    }
                                    if texture_mode == Some(FileImportMode::AddPtx) {
                                        button {
                                            r#type: "button",
                                            class: if import.mode == FileImportMode::AddPtx {
                                                "rsb-import-option is-selected"
                                            } else {
                                                "rsb-import-option"
                                            },
                                            aria_pressed: import.mode == FileImportMode::AddPtx,
                                            onclick: move |_| {
                                                if let EditDialog::ImportFiles(import) = &mut *draft.write() {
                                                    import.mode = FileImportMode::AddPtx;
                                                }
                                            },
                                            span { class: "rsb-import-option-icon", Glyph { name: "image" } }
                                            div {
                                                strong { "作为 PTX 纹理添加" }
                                                span { "确认尺寸和格式后写入 RSG Part 1 与全局 PTX 表" }
                                            }
                                        }
                                    }
                                    if texture_mode == Some(FileImportMode::EncodePtx) {
                                        button {
                                            r#type: "button",
                                            class: if import.mode == FileImportMode::EncodePtx {
                                                "rsb-import-option is-selected"
                                            } else {
                                                "rsb-import-option"
                                            },
                                            aria_pressed: import.mode == FileImportMode::EncodePtx,
                                            onclick: move |_| {
                                                if let EditDialog::ImportFiles(import) = &mut *draft.write() {
                                                    import.mode = FileImportMode::EncodePtx;
                                                }
                                            },
                                            span { class: "rsb-import-option-icon", Glyph { name: "image" } }
                                            div {
                                                strong { "编码为 PTX 纹理" }
                                                span { "下一步选择编码布局与属性，再写入 RSG Part 1" }
                                            }
                                        }
                                    }
                                }
                                p { class: "rsb-field-hint",
                                    if file_count > 1 {
                                        "多选文件统一按普通文件添加；需要作为纹理处理时请单独选择一个文件。"
                                    } else if single_kind == Some(AddedFileKind::Ordinary) {
                                        "当前文件类型没有纹理转换器，将按普通文件原样加入。"
                                    } else {
                                        "所有文件都可原样加入；只有明确选择纹理方式时才会写入 Part 1 和全局 PTX 元数据。"
                                    }
                                }
                            }
                        },
                        EditDialog::AddTexture(texture) => rsx! {
                            label { class: "rsb-field rsb-field--wide",
                                span { "归档路径" }
                                input {
                                    value: texture.path,
                                    autofocus: true,
                                    oninput: move |event| {
                                        if let EditDialog::AddTexture(texture) = &mut *draft.write() {
                                            texture.path = event.value();
                                        }
                                    },
                                }
                            }
                            if texture.source == TextureSourceKind::RasterImage {
                                label { class: "rsb-field rsb-field--wide",
                                    span { "编码布局" }
                                    select {
                                        value: texture_encoding_value(texture.encoding),
                                        oninput: move |event| {
                                            if let EditDialog::AddTexture(texture) = &mut *draft.write() {
                                                texture.encoding =
                                                    parse_texture_encoding(&event.value());
                                            }
                                        },
                                        option { value: "metadata", "根据格式代码与 Alpha 属性" }
                                        option { value: "etc1", "ETC1" }
                                        option { value: "etc1-a8", "ETC1 + A8" }
                                        option { value: "etc1-compressed-alpha", "ETC1 + 压缩 Alpha" }
                                        option { value: "etc1-palette", "ETC1 + Alpha Palette" }
                                    }
                                }
                            }
                            div { class: "rsb-field-grid",
                                TextureInput {
                                    label: "宽度",
                                    value: texture.width,
                                    field: "width",
                                    draft,
                                }
                                TextureInput {
                                    label: "高度",
                                    value: texture.height,
                                    field: "height",
                                    draft,
                                }
                                TextureInput {
                                    label: "格式代码",
                                    value: texture.format,
                                    field: "format",
                                    draft,
                                }
                                TextureInput {
                                    label: "Pitch",
                                    value: texture.pitch,
                                    field: "pitch",
                                    draft,
                                }
                                TextureInput {
                                    label: "附加字节",
                                    value: texture.alpha_size,
                                    field: "alpha_size",
                                    draft,
                                }
                                TextureInput {
                                    label: "缩放",
                                    value: texture.alpha_format,
                                    field: "alpha_format",
                                    draft,
                                }
                            }
                            p { class: "rsb-field-hint",
                                if texture.source == TextureSourceKind::RasterImage {
                                    "PNG、WebP 或 JPEG 会在后台编码；修改宽高会缩放源图片。常用格式代码：0 RGBA8888、30 PVRTC、147 ETC1、148 PVRTC+A8、160–163 ASTC。ETC1 专用布局会自动使用格式代码 147 并修正 Alpha 属性。"
                                } else {
                                    "PTX 本身不保存文件头；默认值取自当前 RSG 的最后一个纹理。添加前会校验数据长度与这些属性是否一致。"
                                }
                            }
                        },
                        EditDialog::Rename { value, .. } => rsx! {
                            label { class: "rsb-field",
                                span { "新名称" }
                                input {
                                    value,
                                    autofocus: true,
                                    oninput: move |event| {
                                        if let EditDialog::Rename { value, .. } = &mut *draft.write() {
                                            *value = event.value();
                                        }
                                    },
                                }
                            }
                        },
                        EditDialog::Delete { label, .. } => rsx! {
                            div { class: "rsb-delete-message",
                                span { class: "rsb-dialog-warning-icon", Glyph { name: "delete" } }
                                div {
                                    strong { "确定删除“{label}”吗？" }
                                    p { "删除会在保存归档时生效；若包含纹理，后续全局 PTX 序号会自动重排。" }
                                }
                            }
                        },
                        EditDialog::Properties(properties) => {
                            match properties.target {
                                PropertiesTarget::Packet(_) => rsx! {
                                    label { class: "rsb-field",
                                        span { "RSG 名称" }
                                        input {
                                            value: properties.name_or_path,
                                            oninput: move |event| {
                                                if let EditDialog::Properties(properties) = &mut *draft.write() {
                                                    properties.name_or_path = event.value();
                                                }
                                            },
                                        }
                                    }
                                    label { class: "rsb-field",
                                        span { "压缩方式" }
                                        select {
                                            value: properties.compression_flags,
                                            oninput: move |event| {
                                                if let EditDialog::Properties(properties) = &mut *draft.write() {
                                                    properties.compression_flags = event.value();
                                                }
                                            },
                                            option { value: "0", "Raw" }
                                            option { value: "1", "Part 1 · zlib" }
                                            option { value: "2", "Part 0 · zlib" }
                                            option { value: "3", "Part 0 + Part 1 · zlib" }
                                        }
                                    }
                                },
                                PropertiesTarget::File(_) => rsx! {
                                    label { class: "rsb-field rsb-field--wide",
                                        span { "归档路径" }
                                        input {
                                            value: properties.name_or_path,
                                            oninput: move |event| {
                                                if let EditDialog::Properties(properties) = &mut *draft.write() {
                                                    properties.name_or_path = event.value();
                                                }
                                            },
                                        }
                                    }
                                    if !properties.width.is_empty() {
                                        div { class: "rsb-field-grid",
                                            PropertyInput {
                                                label: "宽度",
                                                value: properties.width,
                                                field: "width",
                                                draft,
                                            }
                                            PropertyInput {
                                                label: "高度",
                                                value: properties.height,
                                                field: "height",
                                                draft,
                                            }
                                            PropertyInput {
                                                label: "格式代码",
                                                value: properties.format,
                                                field: "format",
                                                draft,
                                            }
                                            PropertyInput {
                                                label: "Pitch",
                                                value: properties.pitch,
                                                field: "pitch",
                                                draft,
                                            }
                                            PropertyInput {
                                                label: "附加字节",
                                                value: properties.alpha_size,
                                                field: "alpha_size",
                                                draft,
                                            }
                                            PropertyInput {
                                                label: "缩放",
                                                value: properties.alpha_format,
                                                field: "alpha_format",
                                                draft,
                                            }
                                        }
                                        p { class: "rsb-field-hint",
                                            "修改纹理属性后会使用当前 RSG 的局部 ID 与全局 PTX 表进行保存。"
                                        }
                                    }
                                },
                            }
                        },
                    }
                }
                footer {
                    button {
                        r#type: "button",
                        class: "rsb-dialog-button",
                        onclick: move |_| on_cancel.call(()),
                        "取消"
                    }
                    button {
                        r#type: "button",
                        class: if danger { "rsb-dialog-button rsb-dialog-button--danger" } else { "rsb-dialog-button rsb-dialog-button--primary" },
                        onclick: move |_| on_confirm.call(draft()),
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}

#[component]
fn PropertyInput(
    label: &'static str,
    value: String,
    field: &'static str,
    mut draft: Signal<EditDialog>,
) -> Element {
    rsx! {
        label { class: "rsb-field",
            span { "{label}" }
            input {
                value,
                inputmode: "numeric",
                oninput: move |event| {
                    let EditDialog::Properties(properties) = &mut *draft.write() else {
                        return;
                    };
                    match field {
                        "width" => properties.width = event.value(),
                        "height" => properties.height = event.value(),
                        "format" => properties.format = event.value(),
                        "pitch" => properties.pitch = event.value(),
                        "alpha_size" => properties.alpha_size = event.value(),
                        "alpha_format" => properties.alpha_format = event.value(),
                        _ => {}
                    }
                },
            }
        }
    }
}

const fn texture_encoding_value(encoding: TextureEncoding) -> &'static str {
    match encoding {
        TextureEncoding::Metadata => "metadata",
        TextureEncoding::Etc1 => "etc1",
        TextureEncoding::Etc1A8 => "etc1-a8",
        TextureEncoding::Etc1CompressedAlpha => "etc1-compressed-alpha",
        TextureEncoding::Etc1Palette => "etc1-palette",
    }
}

fn parse_texture_encoding(value: &str) -> TextureEncoding {
    match value {
        "etc1" => TextureEncoding::Etc1,
        "etc1-a8" => TextureEncoding::Etc1A8,
        "etc1-compressed-alpha" => TextureEncoding::Etc1CompressedAlpha,
        "etc1-palette" => TextureEncoding::Etc1Palette,
        _ => TextureEncoding::Metadata,
    }
}

#[component]
fn TextureInput(
    label: &'static str,
    value: String,
    field: &'static str,
    mut draft: Signal<EditDialog>,
) -> Element {
    rsx! {
        label { class: "rsb-field",
            span { "{label}" }
            input {
                value,
                inputmode: "numeric",
                oninput: move |event| {
                    let EditDialog::AddTexture(texture) = &mut *draft.write() else {
                        return;
                    };
                    match field {
                        "width" => texture.width = event.value(),
                        "height" => texture.height = event.value(),
                        "format" => texture.format = event.value(),
                        "pitch" => texture.pitch = event.value(),
                        "alpha_size" => texture.alpha_size = event.value(),
                        "alpha_format" => texture.alpha_format = event.value(),
                        _ => {}
                    }
                },
            }
        }
    }
}

#[component]
fn UnsavedChangesDialog(
    name: String,
    on_cancel: EventHandler<()>,
    on_discard: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "rsb-dialog-layer",
            button {
                r#type: "button",
                class: "rsb-dialog-backdrop",
                aria_label: "关闭",
                onclick: move |_| on_cancel.call(()),
            }
            section {
                class: "rsb-dialog rsb-dialog--compact",
                role: "alertdialog",
                aria_modal: "true",
                aria_label: "未保存修改",
                header {
                    div {
                        strong { "放弃未保存修改？" }
                        span { "将打开 {name}" }
                    }
                }
                div { class: "rsb-dialog-body",
                    p { class: "rsb-dialog-copy",
                        "当前 RSB 中新增、替换、重命名或属性修改的内容还没有保存。"
                    }
                }
                footer {
                    button {
                        r#type: "button",
                        class: "rsb-dialog-button",
                        onclick: move |_| on_cancel.call(()),
                        "继续编辑"
                    }
                    button {
                        r#type: "button",
                        class: "rsb-dialog-button rsb-dialog-button--danger",
                        onclick: move |_| on_discard.call(()),
                        "放弃并打开"
                    }
                }
            }
        }
    }
}

const PREVIEW_MIN_ZOOM: f64 = 0.05;
const PREVIEW_MAX_ZOOM: f64 = 16.0;
const PREVIEW_CANVAS_INSET: f64 = 48.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct PreviewPoint {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct PreviewSize {
    width: f64,
    height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreviewDrag {
    pointer: PreviewPoint,
    pan: PreviewPoint,
}

fn preview_fit_scale(viewport: PreviewSize, image: PreviewSize) -> f64 {
    if viewport.width <= 1.0 || viewport.height <= 1.0 || image.width <= 0.0 || image.height <= 0.0
    {
        return 1.0;
    }
    let available_width = (viewport.width - PREVIEW_CANVAS_INSET).max(1.0);
    let available_height = (viewport.height - PREVIEW_CANVAS_INSET).max(1.0);
    (available_width / image.width)
        .min(available_height / image.height)
        .clamp(PREVIEW_MIN_ZOOM, PREVIEW_MAX_ZOOM)
}

fn clamp_preview_pan(
    pan: PreviewPoint,
    viewport: PreviewSize,
    image: PreviewSize,
    zoom: f64,
) -> PreviewPoint {
    let available_width = (viewport.width - PREVIEW_CANVAS_INSET).max(1.0);
    let available_height = (viewport.height - PREVIEW_CANVAS_INSET).max(1.0);
    let max_x = ((image.width * zoom - available_width) / 2.0).max(0.0);
    let max_y = ((image.height * zoom - available_height) / 2.0).max(0.0);
    PreviewPoint {
        x: pan.x.clamp(-max_x, max_x),
        y: pan.y.clamp(-max_y, max_y),
    }
}

fn preview_pan_from_drag(
    start: PreviewDrag,
    pointer: PreviewPoint,
    viewport: PreviewSize,
    image: PreviewSize,
    zoom: f64,
) -> PreviewPoint {
    clamp_preview_pan(
        PreviewPoint {
            x: start.pan.x + pointer.x - start.pointer.x,
            y: start.pan.y + pointer.y - start.pointer.y,
        },
        viewport,
        image,
        zoom,
    )
}

fn preview_pan_after_zoom(
    pan: PreviewPoint,
    anchor: PreviewPoint,
    viewport: PreviewSize,
    image: PreviewSize,
    old_zoom: f64,
    new_zoom: f64,
) -> PreviewPoint {
    if old_zoom <= 0.0 {
        return PreviewPoint::default();
    }
    let center = PreviewPoint {
        x: viewport.width / 2.0,
        y: viewport.height / 2.0,
    };
    let ratio = new_zoom / old_zoom;
    clamp_preview_pan(
        PreviewPoint {
            x: anchor.x - center.x - (anchor.x - center.x - pan.x) * ratio,
            y: anchor.y - center.y - (anchor.y - center.y - pan.y) * ratio,
        },
        viewport,
        image,
        new_zoom,
    )
}

fn set_preview_zoom(
    mut fit_mode: Signal<bool>,
    mut manual_zoom: Signal<f64>,
    mut pan: Signal<PreviewPoint>,
    viewport: PreviewSize,
    image: PreviewSize,
    requested_zoom: f64,
    anchor: Option<PreviewPoint>,
) {
    let old_zoom = if fit_mode() {
        preview_fit_scale(viewport, image)
    } else {
        manual_zoom()
    };
    let new_zoom = requested_zoom.clamp(PREVIEW_MIN_ZOOM, PREVIEW_MAX_ZOOM);
    let anchor = anchor.unwrap_or(PreviewPoint {
        x: viewport.width / 2.0,
        y: viewport.height / 2.0,
    });
    let next_pan = preview_pan_after_zoom(pan(), anchor, viewport, image, old_zoom, new_zoom);
    fit_mode.set(false);
    manual_zoom.set(new_zoom);
    pan.set(next_pan);
}

#[component]
fn PreviewModal(
    preview: PtxPreview,
    loading: bool,
    on_close: EventHandler<()>,
    on_context_menu: EventHandler<(PtxPreview, f64, f64)>,
    on_export: EventHandler<usize>,
) -> Element {
    let preview_url = preview.asset.url().to_string();
    let context_preview = preview.clone();
    let export_index = preview.file_index;
    let image_size = PreviewSize {
        width: f64::from(preview.rendered_width.max(1)),
        height: f64::from(preview.rendered_height.max(1)),
    };
    let mut viewport = use_signal(|| PreviewSize {
        width: 720.0,
        height: 560.0,
    });
    let mut fit_mode = use_signal(|| true);
    let mut manual_zoom = use_signal(|| 1.0_f64);
    let mut pan = use_signal(PreviewPoint::default);
    let mut drag = use_signal(|| None::<PreviewDrag>);
    let viewport_snapshot = viewport();
    let fit_mode_snapshot = fit_mode();
    let zoom = if fit_mode_snapshot {
        preview_fit_scale(viewport_snapshot, image_size)
    } else {
        manual_zoom()
    };
    let pan_snapshot = pan();
    let dragging = drag().is_some();
    let can_pan = image_size.width * zoom
        > (viewport_snapshot.width - PREVIEW_CANVAS_INSET).max(1.0)
        || image_size.height * zoom > (viewport_snapshot.height - PREVIEW_CANVAS_INSET).max(1.0);
    let canvas_class = match (can_pan, dragging) {
        (true, true) => "rsb-preview-modal-canvas is-draggable is-dragging",
        (true, false) => "rsb-preview-modal-canvas is-draggable",
        _ => "rsb-preview-modal-canvas",
    };
    let zoom_percent = (zoom * 100.0).round().clamp(1.0, 1600.0) as u32;
    let image_style = format!(
        "width: {}px; height: {}px; margin-left: -{}px; margin-top: -{}px; transform: translate({}px, {}px) scale({zoom});",
        preview.rendered_width.max(1),
        preview.rendered_height.max(1),
        f64::from(preview.rendered_width.max(1)) / 2.0,
        f64::from(preview.rendered_height.max(1)) / 2.0,
        pan_snapshot.x,
        pan_snapshot.y,
    );

    rsx! {
        div { class: "rsb-preview-modal-layer",
            button {
                r#type: "button",
                class: "rsb-preview-modal-backdrop",
                aria_label: "关闭纹理预览",
                onclick: move |_| on_close.call(()),
            }
            section {
                class: "rsb-preview-modal",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{preview.name} 纹理预览",
                header {
                    div { class: "rsb-preview-modal-title",
                        strong { "{preview.name}" }
                        span {
                            "{preview.width} × {preview.height} · {preview.format}"
                            if preview.apple_channel_order { " · Apple channel order" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "rsb-preview-close",
                        aria_label: "关闭纹理预览",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div {
                    class: "{canvas_class}",
                    tabindex: "0",
                    aria_label: "纹理画布，可使用滚轮缩放并拖拽移动",
                    onmounted: move |_| {
                        let _ = document::eval(RSB_PREVIEW_POINTER_CAPTURE);
                    },
                    oncontextmenu: move |event| {
                        event.prevent_default();
                        event.stop_propagation();
                        let point = event.client_coordinates();
                        on_context_menu.call((context_preview.clone(), point.x, point.y));
                    },
                    onresize: move |event| {
                        let Ok(size) = event.get_content_box_size() else {
                            return;
                        };
                        let next = PreviewSize {
                            width: size.width.max(1.0),
                            height: size.height.max(1.0),
                        };
                        viewport.set(next);
                        if !fit_mode() {
                            pan.set(clamp_preview_pan(pan(), next, image_size, manual_zoom()));
                        }
                    },
                    onwheel: move |event| {
                        event.prevent_default();
                        let delta = event.delta().strip_units().y;
                        let factor = (-delta * 0.0015).exp().clamp(0.8, 1.25);
                        let point = event.element_coordinates();
                        set_preview_zoom(
                            fit_mode,
                            manual_zoom,
                            pan,
                            viewport(),
                            image_size,
                            zoom * factor,
                            Some(PreviewPoint { x: point.x, y: point.y }),
                        );
                    },
                    ondoubleclick: move |event| {
                        event.prevent_default();
                        pan.set(PreviewPoint::default());
                        if fit_mode() {
                            fit_mode.set(false);
                            manual_zoom.set(1.0);
                        } else {
                            fit_mode.set(true);
                        }
                    },
                    onpointerdown: move |event| {
                        if !event.is_primary()
                            || !matches!(event.trigger_button(), None | Some(MouseButton::Primary))
                            || !can_pan
                        {
                            return;
                        }
                        event.prevent_default();
                        let point = event.client_coordinates();
                        drag.set(Some(PreviewDrag {
                            pointer: PreviewPoint { x: point.x, y: point.y },
                            pan: pan(),
                        }));
                    },
                    onpointermove: move |event| {
                        let Some(start) = drag() else {
                            return;
                        };
                        event.prevent_default();
                        let point = event.client_coordinates();
                        pan.set(preview_pan_from_drag(
                            start,
                            PreviewPoint {
                                x: point.x,
                                y: point.y,
                            },
                            viewport(),
                            image_size,
                            zoom,
                        ));
                    },
                    onpointerup: move |_| drag.set(None),
                    onpointercancel: move |_| drag.set(None),
                    div {
                        class: "rsb-preview-modal-image",
                        style: "{image_style}",
                        img {
                            src: preview_url,
                            alt: "{preview.name}",
                            decoding: "async",
                            draggable: "false",
                        }
                    }
                    div {
                        class: "rsb-preview-view-controls",
                        onpointerdown: move |event| event.stop_propagation(),
                        button {
                            r#type: "button",
                            disabled: zoom <= PREVIEW_MIN_ZOOM + f64::EPSILON,
                            title: "缩小",
                            aria_label: "缩小纹理",
                            onclick: move |_| set_preview_zoom(
                                fit_mode,
                                manual_zoom,
                                pan,
                                viewport(),
                                image_size,
                                zoom / 1.25,
                                None,
                            ),
                            "−"
                        }
                        button {
                            r#type: "button",
                            class: if fit_mode_snapshot { "is-active" } else { "" },
                            title: "适应窗口",
                            aria_label: "使纹理适应预览窗口",
                            aria_pressed: fit_mode_snapshot,
                            onclick: move |_| {
                                fit_mode.set(true);
                                pan.set(PreviewPoint::default());
                            },
                            "适应"
                        }
                        button {
                            r#type: "button",
                            class: if !fit_mode_snapshot && (zoom - 1.0).abs() < 0.0001 { "is-active" } else { "" },
                            title: "实际尺寸",
                            aria_label: "以实际尺寸显示纹理",
                            aria_pressed: !fit_mode_snapshot && (zoom - 1.0).abs() < 0.0001,
                            onclick: move |_| {
                                fit_mode.set(false);
                                manual_zoom.set(1.0);
                                pan.set(PreviewPoint::default());
                            },
                            "1:1"
                        }
                        output {
                            class: "rsb-preview-zoom-value",
                            aria_live: "polite",
                            "{zoom_percent}%"
                        }
                        button {
                            r#type: "button",
                            disabled: zoom >= PREVIEW_MAX_ZOOM - f64::EPSILON,
                            title: "放大",
                            aria_label: "放大纹理",
                            onclick: move |_| set_preview_zoom(
                                fit_mode,
                                manual_zoom,
                                pan,
                                viewport(),
                                image_size,
                                zoom * 1.25,
                                None,
                            ),
                            "+"
                        }
                        span {
                            class: "rsb-preview-control-separator",
                            aria_hidden: "true",
                        }
                        button {
                            r#type: "button",
                            class: "rsb-preview-export-button",
                            title: "导出 PNG",
                            aria_label: "导出原始尺寸 PNG",
                            onclick: move |_| on_export.call(export_index),
                            Glyph { name: "export" }
                        }
                    }
                    if loading {
                        div { class: "rsb-preview-loading-badge",
                            span { class: "rsb-preview-spinner" }
                            "正在加载高分辨率预览"
                        }
                    }
                }
                footer {
                    div { class: "rsb-preview-footer-meta",
                        span { "PTX #{preview.global_index}" }
                        span { "{format_bytes(preview.payload_size as u64)}" }
                        if preview.rendered_width != preview.width || preview.rendered_height != preview.height {
                            span { "预览 {preview.rendered_width} × {preview.rendered_height}" }
                        }
                    }
                    span { class: "rsb-preview-footer-hint", "滚轮缩放 · 拖拽移动 · 双击切换适应/1:1" }
                }
            }
        }
    }
}

#[component]
fn PreviewLoadingModal(
    preview: Option<PtxPreview>,
    on_close: EventHandler<()>,
    on_context_menu: EventHandler<(PtxPreview, f64, f64)>,
    on_export: EventHandler<usize>,
) -> Element {
    if let Some(preview) = preview {
        return rsx! {
            PreviewModal {
                preview,
                loading: true,
                on_close,
                on_context_menu,
                on_export,
            }
        };
    }
    rsx! {
        div { class: "rsb-preview-modal-layer",
            button {
                r#type: "button",
                class: "rsb-preview-modal-backdrop",
                aria_label: "关闭纹理预览",
                onclick: move |_| on_close.call(()),
            }
            section {
                class: "rsb-preview-modal rsb-preview-modal--loading",
                role: "dialog",
                aria_modal: "true",
                aria_label: "正在准备纹理预览",
                header {
                    div {
                        strong { "正在准备纹理预览" }
                        span { "后台解码不会阻塞界面" }
                    }
                    button {
                        r#type: "button",
                        aria_label: "关闭纹理预览",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div { class: "rsb-preview-modal-canvas rsb-preview-modal-canvas--loading",
                    span { class: "rsb-preview-spinner rsb-preview-spinner--large" }
                    strong { "正在解码 PTX" }
                    small { "可以随时关闭或切换文件" }
                }
            }
        }
    }
}

#[component]
fn PreviewErrorModal(message: String, on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "rsb-preview-modal-layer",
            button {
                r#type: "button",
                class: "rsb-preview-modal-backdrop",
                aria_label: "关闭纹理预览",
                onclick: move |_| on_close.call(()),
            }
            section {
                class: "rsb-preview-modal rsb-preview-modal--error",
                role: "alertdialog",
                aria_modal: "true",
                aria_label: "纹理预览失败",
                div { class: "rsb-preview-modal-error",
                    Glyph { name: "image" }
                    strong { "无法预览此纹理" }
                    p { "{message}" }
                    button {
                        r#type: "button",
                        onclick: move |_| on_close.call(()),
                        "关闭"
                    }
                }
            }
        }
    }
}

#[component]
fn PacketInspector(record: crate::domain::PacketRecord) -> Element {
    let mut rows = vec![
        ("包索引".into(), format!("#{}", record.index)),
        ("压缩方式".into(), record.compression_label().into()),
        ("原始大小".into(), format_bytes(record.unpacked_data_size())),
        ("压缩后".into(), format_bytes(record.stored_data_size())),
        ("压缩率".into(), record.ratio_label()),
        (
            "归档偏移".into(),
            format!("0x{:08X}", record.info.rsg_offset),
        ),
        ("PTX 数量".into(), record.info.ptx_number.to_string()),
    ];
    if let Some(error) = &record.error {
        rows.push(("头部错误".into(), error.clone()));
    }
    rsx! {
        InspectorTitle {
            icon: "package",
            name: record.info.name.clone(),
            kind: "RSG 数据包".to_string(),
        }
        InspectorProperties { rows }
        div { class: "rsb-inspector-tip",
            strong { "双击打开" }
            p { "进入数据包时才会执行 zlib 解压并读取文件列表。" }
        }
    }
}

#[component]
fn InspectorTitle(icon: &'static str, name: String, kind: String) -> Element {
    rsx! {
        div { class: "rsb-inspector-title",
            span { class: "rsb-inspector-icon", Glyph { name: icon } }
            div {
                strong { "{name}" }
                span { "{kind}" }
            }
        }
    }
}

#[component]
fn InspectorProperties(rows: Vec<(String, String)>) -> Element {
    rsx! {
        dl { class: "rsb-property-list",
            for (label, value) in rows {
                div {
                    dt { "{label}" }
                    dd { title: "{value}", "{value}" }
                }
            }
        }
    }
}

#[component]
fn InspectorEmpty() -> Element {
    rsx! {
        div { class: "rsb-inspector-empty",
            Glyph { name: "pointer" }
            strong { "选择一个项目" }
            p { "这里会显示类型、大小、压缩方式与纹理信息。" }
        }
    }
}

#[component]
fn StatusBar(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed_packets: RemovedPackets,
    virtual_directories: BTreeSet<Vec<String>>,
    packet: Option<Arc<PacketDocument>>,
    location: BrowserLocation,
    status: AppStatus,
) -> Element {
    let item_count = match location {
        BrowserLocation::Archive => {
            editing::visible_packet_count(&archive, &edits, &removed_packets)
        }
        BrowserLocation::Packet { directory, .. } => packet
            .as_ref()
            .map(|value| {
                value
                    .directory_index
                    .item_count_with_virtual(&directory, &virtual_directories)
            })
            .unwrap_or_default(),
    };
    rsx! {
        footer { class: "rsb-status-bar",
            span { class: "rsb-status-dot rsb-status-dot--{status.tone.class()}" }
            span { class: "rsb-status-message", "{status.message}" }
            span { class: "rsb-status-count", "{item_count} 个项目" }
        }
    }
}

#[component]
fn EmptyArchive(on_open: EventHandler<ArchiveDocument>, on_error: EventHandler<String>) -> Element {
    rsx! {
        div { class: "rsb-empty",
            div { class: "rsb-empty-icon", Glyph { name: "archive" } }
            div { class: "rsb-empty-title", "打开 RSB 归档" }
            div { class: "rsb-empty-subtitle",
                "浏览 RSG 数据包、编辑文件与属性，并直接预览或导出 PTX 纹理。"
            }
            OpenArchiveButton { on_open, on_error, primary: true }
            div { class: "rsb-format-row", aria_hidden: "true",
                span { "RSB" }
                span { "RSG" }
                span { "PTX" }
                span { "RTON" }
            }
        }
    }
}

#[component]
fn Glyph(name: &'static str) -> Element {
    let path = match name {
        "open" => {
            "M3 6.75A2.75 2.75 0 0 1 5.75 4h4l2 2h6.5A2.75 2.75 0 0 1 21 8.75v8.5A2.75 2.75 0 0 1 18.25 20H5.75A2.75 2.75 0 0 1 3 17.25V6.75Zm0 3.25h18"
        }
        "save" => "M5 3h12l2 2v16H5V3Zm3 0v6h8V3M8 21v-7h8v7",
        "save-as" => "M5 3h10l3 3v5M8 3v6h7V3M5 21V3m4 18 9-9 3 3-6 6H9Z",
        "add" => "M12 5v14M5 12h14",
        "replace" => "m7 7-3 3 3 3M4 10h11a5 5 0 0 1 5 5v1m-3 1 3-3 3 3",
        "rename" => "m4 20 4.5-1 10-10-3.5-3.5-10 10L4 20Zm9-12 3.5 3.5M4 4h7",
        "delete" => "M5 7h14M9 7V4h6v3m2 0-1 13H8L7 7m3 4v6m4-6v6",
        "properties" => "M4 6h10m4 0h2M4 12h2m4 0h10M4 18h7m4 0h5M14 4v4M6 10v4m5 2v4",
        "up" => "m5 15 7-7 7 7",
        "tree" => "M4 5h6M7 5v14m0-9h5m-5 6h5M12 8h8v4h-8V8Zm0 6h8v4h-8v-4Z",
        "extract" => "M12 3v12m0 0 5-5m-5 5-5-5M4 19h16",
        "export" => "M12 3v12m0 0 5-5m-5 5-5-5M5 20h14",
        "panel" => "M4 4h16v16H4V4Zm10 0v16",
        "more" => "M5 12h.01M12 12h.01M19 12h.01",
        "close" => "M6 6l12 12M18 6 6 18",
        "archive" => "M4 7h16v13H4V7Zm-1-3h18v3H3V4Zm6 7h6",
        "package" => "m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Zm-8 4.5 8 4.5 8-4.5M12 12v9",
        "folder" => {
            "M3 6.5A2.5 2.5 0 0 1 5.5 4H10l2 2h6.5A2.5 2.5 0 0 1 21 8.5v8A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5v-10Z"
        }
        "folder-add" => {
            "M3 7A2.5 2.5 0 0 1 5.5 4H10l2 2h6.5A2.5 2.5 0 0 1 21 8.5V11M3 7v9.5A2.5 2.5 0 0 0 5.5 19H11m6-5v7m-3.5-3.5h7"
        }
        "file" => "M6 3h8l4 4v14H6V3Zm8 0v5h4",
        "image" => "M4 4h16v16H4V4Zm0 12 5-5 4 4 2-2 5 5M15.5 8.5h.01",
        "search" => "m20 20-4.2-4.2M18 11a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z",
        "pointer" => "m5 3 13 9-6 1.5L9 20 5 3Z",
        _ => "",
    };
    rsx! {
        svg {
            class: "rsb-glyph",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "{path}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AddedFileKind, EditDialog, FileImportMode, PreviewDrag, PreviewPoint, PreviewSize,
        added_file_kind, archive_context_capabilities, archive_context_open_label,
        archive_file_name, clamp_preview_pan, file_import_dialog, image_ptx_name,
        is_raster_image_file, is_rton_file, preview_fit_scale, preview_pan_after_zoom,
        preview_pan_from_drag,
    };
    use crate::domain::{PacketDocument, PacketRecord, RowSelection};
    use crate::editing::AddedFile;
    use rsb_archive::{Part1Extra, RsgInfo, UnpackedFile};

    #[test]
    fn recognizes_rton_archive_paths_case_insensitively() {
        assert!(is_rton_file("properties/plants.rton"));
        assert!(is_rton_file(r"properties\ZOMBIES.RTON"));
        assert!(!is_rton_file("properties/rton.json"));
        assert!(!is_rton_file("properties/not-rton"));
    }

    #[test]
    fn extracts_archive_file_name_for_both_separator_styles() {
        assert_eq!(archive_file_name("properties/plants.rton"), "plants.rton");
        assert_eq!(
            archive_file_name(r"properties\zombies.rton"),
            "zombies.rton"
        );
    }

    #[test]
    fn recognizes_supported_raster_sources_and_derives_ptx_names() {
        for name in ["texture.png", "texture.WEBP", "texture.jpg", "texture.JPEG"] {
            assert!(is_raster_image_file(name), "{name}");
        }
        assert!(!is_raster_image_file("texture.gif"));
        assert!(!is_raster_image_file("texture.ptx"));
        assert_eq!(image_ptx_name("texture.png"), "texture.ptx");
        assert_eq!(image_ptx_name("atlas.part.webp"), "atlas.part.ptx");
    }

    #[test]
    fn preview_fit_uses_the_available_canvas_without_changing_dialog_size() {
        let wide = preview_fit_scale(
            PreviewSize {
                width: 1000.0,
                height: 700.0,
            },
            PreviewSize {
                width: 2048.0,
                height: 1024.0,
            },
        );
        assert!((wide - 0.464_843_75).abs() < 0.000_001);

        let small = preview_fit_scale(
            PreviewSize {
                width: 1000.0,
                height: 700.0,
            },
            PreviewSize {
                width: 128.0,
                height: 128.0,
            },
        );
        assert!((small - 5.093_75).abs() < 0.000_001);
    }

    #[test]
    fn preview_pan_is_bounded_and_zoom_keeps_the_pointer_anchor() {
        let viewport = PreviewSize {
            width: 1000.0,
            height: 800.0,
        };
        let image = PreviewSize {
            width: 2000.0,
            height: 1200.0,
        };
        let anchored = preview_pan_after_zoom(
            PreviewPoint::default(),
            PreviewPoint { x: 750.0, y: 400.0 },
            viewport,
            image,
            0.5,
            1.0,
        );
        assert!((anchored.x + 250.0).abs() < 0.000_001);
        assert!(anchored.y.abs() < 0.000_001);

        let clamped = clamp_preview_pan(
            PreviewPoint {
                x: 10_000.0,
                y: -10_000.0,
            },
            viewport,
            image,
            1.0,
        );
        assert_eq!(
            clamped,
            PreviewPoint {
                x: 524.0,
                y: -224.0
            }
        );
    }

    #[test]
    fn preview_drag_applies_client_delta_on_both_axes() {
        let viewport = PreviewSize {
            width: 1000.0,
            height: 800.0,
        };
        let image = PreviewSize {
            width: 2000.0,
            height: 1600.0,
        };
        let moved = preview_pan_from_drag(
            PreviewDrag {
                pointer: PreviewPoint { x: 100.0, y: 100.0 },
                pan: PreviewPoint { x: 20.0, y: -30.0 },
            },
            PreviewPoint { x: 180.0, y: 260.0 },
            viewport,
            image,
            1.0,
        );
        assert_eq!(moved, PreviewPoint { x: 100.0, y: 130.0 });
    }

    #[test]
    fn all_file_types_share_the_same_explicit_import_dialog() {
        let cases = [
            ("notes.bin", AddedFileKind::Ordinary, None),
            (
                "atlas.ptx",
                AddedFileKind::EncodedPtx,
                Some(FileImportMode::AddPtx),
            ),
            (
                "atlas.png",
                AddedFileKind::RasterImage,
                Some(FileImportMode::EncodePtx),
            ),
        ];
        for (name, kind, texture_mode) in cases {
            assert_eq!(added_file_kind(name), kind);
            let dialog = file_import_dialog(
                vec![AddedFile {
                    name: name.into(),
                    data: vec![1, 2, 3],
                }],
                vec!["IMAGES".into()],
            )
            .unwrap();
            let EditDialog::ImportFiles(draft) = dialog else {
                panic!("every file must use the common import dialog");
            };
            assert_eq!(draft.mode, FileImportMode::Direct);
            assert_eq!(draft.texture_mode(), texture_mode);
        }

        let EditDialog::ImportFiles(multiple) = file_import_dialog(
            vec![
                AddedFile {
                    name: "a.png".into(),
                    data: vec![1],
                },
                AddedFile {
                    name: "b.ptx".into(),
                    data: vec![2],
                },
            ],
            Vec::new(),
        )
        .unwrap() else {
            panic!("multiple files must use the common import dialog");
        };
        assert_eq!(multiple.mode, FileImportMode::Direct);
        assert_eq!(multiple.texture_mode(), None);
    }

    #[test]
    fn archive_context_menu_respects_file_types_and_part_one_safety() {
        let packet = PacketDocument::new(
            PacketRecord {
                index: 0,
                info: RsgInfo {
                    name: "Packet".into(),
                    rsg_offset: 0,
                    rsg_length: 0,
                    pool_index: 0,
                    ptx_number: 1,
                    ptx_before_number: 0,
                    packet_head_info: None,
                },
                header: None,
                error: None,
            },
            Vec::new(),
            vec![
                UnpackedFile {
                    path: "DATA/CONFIG.RTON".into(),
                    data: vec![1],
                    is_part1: false,
                    part1_info: None,
                },
                UnpackedFile {
                    path: "IMAGES/ATLAS.PTX".into(),
                    data: vec![2],
                    is_part1: true,
                    part1_info: Some(Part1Extra {
                        id: 0,
                        width: 4,
                        height: 4,
                    }),
                },
                UnpackedFile {
                    path: "DATA/RAW.PTX".into(),
                    data: vec![3],
                    is_part1: false,
                    part1_info: None,
                },
            ],
        );

        let rton = RowSelection::File(0);
        let rton_capabilities = archive_context_capabilities(&rton, Some(&packet), None, false);
        assert!(rton_capabilities.open);
        assert!(rton_capabilities.replace);
        assert!(rton_capabilities.delete);
        assert_eq!(
            archive_context_open_label(&rton, Some(&packet)),
            "在 RTON Editor 中打开"
        );

        let ptx = RowSelection::File(1);
        let ptx_capabilities = archive_context_capabilities(&ptx, Some(&packet), None, false);
        assert!(ptx_capabilities.open);
        assert!(ptx_capabilities.delete);
        assert!(ptx_capabilities.export_png);
        assert_eq!(archive_context_open_label(&ptx, Some(&packet)), "预览");

        let direct_ptx = RowSelection::File(2);
        let direct_ptx_capabilities =
            archive_context_capabilities(&direct_ptx, Some(&packet), None, false);
        assert!(!direct_ptx_capabilities.open);
        assert!(direct_ptx_capabilities.replace);
        assert!(direct_ptx_capabilities.delete);
        assert!(!direct_ptx_capabilities.export_png);

        let texture_directory = RowSelection::Directory(vec!["IMAGES".into()]);
        let directory_capabilities =
            archive_context_capabilities(&texture_directory, Some(&packet), None, false);
        assert!(directory_capabilities.add);
        assert!(directory_capabilities.delete);

        let packet_selection = RowSelection::Packet(0);
        let packet_capabilities =
            archive_context_capabilities(&packet_selection, None, Some(&packet.record), false);
        assert!(packet_capabilities.replace);
        assert!(packet_capabilities.delete);
    }
}
