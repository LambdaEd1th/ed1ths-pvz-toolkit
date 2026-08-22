use std::sync::Arc;

use crypt_data::{DEFAULT_KEY, DEFAULT_LIMIT, HEADER_SIZE, MAGIC};
use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use toolkit_ui::{DropIndicator, InlineNotice, ToolPage, WorkspaceCard, push_application_log};

use crate::{
    platform,
    processing::{self, TransformMode},
};

const CRYPT_DATA_PAGE_CSS: Asset = asset!("/assets/crypt-data/page.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceKind {
    Raw,
    Wrapped,
}

impl SourceKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Raw => "原始文件",
            Self::Wrapped => "CRYPT_RES",
        }
    }

    const fn mode(self) -> TransformMode {
        match self {
            Self::Raw => TransformMode::Encrypt,
            Self::Wrapped => TransformMode::Decrypt,
        }
    }
}

#[derive(Clone)]
struct CryptDocument {
    name: String,
    source: Arc<[u8]>,
    source_kind: SourceKind,
    declared_raw_size: Option<usize>,
}

#[derive(Clone)]
struct CryptTab {
    id: u64,
    document: Arc<CryptDocument>,
    key: String,
    limit: usize,
    result: Option<Arc<[u8]>>,
    built_key: String,
    built_limit: usize,
}

impl PartialEq for CryptTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && Arc::ptr_eq(&self.document, &other.document)
            && self.key == other.key
            && self.limit == other.limit
            && option_arc_ptr_eq(&self.result, &other.result)
            && self.built_key == other.built_key
            && self.built_limit == other.built_limit
    }
}

impl CryptTab {
    fn new(id: u64, name: String, bytes: Vec<u8>) -> Self {
        let source_kind = detect_source_kind(&bytes);
        let declared_raw_size = declared_raw_size(&bytes);
        Self {
            id,
            document: Arc::new(CryptDocument {
                name,
                source: Arc::from(bytes),
                source_kind,
                declared_raw_size,
            }),
            key: String::from_utf8_lossy(DEFAULT_KEY).into_owned(),
            limit: DEFAULT_LIMIT,
            result: None,
            built_key: String::new(),
            built_limit: DEFAULT_LIMIT,
        }
    }

    fn dirty(&self) -> bool {
        self.result.is_none() || self.key != self.built_key || self.limit != self.built_limit
    }

    fn output_name(&self) -> String {
        output_name(&self.document.name, self.document.source_kind)
    }
}

fn option_arc_ptr_eq(left: &Option<Arc<[u8]>>, right: &Option<Arc<[u8]>>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
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
pub fn CryptDataPage() -> Element {
    let tabs = use_signal(Vec::<CryptTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);

    let tabs_snapshot = tabs();
    let active_id = active_tab_id();
    let active_tab = active_id
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: CRYPT_DATA_PAGE_CSS }
        div {
            class: if dragging() { "crypt-page-host is-dragging" } else { "crypt-page-host" },
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
                namespace: "crypt-data",
                class: if tabs_snapshot.is_empty() { "crypt-page is-empty" } else { "crypt-page" },
                if !tabs_snapshot.is_empty() {
                    TabStrip {
                        tabs: tabs_snapshot.clone(),
                        active_tab_id: active_id,
                        busy: busy(),
                        on_activate: move |id| {
                            active_tab_id.set(Some(id));
                            status.set(AppStatus::default());
                        },
                        on_close: move |id| close_tab(tabs, active_tab_id, id, status),
                        on_files: move |files| load_files(files, tabs, active_tab_id, next_tab_id, busy, status),
                    }
                }
                WorkspaceCard { class: "crypt-workspace-card", aria_label: "Crypt-Data",
                    if let Some(tab) = active_tab {
                        DocumentWorkspace { tab, tabs, busy, status }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| load_files(files, tabs, active_tab_id, next_tab_id, busy, status),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "crypt-status",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "松开以打开原始文件或 .cdat" }
                    }
                    if busy() {
                        div { class: "crypt-busy", role: "status",
                            span {}
                            "正在处理 Crypt-Data…"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TabStrip(
    tabs: Vec<CryptTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "crypt-tab-strip ui-document-tab-strip",
            div { class: "crypt-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 Crypt-Data 文件",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: if Some(tab.id) == active_tab_id { "crypt-tab ui-document-tab is-active" } else { "crypt-tab ui-document-tab" },
                        button {
                            class: "crypt-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            title: "{tab.document.name}",
                            disabled: busy,
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "crypt-tab-dot ui-document-tab-dot" }
                            span { class: "crypt-tab-name ui-document-tab-name", "{tab.document.name}" }
                            if tab.dirty() { span { class: "crypt-tab-unsaved", title: "转换参数尚未应用" } }
                        }
                        button {
                            class: "crypt-tab-close ui-document-tab-close",
                            title: "关闭 {tab.document.name}",
                            aria_label: "关闭 {tab.document.name}",
                            disabled: busy,
                            onclick: move |_| on_close.call(tab.id),
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: if busy { "crypt-tab-add ui-document-new-tab is-disabled" } else { "crypt-tab-add ui-document-new-tab" },
                    title: "添加文件",
                    aria_label: "添加文件",
                    Glyph { name: "add" }
                    input {
                        class: "crypt-file-input",
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
        section { class: "crypt-empty",
            div { class: "crypt-empty-visual", aria_hidden: "true",
                span { class: "crypt-empty-file", Glyph { name: "file" } }
                i { class: "crypt-empty-arrow", "→" }
                span { class: "crypt-empty-codec", Glyph { name: "key" } }
                i { class: "crypt-empty-arrow", "→" }
                span { class: "crypt-empty-file is-wrapped", Glyph { name: "lock" } }
            }
            span { class: "crypt-eyebrow", "CRYPT-DATA WORKSPACE" }
            h2 { "加密或解密 PopCap Crypt-Data" }
            p { "打开任意原始文件可生成 .cdat；打开带 CRYPT_RES 头部的文件会自动切换为解密流程。" }
            label { class: if busy { "crypt-primary is-disabled" } else { "crypt-primary" },
                Glyph { name: "open" }
                "选择文件"
                input {
                    class: "crypt-file-input",
                    r#type: "file",
                    multiple: true,
                    disabled: busy,
                    onchange: move |event| {
                        let files = event.files();
                        if !files.is_empty() { on_files.call(files); }
                    }
                }
            }
            small { "支持任意原始文件、.cdat、多文件标签页与拖拽打开" }
        }
    }
}

#[component]
fn DocumentWorkspace(
    tab: CryptTab,
    tabs: Signal<Vec<CryptTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
) -> Element {
    let dirty = tab.dirty();
    let result_size = tab
        .result
        .as_ref()
        .map_or_else(|| "待处理".to_string(), |bytes| format_bytes(bytes.len()));
    let output_kind = output_kind(&tab);
    let operation = operation_label(tab.document.source_kind);
    let tab_for_process = tab.clone();
    let tab_for_export = tab.clone();
    rsx! {
        div { class: "crypt-document",
            header { class: "crypt-document-header",
                div {
                    span { class: "crypt-eyebrow", "CRYPT-DATA WORKSPACE" }
                    h2 { "{tab.document.name}" }
                    p { "{tab.document.source_kind.label()} · 自动选择{operation}流程" }
                }
                div { class: "crypt-header-actions",
                    button {
                        class: "crypt-action primary",
                        title: if dirty { "请先应用转换参数" } else { "导出转换结果" },
                        disabled: busy() || dirty,
                        onclick: move |_| export_result(tab_for_export.clone(), status),
                        Glyph { name: "download" }
                        "导出结果"
                    }
                }
            }

            div { class: "crypt-summary-grid",
                SummaryCard { glyph: "file", label: "输入大小", value: format_bytes(tab.document.source.len()) }
                SummaryCard { glyph: "output", label: "输出大小", value: result_size }
                SummaryCard { glyph: "type", label: "输入类型", value: tab.document.source_kind.label().to_string() }
                SummaryCard { glyph: "limit", label: "加密范围", value: format!("{} bytes", tab.limit) }
            }

            div { class: "crypt-main-grid",
                FlowCard { tab: tab.clone() }
                SettingsCard {
                    key: "settings-{tab.id}",
                    tab: tab_for_process,
                    tabs,
                    busy,
                    status,
                }
                PreviewCard {
                    source: tab.document.source.clone(),
                    result: tab.result.clone(),
                    source_label: tab.document.source_kind.label(),
                    result_label: output_kind,
                }
            }
        }
    }
}

#[component]
fn SummaryCard(glyph: &'static str, label: &'static str, value: String) -> Element {
    rsx! {
        article { class: "crypt-summary-card",
            span { Glyph { name: glyph } }
            div { small { "{label}" } strong { "{value}" } }
        }
    }
}

#[component]
fn FlowCard(tab: CryptTab) -> Element {
    let output = output_kind(&tab);
    let declared = tab
        .document
        .declared_raw_size
        .map_or_else(|| "—".to_string(), format_bytes);
    rsx! {
        section { class: "crypt-card crypt-flow-card",
            PanelHeading { eyebrow: "TRANSFORM", title: "文件流", glyph: "swap" }
            div { class: "crypt-flow",
                div { span { Glyph { name: "file" } } strong { "{tab.document.source_kind.label()}" } small { "{format_bytes(tab.document.source.len())}" } }
                i { "→" }
                div { class: "is-codec", span { Glyph { name: "key" } } strong { "XOR" } small { "前 {tab.limit} bytes" } }
                i { "→" }
                div { span { Glyph { name: "output" } } strong { "{output}" } small { "{tab.output_name()}" } }
            }
            dl { class: "crypt-file-pair",
                div { dt { "输入文件" } dd { "{tab.document.name}" } }
                div { dt { "输出文件" } dd { "{tab.output_name()}" } }
                div { dt { "声明原始大小" } dd { "{declared}" } }
                div { dt { "格式头部" } dd { if tab.document.source_kind == SourceKind::Wrapped { "CRYPT_RES\\n\\0" } else { "无" } } }
            }
        }
    }
}

#[component]
fn SettingsCard(
    tab: CryptTab,
    tabs: Signal<Vec<CryptTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
) -> Element {
    let dirty = tab.dirty();
    let operation = operation_label(tab.document.source_kind);
    rsx! {
        section { class: "crypt-card crypt-settings-card",
            PanelHeading { eyebrow: "PARAMETERS", title: "转换设置", glyph: "settings" }
            label { class: "crypt-field",
                span { "密钥" small { "UTF-8" } }
                input {
                    r#type: "password",
                    value: "{tab.key}",
                    disabled: busy(),
                    autocomplete: "off",
                    oninput: move |event| update_settings(tab.id, tabs, Some(event.value()), None),
                }
            }
            label { class: "crypt-field",
                span { "加密范围" output { "{tab.limit} bytes" } }
                input {
                    r#type: "number",
                    min: "0",
                    step: "1",
                    value: "{tab.limit}",
                    disabled: busy(),
                    oninput: move |event| update_limit(tab.id, tabs, status, &event.value()),
                }
            }
            div { class: "crypt-presets",
                button { disabled: busy(), onclick: move |_| update_settings(tab.id, tabs, None, Some(64)), "64" }
                button { disabled: busy(), onclick: move |_| update_settings(tab.id, tabs, None, Some(128)), "128" }
                button { disabled: busy(), onclick: move |_| update_settings(tab.id, tabs, None, Some(DEFAULT_LIMIT)), "256" }
            }
            button {
                class: if dirty { "crypt-rebuild is-dirty" } else { "crypt-rebuild" },
                disabled: busy(),
                onclick: move |_| process_tab(tab.clone(), tabs, busy, status),
                Glyph { name: if tab.document.source_kind == SourceKind::Wrapped { "unlock" } else { "lock" } }
                if tab.result.is_none() {
                    "开始{operation}"
                } else if dirty {
                    "应用并重新{operation}"
                } else {
                    "重新{operation}"
                }
            }
            if tab.document.source_kind == SourceKind::Wrapped {
                p { class: "crypt-setting-note warning", "Crypt-Data 没有密钥校验码；错误密钥仍会产生输出，请结合文件内容确认。" }
            } else if tab.document.source.len() < tab.limit {
                p { class: "crypt-setting-note warning", "输入小于加密范围，将按格式规则原样透传；降低范围可强制生成 CRYPT_RES。" }
            } else {
                p { class: "crypt-setting-note", "桌面版在线程池处理大文件；密钥仅保存在当前本地标签页中。" }
            }
        }
    }
}

#[component]
fn PreviewCard(
    source: Arc<[u8]>,
    result: Option<Arc<[u8]>>,
    source_label: &'static str,
    result_label: &'static str,
) -> Element {
    let source_lines = preview_lines(&source);
    let result_lines = result.as_deref().map(preview_lines);
    rsx! {
        section { class: "crypt-card crypt-preview-card",
            PanelHeading { eyebrow: "BYTES", title: "数据预览", glyph: "code" }
            div { class: "crypt-preview-grid",
                HexPreview { label: "输入", kind: source_label, total: source.len(), lines: source_lines }
                if let Some(lines) = result_lines {
                    HexPreview { label: "输出", kind: result_label, total: result.as_ref().map_or(0, |bytes| bytes.len()), lines }
                } else {
                    div { class: "crypt-preview-pending",
                        Glyph { name: "output" }
                        strong { "输出待生成" }
                        small { "应用转换参数后可比较输入与输出字节" }
                    }
                }
            }
        }
    }
}

#[component]
fn HexPreview(
    label: &'static str,
    kind: &'static str,
    total: usize,
    lines: Vec<PreviewLine>,
) -> Element {
    rsx! {
        div { class: "crypt-preview-column",
            div { class: "crypt-preview-meta", span { "{label} · 前 {total.min(128)} / {total} 字节" } span { "{kind}" } }
            div { class: "crypt-hex", role: "region", aria_label: "{label}十六进制预览",
                for line in lines {
                    div { class: "crypt-hex-line",
                        code { class: "offset", "{line.offset}" }
                        code { class: "bytes", "{line.bytes}" }
                        code { class: "ascii", "{line.ascii}" }
                    }
                }
                if total == 0 { div { class: "crypt-hex-empty", "文件为空" } }
            }
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: &'static str, glyph: &'static str) -> Element {
    rsx! {
        div { class: "crypt-panel-heading",
            div { span { class: "crypt-eyebrow", "{eyebrow}" } h3 { "{title}" } }
            span { Glyph { name: glyph } }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct PreviewLine {
    offset: String,
    bytes: String,
    ascii: String,
}

fn preview_lines(data: &[u8]) -> Vec<PreviewLine> {
    data[..data.len().min(128)]
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
    mut tabs: Signal<Vec<CryptTab>>,
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
        format!("正在读取并分析 {count} 个文件…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let mut opened = Vec::new();
        let mut errors = Vec::new();
        for (id, file) in pending {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => {
                    let tab = CryptTab::new(id, name.clone(), bytes.to_vec());
                    push_application_log(
                        "CRYPT-DATA",
                        "INFO",
                        "OPEN",
                        format!("Opened {name} ({})", tab.document.source_kind.label()),
                    );
                    opened.push(tab);
                }
                Err(error) => {
                    push_application_log(
                        "CRYPT-DATA",
                        "ERROR",
                        "OPEN",
                        format!("Open {name} failed: {error}"),
                    );
                    errors.push(format!("{name}：无法读取文件：{error}"));
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
    mut tabs: Signal<Vec<CryptTab>>,
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

fn update_settings(
    tab_id: u64,
    mut tabs: Signal<Vec<CryptTab>>,
    key: Option<String>,
    limit: Option<usize>,
) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if let Some(key) = key {
            tab.key = key;
        }
        if let Some(limit) = limit {
            tab.limit = limit;
        }
    }
}

fn update_limit(
    tab_id: u64,
    tabs: Signal<Vec<CryptTab>>,
    mut status: Signal<AppStatus>,
    value: &str,
) {
    match value.parse::<usize>() {
        Ok(limit) => {
            update_settings(tab_id, tabs, None, Some(limit));
            status.set(AppStatus::default());
        }
        Err(_) => status.set(AppStatus::new(
            "加密范围必须是非负整数。",
            StatusTone::Error,
        )),
    }
}

fn process_tab(
    tab: CryptTab,
    mut tabs: Signal<Vec<CryptTab>>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() {
        return;
    }
    let id = tab.id;
    let operation = operation_label(tab.document.source_kind);
    let key = tab.key.as_bytes().to_vec();
    let key_snapshot = tab.key.clone();
    let limit = tab.limit;
    let source = tab.document.source.clone();
    let source_len = source.len();
    let source_kind = tab.document.source_kind;
    let name = tab.document.name.clone();
    busy.set(true);
    status.set(AppStatus::new(
        format!("正在{operation} {name}…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let result = processing::transform(source, source_kind.mode(), key, limit).await;
        busy.set(false);
        match result {
            Ok(bytes) => {
                let output_size = bytes.len();
                if let Some(current) = tabs.write().iter_mut().find(|tab| tab.id == id) {
                    current.result = Some(Arc::from(bytes));
                    current.built_key = key_snapshot;
                    current.built_limit = limit;
                }
                push_application_log(
                    "CRYPT-DATA",
                    "INFO",
                    if source_kind == SourceKind::Wrapped {
                        "DECRYPT"
                    } else {
                        "ENCRYPT"
                    },
                    format!("Processed {name} ({source_len} -> {output_size} bytes)"),
                );
                status.set(if source_kind == SourceKind::Raw && source_len < limit {
                    AppStatus::new(
                        "输入小于加密范围，已按格式规则原样透传。",
                        StatusTone::Warning,
                    )
                } else {
                    AppStatus::new(
                        format!("{operation}完成，可以导出结果。"),
                        StatusTone::Success,
                    )
                });
            }
            Err(error) => {
                push_application_log(
                    "CRYPT-DATA",
                    "ERROR",
                    if source_kind == SourceKind::Wrapped {
                        "DECRYPT"
                    } else {
                        "ENCRYPT"
                    },
                    format!("Process {name} failed: {error}"),
                );
                status.set(AppStatus::new(
                    format!("{operation}失败：{error}"),
                    StatusTone::Error,
                ));
            }
        }
    });
}

fn export_result(tab: CryptTab, mut status: Signal<AppStatus>) {
    let Some(result) = tab.result.clone() else {
        return;
    };
    let name = tab.output_name();
    let source_kind = tab.document.source_kind;
    spawn(async move {
        let (filter, extensions) = if source_kind == SourceKind::Raw {
            ("Crypt-Data 文件", vec!["cdat"])
        } else {
            ("解密文件", Vec::new())
        };
        match platform::save_bytes(&name, filter, &extensions, &result).await {
            Ok(true) => {
                push_application_log("CRYPT-DATA", "INFO", "EXPORT", format!("Exported {name}"));
                status.set(AppStatus::new(
                    format!("已导出 {name}。"),
                    StatusTone::Success,
                ));
            }
            Ok(false) => {}
            Err(error) => {
                push_application_log(
                    "CRYPT-DATA",
                    "ERROR",
                    "EXPORT",
                    format!("Export {name} failed: {error}"),
                );
                status.set(AppStatus::new(error, StatusTone::Error));
            }
        }
    });
}

fn detect_source_kind(bytes: &[u8]) -> SourceKind {
    if bytes.starts_with(&MAGIC) {
        SourceKind::Wrapped
    } else {
        SourceKind::Raw
    }
}

fn declared_raw_size(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < HEADER_SIZE || !bytes.starts_with(&MAGIC) {
        return None;
    }
    let value = u64::from_le_bytes(bytes[MAGIC.len()..HEADER_SIZE].try_into().ok()?);
    usize::try_from(value).ok()
}

fn output_kind(tab: &CryptTab) -> &'static str {
    match tab.document.source_kind {
        SourceKind::Wrapped => "原始文件",
        SourceKind::Raw
            if tab.document.source.len() >= tab.limit
                || tab
                    .result
                    .as_deref()
                    .is_some_and(|bytes| bytes.starts_with(&MAGIC)) =>
        {
            "CRYPT_RES"
        }
        SourceKind::Raw => "透传文件",
    }
}

const fn operation_label(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Raw => "加密",
        SourceKind::Wrapped => "解密",
    }
}

fn output_name(name: &str, kind: SourceKind) -> String {
    match kind {
        SourceKind::Raw => {
            if name.to_ascii_lowercase().ends_with(".cdat") {
                format!("{}.encrypted.cdat", &name[..name.len() - 5])
            } else {
                format!("{name}.cdat")
            }
        }
        SourceKind::Wrapped => {
            if name.to_ascii_lowercase().ends_with(".cdat") {
                let stem = &name[..name.len() - 5];
                if stem.is_empty() {
                    "decrypted.bin".to_string()
                } else {
                    stem.to_string()
                }
            } else {
                format!("{name}.decrypted")
            }
        }
    }
}

fn format_bytes(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
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

#[component]
fn Glyph(#[props(into)] name: String) -> Element {
    let path = match name.as_str() {
        "file" => "M6 3h8l4 4v14H6zM14 3v5h5",
        "open" => "M3 7h6l2 2h10v10H3zM3 7V5h7l2 2",
        "add" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "lock" => "M7 11V8a5 5 0 0 1 10 0v3M5 11h14v10H5z",
        "unlock" => "M8 11V8a4 4 0 0 1 7.7-1.5M5 11h14v10H5z",
        "key" => "M14 9a5 5 0 1 1-1 6l-4 4H6v-3H3v-3l4-4a5 5 0 0 1 7 0z",
        "download" => "M12 3v12M7 10l5 5 5-5M5 20h14",
        "output" => "M5 4h10l4 4v12H5zM15 4v5h5M9 14h6M12 11v6",
        "type" => "M4 6h16M8 6v14M16 6v14M6 20h4M14 20h4",
        "limit" => "M4 8V5h3M17 5h3v3M20 16v3h-3M7 19H4v-3M8 12h8",
        "swap" => "M4 8h14M14 4l4 4-4 4M20 16H6M10 12l-4 4 4 4",
        "settings" => "M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M6 14v6",
        "code" => "M8 9l-4 3 4 3M16 9l4 3-4 3M14 5l-4 14",
        "eye" => "M2 12s4-6 10-6 10 6 10 6-4 6-10 6S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
        "eye-off" => {
            "M3 3l18 18M10.6 6.2A10.5 10.5 0 0 1 12 6c6 0 10 6 10 6a17 17 0 0 1-2.3 2.9M6.2 6.2C3.5 8 2 12 2 12s4 6 10 6a9.8 9.8 0 0 0 3.8-.8M9.9 9.9a3 3 0 0 0 4.2 4.2"
        }
        _ => "M4 12h16",
    };
    rsx! {
        svg { class: "crypt-glyph", view_box: "0 0 24 24",
            path { d: path, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_magic_and_reads_declared_size() {
        let raw = vec![1; DEFAULT_LIMIT];
        let encoded = crypt_data::encrypt(&raw, DEFAULT_KEY).unwrap();
        assert_eq!(detect_source_kind(&encoded), SourceKind::Wrapped);
        assert_eq!(declared_raw_size(&encoded), Some(raw.len()));
        assert_eq!(detect_source_kind(&raw), SourceKind::Raw);
    }

    #[test]
    fn output_names_follow_cdat_conventions() {
        assert_eq!(output_name("data.bin", SourceKind::Raw), "data.bin.cdat");
        assert_eq!(output_name("data.cdat", SourceKind::Wrapped), "data");
        assert_eq!(
            output_name("data.bin", SourceKind::Wrapped),
            "data.bin.decrypted"
        );
    }

    #[test]
    fn preview_is_bounded() {
        let data = vec![0; 1024];
        assert_eq!(preview_lines(&data).len(), 8);
    }

    #[test]
    fn output_kind_forecasts_wrapping_before_processing() {
        let large = CryptTab::new(1, "large.bin".to_string(), vec![0; DEFAULT_LIMIT]);
        let small = CryptTab::new(2, "small.bin".to_string(), vec![0; DEFAULT_LIMIT - 1]);
        assert_eq!(output_kind(&large), "CRYPT_RES");
        assert_eq!(output_kind(&small), "透传文件");
    }
}
