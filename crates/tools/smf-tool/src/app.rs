use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use smf_worker::{
    DocumentMetadata, HeaderVariant, PrepareRequest, PreparedDocument, RebuildRequest, SourceKind,
};
use toolkit_ui::{DropIndicator, InlineNotice, ToolPage, WorkspaceCard, push_application_log};

use crate::{platform, processing};

const SMF_PAGE_CSS: Asset = asset!("/assets/smf/page.css");

#[derive(Clone)]
struct SmfDocument {
    name: String,
    source_kind: SourceKind,
    payload_kind: String,
    payload_name: String,
    smf_name: String,
    tag_name: String,
    payload: Arc<[u8]>,
    encoded: Arc<[u8]>,
    metadata: DocumentMetadata,
}

impl From<PreparedDocument> for SmfDocument {
    fn from(value: PreparedDocument) -> Self {
        Self {
            name: value.name,
            source_kind: value.source_kind,
            payload_kind: value.payload_kind,
            payload_name: value.payload_name,
            smf_name: value.smf_name,
            tag_name: value.tag_name,
            payload: Arc::from(value.payload),
            encoded: Arc::from(value.encoded),
            metadata: value.metadata,
        }
    }
}

#[derive(Clone)]
struct SmfTab {
    id: u64,
    document: Arc<SmfDocument>,
    variant: HeaderVariant,
    compression_level: u32,
    built_variant: HeaderVariant,
    built_level: u32,
}

impl PartialEq for SmfTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && Arc::ptr_eq(&self.document, &other.document)
            && self.variant == other.variant
            && self.compression_level == other.compression_level
            && self.built_variant == other.built_variant
            && self.built_level == other.built_level
    }
}

impl SmfTab {
    fn new(id: u64, prepared: PreparedDocument) -> Self {
        let variant = prepared.metadata.variant;
        let compression_level = prepared.compression_level;
        Self {
            id,
            document: Arc::new(prepared.into()),
            variant,
            compression_level,
            built_variant: variant,
            built_level: compression_level,
        }
    }

    fn dirty(&self) -> bool {
        self.variant != self.built_variant || self.compression_level != self.built_level
    }
}

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

impl AppStatus {
    fn new(message: impl Into<String>, tone: StatusTone) -> Self {
        Self {
            message: message.into(),
            tone,
        }
    }
}

#[component]
pub fn SmfPage() -> Element {
    let tabs = use_signal(Vec::<SmfTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);

    toolkit_ui::use_tool_open(toolkit_ui::ToolKind::Smf, busy, move |files| {
        let files = files
            .into_iter()
            .map(toolkit_ui::ToolFile::into_file_data)
            .collect();
        load_files(files, tabs, active_tab_id, next_tab_id, busy, status);
    });
    let tabs_snapshot = tabs();
    let active_id_snapshot = active_tab_id();
    let active_tab = active_id_snapshot
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: SMF_PAGE_CSS }
        div {
            class: if dragging() { "smf-page-host is-dragging" } else { "smf-page-host" },
            ondragenter: move |event| {
                event.prevent_default();
                if !event.files().is_empty() { dragging.set(true); }
            },
            ondragover: move |event| {
                event.prevent_default();
                if !event.files().is_empty() { dragging.set(true); }
            },
            ondragleave: move |_| dragging.set(false),
            ondrop: move |event| {
                event.prevent_default();
                dragging.set(false);
                let files = event.files();
                if !files.is_empty() {
                    load_files(files, tabs, active_tab_id, next_tab_id, busy, status);
                }
            },
            ToolPage {
                namespace: "smf",
                class: if tabs_snapshot.is_empty() { "smf-page is-empty" } else { "smf-page" },
                if !tabs_snapshot.is_empty() {
                    TabStrip {
                        tabs: tabs_snapshot.clone(),
                        active_tab_id: active_id_snapshot,
                        busy: busy(),
                        on_activate: move |id| {
                            active_tab_id.set(Some(id));
                            status.set(AppStatus::default());
                        },
                        on_close: move |id| close_tab(tabs, active_tab_id, id, status),
                        on_files: move |files| load_files(files, tabs, active_tab_id, next_tab_id, busy, status),
                    }
                }
                WorkspaceCard { class: "smf-workspace-card", aria_label: "SMF Container",
                    if let Some(tab) = active_tab {
                        DocumentWorkspace { tab, tabs, active_tab_id, busy, status }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| load_files(files, tabs, active_tab_id, next_tab_id, busy, status),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "smf-status",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "松开以打开 SMF 或待压缩文件" }
                    }
                    if busy() {
                        div { class: "smf-busy", role: "status",
                            span {}
                            "正在后台处理 SMF…"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TabStrip(
    tabs: Vec<SmfTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "smf-tab-strip ui-document-tab-strip",
            div { class: "smf-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 SMF 文件",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: if Some(tab.id) == active_tab_id { "smf-tab ui-document-tab is-active" } else { "smf-tab ui-document-tab" },
                        button {
                            class: "smf-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            title: "{tab.document.name}",
                            disabled: busy,
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "smf-tab-dot ui-document-tab-dot" }
                            span { class: "smf-tab-name ui-document-tab-name", "{tab.document.name}" }
                            if tab.dirty() { span { class: "smf-tab-unsaved", title: "压缩设置尚未应用" } }
                        }
                        button {
                            class: "smf-tab-close ui-document-tab-close",
                            title: "关闭 {tab.document.name}",
                            aria_label: "关闭 {tab.document.name}",
                            disabled: busy,
                            onclick: move |_| on_close.call(tab.id),
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: if busy { "smf-tab-add ui-document-new-tab is-disabled" } else { "smf-tab-add ui-document-new-tab" },
                    title: "添加文件",
                    aria_label: "添加文件",
                    Glyph { name: "add" }
                    input {
                        class: "smf-file-input",
                        r#type: "file",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() { on_files.call(files); }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn EmptyWorkspace(busy: bool, on_files: EventHandler<Vec<FileData>>) -> Element {
    rsx! {
        section { class: "smf-empty",
            div { class: "smf-empty-visual", aria_hidden: "true",
                span { class: "smf-empty-file", Glyph { name: "file" } }
                i { class: "smf-empty-arrow", "→" }
                span { class: "smf-empty-zlib", "zlib" }
                i { class: "smf-empty-arrow", "→" }
                span { class: "smf-empty-file is-smf", Glyph { name: "archive" } }
            }
            span { class: "smf-eyebrow", "SMF CONTAINER" }
            h2 { "压缩或解开 PopCap SMF" }
            p { "拖入任意文件可封装为 SMF；拖入现成 SMF 会自动识别 8/16 字节头并解压负载。" }
            label { class: if busy { "smf-primary is-disabled" } else { "smf-primary" },
                Glyph { name: "open" }
                "选择文件"
                input {
                    class: "smf-file-input",
                    r#type: "file",
                    multiple: true,
                    disabled: busy,
                    onchange: move |event| {
                        let files = event.files();
                        if !files.is_empty() { on_files.call(files); }
                    }
                }
            }
            small { "支持原始文件、.smf 与多文件批量打开" }
        }
    }
}

#[component]
fn DocumentWorkspace(
    tab: SmfTab,
    tabs: Signal<Vec<SmfTab>>,
    active_tab_id: Signal<Option<u64>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
) -> Element {
    let metadata = &tab.document.metadata;
    let dirty = tab.dirty();
    let source_label = match tab.document.source_kind {
        SourceKind::Smf => "SMF 输入",
        SourceKind::Raw => "原始输入",
    };
    let ratio = compression_percent(metadata);
    let preview = preview_lines(&tab.document.payload);
    rsx! {
        div { class: "smf-document",
            header { class: "smf-document-header",
                div {
                    span { class: "smf-eyebrow", "SMF WORKSPACE" }
                    h2 { "{tab.document.name}" }
                    p { "{source_label} · {tab.document.payload_kind}" }
                }
                div { class: "smf-header-actions",
                    button {
                        class: "smf-action",
                        title: "导出解压后的原始负载",
                        disabled: busy(),
                        onclick: {
                            let tab = tab.clone();
                            move |_| export_payload(tab.clone(), status)
                        },
                        Glyph { name: "download" }
                        "原始文件"
                    }
                    button {
                        class: "smf-action primary",
                        title: if dirty { "请先应用压缩设置" } else { "导出 SMF" },
                        disabled: busy() || dirty,
                        onclick: {
                            let tab = tab.clone();
                            move |_| export_smf(tab.clone(), status)
                        },
                        Glyph { name: "archive" }
                        "SMF"
                    }
                    button {
                        class: "smf-icon-action",
                        title: if dirty { "请先应用压缩设置" } else { "导出 .tag.smf" },
                        aria_label: "导出 Tag",
                        disabled: busy() || dirty,
                        onclick: {
                            let tab = tab.clone();
                            move |_| export_tag(tab.clone(), status)
                        },
                        Glyph { name: "shield" }
                    }
                }
            }

            div { class: "smf-summary-grid",
                SummaryCard { glyph: "file", label: "原始大小", value: format_bytes(metadata.uncompressed_size) }
                SummaryCard { glyph: "archive", label: "SMF 大小", value: format_bytes(metadata.total_size) }
                SummaryCard { glyph: "compress", label: "压缩率", value: ratio }
                SummaryCard { glyph: "header", label: "头部", value: metadata.variant.label().to_string() }
            }

            div { class: "smf-main-grid",
                section { class: "smf-card smf-flow-card",
                    PanelHeading { eyebrow: "CONTAINER", title: "文件流", glyph: "archive" }
                    div { class: "smf-flow",
                        div { span { Glyph { name: "file" } } strong { "Payload" } small { "{tab.document.payload_kind}" } }
                        i { "→" }
                        div { class: "is-codec", span { Glyph { name: "compress" } } strong { "zlib" } small { "Level {tab.compression_level}" } }
                        i { "→" }
                        div { span { Glyph { name: "archive" } } strong { "SMF" } small { "{tab.variant.label()}" } }
                    }
                    dl { class: "smf-file-pair",
                        div { dt { "负载文件" } dd { "{tab.document.payload_name}" } }
                        div { dt { "容器文件" } dd { "{tab.document.smf_name}" } }
                        div { dt { "Tag 文件" } dd { "{tab.document.tag_name}" } }
                        div { dt { "MD5" } dd { class: "smf-digest", "{metadata.md5}" } }
                    }
                }

                section { class: "smf-card smf-settings-card",
                    PanelHeading { eyebrow: "ENCODING", title: "压缩设置", glyph: "settings" }
                    label { class: "smf-field",
                        span { "头部布局" }
                        select {
                            value: variant_code(tab.variant),
                            disabled: busy(),
                            onchange: move |event| {
                                let variant = if event.value() == "extended" { HeaderVariant::Extended64 } else { HeaderVariant::Compact32 };
                                update_tab_settings(tabs, active_tab_id, Some(variant), None);
                            },
                            option { value: "compact", "32-bit · 8-byte header" }
                            option { value: "extended", "64-bit · 16-byte header" }
                        }
                    }
                    label { class: "smf-field smf-level-field",
                        span { "zlib 级别" output { "{tab.compression_level}" } }
                        input {
                            r#type: "range",
                            min: "0",
                            max: "9",
                            step: "1",
                            value: "{tab.compression_level}",
                            disabled: busy(),
                            oninput: move |event| {
                                if let Ok(level) = event.value().parse::<u32>() {
                                    update_tab_settings(tabs, active_tab_id, None, Some(level));
                                }
                            }
                        }
                        div { class: "smf-level-labels", span { "Store" } span { "Balanced" } span { "Best" } }
                    }
                    div { class: "smf-presets",
                        button { disabled: busy(), onclick: move |_| update_tab_settings(tabs, active_tab_id, None, Some(1)), "快速" }
                        button { disabled: busy(), onclick: move |_| update_tab_settings(tabs, active_tab_id, None, Some(6)), "平衡" }
                        button { disabled: busy(), onclick: move |_| update_tab_settings(tabs, active_tab_id, None, Some(9)), "最小" }
                    }
                    button {
                        class: if dirty { "smf-rebuild is-dirty" } else { "smf-rebuild" },
                        disabled: busy(),
                        onclick: move |_| rebuild_active(tabs, active_tab_id, busy, status),
                        Glyph { name: "refresh" }
                        if dirty { "应用压缩设置" } else { "重新压缩" }
                    }
                    if dirty {
                        p { class: "smf-setting-note warning", "设置已修改；应用后才能导出新的 SMF 与 Tag。" }
                    } else {
                        p { class: "smf-setting-note", "压缩在后台线程或 Web Worker 中执行，不阻塞界面。" }
                    }
                }

                section { class: "smf-card smf-preview-card",
                    PanelHeading { eyebrow: "PAYLOAD", title: "数据预览", glyph: "code" }
                    div { class: "smf-preview-meta",
                        span { "前 {tab.document.payload.len().min(256)} / {tab.document.payload.len()} 字节" }
                        span { "{tab.document.payload_kind}" }
                    }
                    div { class: "smf-hex", role: "region", aria_label: "负载十六进制预览",
                        for line in preview {
                            div { class: "smf-hex-line",
                                code { class: "offset", "{line.offset}" }
                                code { class: "bytes", "{line.bytes}" }
                                code { class: "ascii", "{line.ascii}" }
                            }
                        }
                        if tab.document.payload.is_empty() {
                            div { class: "smf-hex-empty", "负载为空" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SummaryCard(glyph: &'static str, label: &'static str, value: String) -> Element {
    rsx! {
        article { class: "smf-summary-card",
            span { Glyph { name: glyph } }
            div { small { "{label}" } strong { "{value}" } }
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: &'static str, glyph: &'static str) -> Element {
    rsx! {
        div { class: "smf-panel-heading",
            div { span { class: "smf-eyebrow", "{eyebrow}" } h3 { "{title}" } }
            span { Glyph { name: glyph } }
        }
    }
}

#[derive(Clone)]
struct PreviewLine {
    offset: String,
    bytes: String,
    ascii: String,
}

fn preview_lines(data: &[u8]) -> Vec<PreviewLine> {
    data[..data.len().min(256)]
        .chunks(16)
        .enumerate()
        .map(|(index, chunk)| PreviewLine {
            offset: format!("{:08X}", index * 16),
            bytes: chunk
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
            ascii: chunk
                .iter()
                .map(|byte| {
                    if byte.is_ascii_graphic() || *byte == b' ' {
                        char::from(*byte)
                    } else {
                        '·'
                    }
                })
                .collect(),
        })
        .collect()
}

fn load_files(
    files: Vec<FileData>,
    mut tabs: Signal<Vec<SmfTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    mut next_tab_id: Signal<u64>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if files.is_empty() || busy() {
        return;
    }
    let pending = files
        .into_iter()
        .map(|file| {
            let id = next_tab_id();
            next_tab_id.set(id.wrapping_add(1).max(1));
            (id, file)
        })
        .collect::<Vec<_>>();
    let count = pending.len();
    busy.set(true);
    status.set(AppStatus::new(
        if count == 1 {
            "正在读取并分析文件…".to_string()
        } else {
            format!("正在读取并分析 {count} 个文件…")
        },
        StatusTone::Neutral,
    ));
    spawn(async move {
        let mut opened = Vec::new();
        let mut errors = Vec::new();
        for (index, (id, file)) in pending.into_iter().enumerate() {
            let name = file.name();
            if count > 1 {
                status.set(AppStatus::new(
                    format!("正在处理文件（{}/{count}）…", index + 1),
                    StatusTone::Neutral,
                ));
            }
            let result = match file.read_bytes().await {
                Ok(bytes) => {
                    processing::prepare_document(PrepareRequest {
                        name: name.clone(),
                        data: bytes.to_vec(),
                    })
                    .await
                }
                Err(error) => Err(format!("无法读取文件：{error}")),
            };
            match result {
                Ok(prepared) => {
                    push_application_log(
                        "SMF",
                        "INFO",
                        "OPEN",
                        format!("Opened {name} ({})", prepared.payload_kind),
                    );
                    opened.push(SmfTab::new(id, prepared));
                }
                Err(error) => {
                    push_application_log(
                        "SMF",
                        "ERROR",
                        "OPEN",
                        format!("Open {name} failed: {error}"),
                    );
                    errors.push(format!("{name}：{error}"));
                }
            }
        }
        busy.set(false);
        if let Some(last) = opened.last() {
            active_tab_id.set(Some(last.id));
        }
        let opened_count = opened.len();
        tabs.write().extend(opened);
        status.set(if errors.is_empty() {
            AppStatus::new(
                format!("已打开 {opened_count} 个文件。"),
                StatusTone::Success,
            )
        } else if opened_count == 0 {
            AppStatus::new(errors.join("；"), StatusTone::Error)
        } else {
            AppStatus::new(
                format!("已打开 {opened_count} 个文件；{} 个失败。", errors.len()),
                StatusTone::Warning,
            )
        });
    });
}

fn close_tab(
    mut tabs: Signal<Vec<SmfTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    id: u64,
    mut status: Signal<AppStatus>,
) {
    let mut tabs_write = tabs.write();
    let Some(index) = tabs_write.iter().position(|tab| tab.id == id) else {
        return;
    };
    let was_active = active_tab_id() == Some(id);
    tabs_write.remove(index);
    if was_active {
        let next = index.min(tabs_write.len().saturating_sub(1));
        active_tab_id.set(tabs_write.get(next).map(|tab| tab.id));
    }
    drop(tabs_write);
    status.set(AppStatus::default());
}

fn update_tab_settings(
    mut tabs: Signal<Vec<SmfTab>>,
    active_tab_id: Signal<Option<u64>>,
    variant: Option<HeaderVariant>,
    level: Option<u32>,
) {
    let Some(id) = active_tab_id() else {
        return;
    };
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == id) {
        if let Some(variant) = variant {
            tab.variant = variant;
        }
        if let Some(level) = level {
            tab.compression_level = level.min(9);
        }
    }
}

fn rebuild_active(
    mut tabs: Signal<Vec<SmfTab>>,
    active_tab_id: Signal<Option<u64>>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() {
        return;
    }
    let Some(id) = active_tab_id() else {
        return;
    };
    let Some(tab) = tabs().iter().find(|tab| tab.id == id).cloned() else {
        return;
    };
    busy.set(true);
    status.set(AppStatus::new("正在重新压缩 SMF…", StatusTone::Neutral));
    spawn(async move {
        let result = processing::rebuild_document(RebuildRequest {
            payload: tab.document.payload.as_ref().to_vec(),
            variant: tab.variant,
            compression_level: tab.compression_level,
        })
        .await;
        busy.set(false);
        match result {
            Ok(rebuilt) => {
                if let Some(current) = tabs.write().iter_mut().find(|candidate| candidate.id == id)
                {
                    let mut document = (*current.document).clone();
                    document.encoded = Arc::from(rebuilt.encoded);
                    document.metadata = rebuilt.metadata;
                    current.document = Arc::new(document);
                    current.built_variant = current.variant;
                    current.built_level = current.compression_level;
                }
                push_application_log(
                    "SMF",
                    "INFO",
                    "ENCODE",
                    format!("Rebuilt {}", tab.document.smf_name),
                );
                status.set(AppStatus::new(
                    "SMF 已重新压缩，可以导出。",
                    StatusTone::Success,
                ));
            }
            Err(error) => {
                push_application_log("SMF", "ERROR", "ENCODE", format!("Rebuild failed: {error}"));
                status.set(AppStatus::new(error, StatusTone::Error));
            }
        }
    });
}

fn export_payload(tab: SmfTab, mut status: Signal<AppStatus>) {
    spawn(async move {
        export_bytes(
            &tab.document.payload_name,
            "原始文件",
            &[],
            tab.document.payload.as_ref(),
            "PAYLOAD",
            &mut status,
        )
        .await;
    });
}

fn export_smf(tab: SmfTab, mut status: Signal<AppStatus>) {
    spawn(async move {
        export_bytes(
            &tab.document.smf_name,
            "SMF 文件",
            &["smf"],
            tab.document.encoded.as_ref(),
            "SMF",
            &mut status,
        )
        .await;
    });
}

fn export_tag(tab: SmfTab, mut status: Signal<AppStatus>) {
    spawn(async move {
        let contents = format!("{}\r\n", tab.document.metadata.md5);
        export_bytes(
            &tab.document.tag_name,
            "SMF Tag",
            &["smf"],
            contents.as_bytes(),
            "TAG",
            &mut status,
        )
        .await;
    });
}

async fn export_bytes(
    name: &str,
    filter: &str,
    extensions: &[&str],
    bytes: &[u8],
    kind: &str,
    status: &mut Signal<AppStatus>,
) {
    match platform::save_bytes(name, filter, extensions, bytes).await {
        Ok(true) => {
            push_application_log("SMF", "INFO", "EXPORT", format!("Exported {name} ({kind})"));
            status.set(AppStatus::new(
                format!("已导出 {name}。"),
                StatusTone::Success,
            ));
        }
        Ok(false) => {}
        Err(error) => {
            push_application_log(
                "SMF",
                "ERROR",
                "EXPORT",
                format!("Export {name} failed: {error}"),
            );
            status.set(AppStatus::new(error, StatusTone::Error));
        }
    }
}

fn compression_percent(metadata: &DocumentMetadata) -> String {
    if metadata.uncompressed_size == 0 {
        "0.0%".to_string()
    } else {
        format!(
            "{:.1}%",
            metadata.compressed_size as f64 / metadata.uncompressed_size as f64 * 100.0
        )
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

const fn variant_code(variant: HeaderVariant) -> &'static str {
    match variant {
        HeaderVariant::Compact32 => "compact",
        HeaderVariant::Extended64 => "extended",
    }
}

#[component]
fn Glyph(name: &'static str) -> Element {
    let path = match name {
        "open" => "M3 7h6l2 2h10v10H3zM3 7V5h7l2 2",
        "add" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "file" => "M6 3h8l4 4v14H6zM14 3v5h5",
        "archive" => "M4 5h16v4H4zM5 9h14v11H5zM10 13h4",
        "download" => "M12 3v12M7 10l5 5 5-5M5 20h14",
        "shield" => "M12 3l7 3v5c0 5-3 8-7 10-4-2-7-5-7-10V6zM9 12l2 2 4-5",
        "compress" => "M8 3v6H2M16 3v6h6M8 21v-6H2M16 21v-6h6",
        "header" => "M4 5h16v14H4zM4 9h16M8 5v4",
        "settings" => "M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M6 14v6",
        "refresh" => "M20 7v5h-5M4 17v-5h5M6.1 9a7 7 0 0 1 11.8-2L20 10M4 14l2.1 3A7 7 0 0 0 18 15",
        "code" => "M8 9l-4 3 4 3M16 9l4 3-4 3M14 5l-4 14",
        _ => "M4 12h16",
    };
    rsx! {
        svg { class: "smf-glyph", view_box: "0 0 24 24",
            path { d: path, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_bounded_and_readable() {
        let lines = preview_lines(&(0_u8..=255).cycle().take(1024).collect::<Vec<_>>());
        assert_eq!(lines.len(), 16);
        assert_eq!(lines[0].offset, "00000000");
        assert_eq!(lines[15].offset, "000000F0");
    }

    #[test]
    fn byte_format_and_variant_codes_are_stable() {
        assert_eq!(format_bytes(1024), "1.00 KiB");
        assert_eq!(variant_code(HeaderVariant::Compact32), "compact");
        assert_eq!(variant_code(HeaderVariant::Extended64), "extended");
    }
}
