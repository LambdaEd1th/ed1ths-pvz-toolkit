use std::sync::Arc;

use compiled_text_worker::{DecodeRequest, DocumentMetadata, EncodeRequest, HeaderVariant};
use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use toolkit_ui::{DropIndicator, InlineNotice, ToolPage, WorkspaceCard, push_application_log};

use crate::{platform, processing};

const PAGE_CSS: Asset = asset!("/assets/compiled-text/page.css");
const DEFAULT_MAX_OUTPUT_SIZE: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceKind {
    Plain,
    Compiled,
}

impl SourceKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Plain => "原始文本",
            Self::Compiled => "Compiled Text",
        }
    }
}

#[derive(Clone, PartialEq)]
struct CompiledTextTab {
    id: u64,
    name: String,
    source: Arc<[u8]>,
    source_kind: SourceKind,
    text: String,
    unlocked: bool,
    invalid_utf8: bool,
    seed: String,
    variant: HeaderVariant,
    compression_level: u32,
    encoded: Option<Arc<[u8]>>,
    metadata: Option<DocumentMetadata>,
    generation: u64,
    encoded_generation: Option<u64>,
}

impl CompiledTextTab {
    fn from_file(id: u64, name: String, bytes: Vec<u8>) -> Self {
        let source_kind = detect_source_kind(&name, &bytes);
        let unlocked = source_kind == SourceKind::Plain;
        let invalid_utf8 = unlocked && std::str::from_utf8(&bytes).is_err();
        let text = if unlocked {
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            String::new()
        };
        let encoded =
            (source_kind == SourceKind::Compiled).then(|| Arc::<[u8]>::from(bytes.clone()));
        Self {
            id,
            name,
            source: Arc::from(bytes),
            source_kind,
            text,
            unlocked,
            invalid_utf8,
            seed: String::new(),
            variant: HeaderVariant::Compact32,
            compression_level: 9,
            encoded,
            metadata: None,
            generation: 0,
            encoded_generation: (source_kind == SourceKind::Compiled).then_some(0),
        }
    }

    fn blank(id: u64) -> Self {
        Self::from_file(id, format!("untitled-{id}.txt"), Vec::new())
    }

    fn encoded_current(&self) -> bool {
        self.encoded.is_some() && self.encoded_generation == Some(self.generation)
    }

    fn plain_name(&self) -> String {
        plain_output_name(&self.name)
    }

    fn compiled_name(&self) -> String {
        compiled_output_name(&self.name)
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
pub fn CompiledTextPage() -> Element {
    let tabs = use_signal(Vec::<CompiledTextTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);

    toolkit_ui::use_tool_open(toolkit_ui::ToolKind::CompiledText, busy, move |files| {
        let files = files
            .into_iter()
            .map(toolkit_ui::ToolFile::into_file_data)
            .collect();
        load_files(files, tabs, active_tab_id, next_tab_id, busy, status);
    });
    let tabs_snapshot = tabs();
    let active_id = active_tab_id();
    let active_tab = active_id
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: PAGE_CSS }
        div {
            class: if dragging() { "compiled-page-host is-dragging" } else { "compiled-page-host" },
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
                namespace: "compiled-text",
                class: if tabs_snapshot.is_empty() { "compiled-page is-empty" } else { "compiled-page" },
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
                        on_new: move |_| create_blank_tab(tabs, active_tab_id, next_tab_id, status),
                    }
                }
                WorkspaceCard { class: "compiled-workspace-card", aria_label: "Compiled Text",
                    if let Some(tab) = active_tab {
                        DocumentWorkspace { tab, tabs, busy, status }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| load_files(files, tabs, active_tab_id, next_tab_id, busy, status),
                            on_new: move |_| create_blank_tab(tabs, active_tab_id, next_tab_id, status),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "compiled-status",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "松开以打开原始文本或 Compiled Text" }
                    }
                    if busy() {
                        div { class: "compiled-busy", role: "status",
                            span {}
                            "正在后台处理 Compiled Text…"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TabStrip(
    tabs: Vec<CompiledTextTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
    on_new: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        nav { class: "compiled-tab-strip ui-document-tab-strip",
            div { class: "compiled-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 Compiled Text 文件",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: if Some(tab.id) == active_tab_id { "compiled-tab ui-document-tab is-active" } else { "compiled-tab ui-document-tab" },
                        button {
                            class: "compiled-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            title: "{tab.name}",
                            disabled: busy,
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "compiled-tab-dot ui-document-tab-dot" }
                            span { class: "compiled-tab-name ui-document-tab-name", "{tab.name}" }
                            if tab.unlocked && !tab.encoded_current() {
                                span { class: "compiled-tab-unsaved", title: "文本或参数尚未编码" }
                            }
                        }
                        button {
                            class: "compiled-tab-close ui-document-tab-close",
                            title: "关闭 {tab.name}",
                            aria_label: "关闭 {tab.name}",
                            disabled: busy,
                            onclick: move |_| on_close.call(tab.id),
                            Glyph { name: "close" }
                        }
                    }
                }
                button {
                    class: "compiled-new-text",
                    title: "新建文本",
                    aria_label: "新建文本",
                    disabled: busy,
                    onclick: move |event| on_new.call(event),
                    Glyph { name: "new" }
                }
                label {
                    class: if busy { "compiled-tab-add ui-document-new-tab is-disabled" } else { "compiled-tab-add ui-document-new-tab" },
                    title: "打开文件",
                    aria_label: "打开文件",
                    Glyph { name: "add" }
                    input {
                        class: "compiled-file-input",
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
fn EmptyWorkspace(
    busy: bool,
    on_files: EventHandler<Vec<FileData>>,
    on_new: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section { class: "compiled-empty",
            div { class: "compiled-empty-visual", aria_hidden: "true",
                span { Glyph { name: "text" } }
                i { "→" }
                span { class: "is-codec", Glyph { name: "key" } }
                i { "→" }
                span { Glyph { name: "compiled" } }
            }
            span { class: "compiled-eyebrow", "COMPILED TEXT WORKSPACE" }
            h2 { "编辑、编码或解码 Compiled Text" }
            p { "本地完成 SMF/zlib、Rijndael-192-CBC 与 Base64 流程；每个文件保留独立 Seed 和编码参数。" }
            div { class: "compiled-empty-actions",
                label { class: if busy { "compiled-primary is-disabled" } else { "compiled-primary" },
                    Glyph { name: "open" }
                    "打开文件"
                    input {
                        class: "compiled-file-input",
                        r#type: "file",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() { on_files.call(files); }
                        }
                    }
                }
                button {
                    class: "compiled-secondary",
                    disabled: busy,
                    onclick: move |event| on_new.call(event),
                    Glyph { name: "new" }
                    "新建文本"
                }
            }
            small { "支持多文件标签、拖拽打开、32/64-bit SMF 头和 0–9 级 zlib 压缩" }
        }
    }
}

#[component]
fn DocumentWorkspace(
    tab: CompiledTextTab,
    tabs: Signal<Vec<CompiledTextTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
) -> Element {
    let tab_for_decode = tab.clone();
    let tab_for_encode = tab.clone();
    let tab_for_plain = tab.clone();
    let tab_for_compiled = tab.clone();
    let metadata = tab.metadata;
    let encoded_size_label = metadata
        .map(|value| format_bytes(value.encoded_size))
        .unwrap_or_else(|| "—".to_string());
    let container_size_label = metadata
        .map(|value| format_bytes(value.container_size))
        .unwrap_or_else(|| "—".to_string());
    let current_input_size = if tab.unlocked {
        tab.text.len() as u64
    } else {
        tab.source.len() as u64
    };
    rsx! {
        div { class: "compiled-document",
            header { class: "compiled-document-header",
                div {
                    span { class: "compiled-eyebrow", "COMPILED TEXT" }
                    h2 { "{tab.name}" }
                    p { "{tab.source_kind.label()} · {format_bytes(current_input_size)}" }
                }
                div { class: "compiled-header-actions",
                    button {
                        class: "compiled-action",
                        title: "导出原始文本",
                        disabled: busy() || !tab.unlocked,
                        onclick: move |_| export_plain(tab_for_plain.clone(), status),
                        Glyph { name: "text" }
                        "导出文本"
                    }
                    button {
                        class: "compiled-action primary",
                        title: if tab.encoded_current() { "导出 Compiled Text" } else { "请先应用编码" },
                        disabled: busy() || !tab.encoded_current(),
                        onclick: move |_| export_compiled(tab_for_compiled.clone(), status),
                        Glyph { name: "download" }
                        "导出编译文件"
                    }
                }
            }

            div { class: "compiled-summary-grid",
                SummaryCard { label: "输入类型", value: tab.source_kind.label().to_string() }
                SummaryCard { label: "明文大小", value: metadata.map_or_else(|| if tab.unlocked { format_bytes(tab.text.len() as u64) } else { "待解码".to_string() }, |value| format_bytes(value.decoded_size)) }
                SummaryCard { label: "密文大小", value: metadata.map_or_else(|| "—".to_string(), |value| format_bytes(value.ciphertext_size)) }
                SummaryCard { label: "SMF 头", value: metadata.map_or(tab.variant, |value| value.variant).label().to_string() }
            }

            div { class: "compiled-main-grid",
                section { class: "compiled-card compiled-editor-card",
                    PanelHeading { eyebrow: "TEXT", title: "文本内容", glyph: "text" }
                    if tab.unlocked {
                        textarea {
                            class: "compiled-editor",
                            value: "{tab.text}",
                            spellcheck: "false",
                            disabled: busy(),
                            oninput: move |event| update_text(tab.id, tabs, event.value()),
                        }
                        div { class: "compiled-editor-meta",
                            span { "{tab.text.chars().count()} 字符" }
                            span { "{tab.text.lines().count().max(1)} 行" }
                            if tab.invalid_utf8 { strong { "输入含无效 UTF-8，编辑器使用了替换字符" } }
                        }
                    } else {
                        div { class: "compiled-locked",
                            Glyph { name: "lock" }
                            strong { "文本尚未解锁" }
                            p { "输入正确的 Seed 后执行解码。错误 Seed 通常会在 SMF 校验阶段被拒绝。" }
                        }
                    }
                }

                aside { class: "compiled-side-stack",
                    section { class: "compiled-card compiled-settings-card",
                        PanelHeading { eyebrow: "PARAMETERS", title: "编解码设置", glyph: "settings" }
                        div { class: "compiled-field-group",
                            span { class: "compiled-field-label", "输入类型" }
                            div { class: "compiled-segmented",
                                button {
                                    class: if tab.source_kind == SourceKind::Plain { "is-active" } else { "" },
                                    disabled: busy(),
                                    onclick: move |_| set_source_kind(tab.id, tabs, SourceKind::Plain, status),
                                    "原始文本"
                                }
                                button {
                                    class: if tab.source_kind == SourceKind::Compiled { "is-active" } else { "" },
                                    disabled: busy(),
                                    onclick: move |_| set_source_kind(tab.id, tabs, SourceKind::Compiled, status),
                                    "Compiled"
                                }
                            }
                        }
                        label { class: "compiled-field",
                            span { "Seed" small { "UTF-8" } }
                            input {
                                r#type: "password",
                                value: "{tab.seed}",
                                autocomplete: "off",
                                disabled: busy(),
                                placeholder: "输入加密 Seed",
                                oninput: move |event| update_seed(tab.id, tabs, event.value()),
                            }
                        }
                        div { class: "compiled-field-group",
                            span { class: "compiled-field-label", "SMF 头部" }
                            div { class: "compiled-segmented",
                                button {
                                    class: if tab.variant == HeaderVariant::Compact32 { "is-active" } else { "" },
                                    disabled: busy(),
                                    onclick: move |_| update_variant(tab.id, tabs, HeaderVariant::Compact32),
                                    "32-bit"
                                }
                                button {
                                    class: if tab.variant == HeaderVariant::Extended64 { "is-active" } else { "" },
                                    disabled: busy(),
                                    onclick: move |_| update_variant(tab.id, tabs, HeaderVariant::Extended64),
                                    "64-bit"
                                }
                            }
                        }
                        label { class: "compiled-field compiled-range-field",
                            span { "zlib 压缩级别" output { "{tab.compression_level}" } }
                            input {
                                r#type: "range",
                                min: "0",
                                max: "9",
                                step: "1",
                                value: "{tab.compression_level}",
                                disabled: busy(),
                                oninput: move |event| update_compression(tab.id, tabs, &event.value()),
                            }
                        }
                        if tab.source_kind == SourceKind::Compiled && !tab.unlocked {
                            button {
                                class: "compiled-process",
                                disabled: busy() || tab.seed.is_empty(),
                                onclick: move |_| decode_tab(tab_for_decode.clone(), tabs, busy, status),
                                Glyph { name: "unlock" }
                                "解码并打开文本"
                            }
                        } else {
                            button {
                                class: if tab.encoded_current() { "compiled-process" } else { "compiled-process is-dirty" },
                                disabled: busy() || tab.seed.is_empty(),
                                onclick: move |_| encode_tab(tab_for_encode.clone(), tabs, busy, status),
                                Glyph { name: "compiled" }
                                if tab.encoded.is_some() { "应用并重新编码" } else { "编码为 Compiled Text" }
                            }
                        }
                        p { class: "compiled-setting-note", "Seed 仅保存在当前本地标签页中；桌面端和 Web 端都在后台线程处理。" }
                    }

                    section { class: "compiled-card compiled-pipeline-card",
                        PanelHeading { eyebrow: "PIPELINE", title: "格式链路", glyph: "flow" }
                        div { class: "compiled-pipeline",
                            span { "UTF-8" }
                            i { "→" }
                            span { "SMF / zlib" }
                            i { "→" }
                            span { "Rijndael-192" }
                            i { "→" }
                            span { "Base64" }
                        }
                        dl { class: "compiled-metadata",
                            div { dt { "Base64 大小" } dd { "{encoded_size_label}" } }
                            div { dt { "SMF 容器" } dd { "{container_size_label}" } }
                            div { dt { "输出文件" } dd { "{tab.compiled_name()}" } }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SummaryCard(label: &'static str, value: String) -> Element {
    rsx! {
        article { class: "compiled-summary-card",
            small { "{label}" }
            strong { "{value}" }
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: &'static str, glyph: &'static str) -> Element {
    rsx! {
        div { class: "compiled-panel-heading",
            div { span { class: "compiled-eyebrow", "{eyebrow}" } h3 { "{title}" } }
            span { Glyph { name: glyph } }
        }
    }
}

fn create_blank_tab(
    mut tabs: Signal<Vec<CompiledTextTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    mut next_tab_id: Signal<u64>,
    mut status: Signal<AppStatus>,
) {
    let id = next_tab_id();
    next_tab_id.set(id.wrapping_add(1).max(1));
    tabs.write().push(CompiledTextTab::blank(id));
    active_tab_id.set(Some(id));
    status.set(AppStatus::new("已新建空白文本。", StatusTone::Success));
}

fn load_files(
    files: Vec<FileData>,
    mut tabs: Signal<Vec<CompiledTextTab>>,
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
        format!("正在读取 {count} 个文件…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let mut opened = Vec::new();
        let mut errors = Vec::new();
        for (id, file) in pending {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => {
                    let tab = CompiledTextTab::from_file(id, name.clone(), bytes.to_vec());
                    push_application_log(
                        "COMPILED-TEXT",
                        "INFO",
                        "OPEN",
                        format!("Opened {name} ({})", tab.source_kind.label()),
                    );
                    opened.push(tab);
                }
                Err(error) => errors.push(format!("{name}：{error}")),
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
    mut tabs: Signal<Vec<CompiledTextTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    id: u64,
    mut status: Signal<AppStatus>,
) {
    let mut list = tabs.write();
    let Some(index) = list.iter().position(|tab| tab.id == id) else {
        return;
    };
    let was_active = active_tab_id() == Some(id);
    list.remove(index);
    if was_active {
        let next = index.min(list.len().saturating_sub(1));
        active_tab_id.set(list.get(next).map(|tab| tab.id));
    }
    drop(list);
    status.set(AppStatus::default());
}

fn update_text(tab_id: u64, mut tabs: Signal<Vec<CompiledTextTab>>, value: String) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.text = value;
        tab.generation = tab.generation.wrapping_add(1);
        tab.invalid_utf8 = false;
    }
}

fn update_seed(tab_id: u64, mut tabs: Signal<Vec<CompiledTextTab>>, value: String) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if tab.unlocked && tab.seed != value {
            tab.generation = tab.generation.wrapping_add(1);
        }
        tab.seed = value;
    }
}

fn update_variant(tab_id: u64, mut tabs: Signal<Vec<CompiledTextTab>>, value: HeaderVariant) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id)
        && tab.variant != value
    {
        tab.variant = value;
        tab.generation = tab.generation.wrapping_add(1);
    }
}

fn update_compression(tab_id: u64, mut tabs: Signal<Vec<CompiledTextTab>>, value: &str) {
    let Ok(value) = value.parse::<u32>() else {
        return;
    };
    if value > 9 {
        return;
    }
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id)
        && tab.compression_level != value
    {
        tab.compression_level = value;
        tab.generation = tab.generation.wrapping_add(1);
    }
}

fn set_source_kind(
    tab_id: u64,
    mut tabs: Signal<Vec<CompiledTextTab>>,
    kind: SourceKind,
    mut status: Signal<AppStatus>,
) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if tab.source_kind == kind {
            return;
        }
        tab.source_kind = kind;
        tab.metadata = None;
        tab.generation = tab.generation.wrapping_add(1);
        match kind {
            SourceKind::Plain => {
                tab.text = String::from_utf8_lossy(&tab.source).into_owned();
                tab.invalid_utf8 = std::str::from_utf8(&tab.source).is_err();
                tab.unlocked = true;
                tab.encoded = None;
                tab.encoded_generation = None;
            }
            SourceKind::Compiled => {
                tab.text.clear();
                tab.invalid_utf8 = false;
                tab.unlocked = false;
                tab.encoded = Some(tab.source.clone());
                tab.encoded_generation = Some(tab.generation);
            }
        }
    }
    status.set(AppStatus::new(
        format!("已切换为{}输入。", kind.label()),
        StatusTone::Neutral,
    ));
}

fn decode_tab(
    tab: CompiledTextTab,
    mut tabs: Signal<Vec<CompiledTextTab>>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() || tab.seed.is_empty() {
        return;
    }
    let id = tab.id;
    let name = tab.name.clone();
    let request = DecodeRequest {
        data: tab.source.to_vec(),
        seed: tab.seed,
        max_output_size: DEFAULT_MAX_OUTPUT_SIZE,
        allow_base64_whitespace: true,
    };
    busy.set(true);
    status.set(AppStatus::new(
        format!("正在解码 {name}…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let result = processing::decode_document(request).await;
        busy.set(false);
        match result {
            Ok(response) => {
                let invalid_utf8 = std::str::from_utf8(&response.data).is_err();
                let text = String::from_utf8_lossy(&response.data).into_owned();
                if let Some(current) = tabs.write().iter_mut().find(|tab| tab.id == id) {
                    current.text = text;
                    current.unlocked = true;
                    current.invalid_utf8 = invalid_utf8;
                    current.variant = response.metadata.variant;
                    current.metadata = Some(response.metadata);
                    current.encoded_generation = Some(current.generation);
                }
                push_application_log(
                    "COMPILED-TEXT",
                    "INFO",
                    "DECODE",
                    format!("Decoded {name} ({} bytes)", response.metadata.decoded_size),
                );
                status.set(if invalid_utf8 {
                    AppStatus::new(
                        "解码完成，但明文含无效 UTF-8；编辑器以替换字符显示。",
                        StatusTone::Warning,
                    )
                } else {
                    AppStatus::new("解码完成。", StatusTone::Success)
                });
            }
            Err(error) => {
                push_application_log(
                    "COMPILED-TEXT",
                    "ERROR",
                    "DECODE",
                    format!("Decode {name} failed: {error}"),
                );
                status.set(AppStatus::new(
                    format!("解码失败：{error}"),
                    StatusTone::Error,
                ));
            }
        }
    });
}

fn encode_tab(
    tab: CompiledTextTab,
    mut tabs: Signal<Vec<CompiledTextTab>>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() || tab.seed.is_empty() || !tab.unlocked {
        return;
    }
    let id = tab.id;
    let name = tab.name.clone();
    let generation = tab.generation;
    let request = EncodeRequest {
        data: tab.text.into_bytes(),
        seed: tab.seed,
        variant: tab.variant,
        compression_level: tab.compression_level,
    };
    busy.set(true);
    status.set(AppStatus::new(
        format!("正在编码 {name}…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let result = processing::encode_document(request).await;
        busy.set(false);
        match result {
            Ok(response) => {
                let encoded_size = response.data.len();
                if let Some(current) = tabs.write().iter_mut().find(|tab| tab.id == id) {
                    current.encoded = Some(Arc::from(response.data));
                    current.metadata = Some(response.metadata);
                    current.encoded_generation = Some(generation);
                }
                push_application_log(
                    "COMPILED-TEXT",
                    "INFO",
                    "ENCODE",
                    format!("Encoded {name} ({encoded_size} bytes)"),
                );
                status.set(AppStatus::new(
                    "编码完成，可以导出编译文件。",
                    StatusTone::Success,
                ));
            }
            Err(error) => {
                push_application_log(
                    "COMPILED-TEXT",
                    "ERROR",
                    "ENCODE",
                    format!("Encode {name} failed: {error}"),
                );
                status.set(AppStatus::new(
                    format!("编码失败：{error}"),
                    StatusTone::Error,
                ));
            }
        }
    });
}

fn export_plain(tab: CompiledTextTab, mut status: Signal<AppStatus>) {
    let name = tab.plain_name();
    let bytes = tab.text.into_bytes();
    spawn(async move {
        match platform::save_bytes(&name, "文本文件", &["txt", "json", "xml"], &bytes).await {
            Ok(true) => {
                push_application_log(
                    "COMPILED-TEXT",
                    "INFO",
                    "EXPORT",
                    format!("Exported {name}"),
                );
                status.set(AppStatus::new(
                    format!("已导出 {name}。"),
                    StatusTone::Success,
                ));
            }
            Ok(false) => {}
            Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
        }
    });
}

fn export_compiled(tab: CompiledTextTab, mut status: Signal<AppStatus>) {
    let name = tab.compiled_name();
    let Some(bytes) = tab.encoded else {
        return;
    };
    spawn(async move {
        match platform::save_bytes(&name, "Compiled Text", &["compiled", "txt"], &bytes).await {
            Ok(true) => {
                push_application_log(
                    "COMPILED-TEXT",
                    "INFO",
                    "EXPORT",
                    format!("Exported {name}"),
                );
                status.set(AppStatus::new(
                    format!("已导出 {name}。"),
                    StatusTone::Success,
                ));
            }
            Ok(false) => {}
            Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
        }
    });
}

fn detect_source_kind(name: &str, data: &[u8]) -> SourceKind {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".compiled") || lower.ends_with(".compiled.txt") {
        return SourceKind::Compiled;
    }
    let compact = data
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if compact.is_empty() || !compact.len().is_multiple_of(4) {
        return SourceKind::Plain;
    }
    if !compact
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    {
        return SourceKind::Plain;
    }
    let padding = compact
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    let decoded_size = compact.len() / 4 * 3 - padding;
    if decoded_size > 0 && decoded_size.is_multiple_of(24) {
        SourceKind::Compiled
    } else {
        SourceKind::Plain
    }
}

fn plain_output_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".compiled.txt") {
        let stem = &name[..name.len() - ".compiled.txt".len()];
        return if stem.is_empty() {
            "decoded.txt".to_string()
        } else {
            format!("{stem}.txt")
        };
    }
    if lower.ends_with(".compiled") {
        let stem = &name[..name.len() - ".compiled".len()];
        return if stem.is_empty() {
            "decoded.txt".to_string()
        } else {
            stem.to_string()
        };
    }
    if name.is_empty() {
        "decoded.txt".to_string()
    } else {
        name.to_string()
    }
}

fn compiled_output_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".compiled") {
        name.to_string()
    } else if name.is_empty() {
        "untitled.txt.compiled".to_string()
    } else {
        format!("{name}.compiled")
    }
}

fn format_bytes(bytes: u64) -> String {
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
        "text" => "M5 4h14v16H5zM8 8h8M8 12h8M8 16h5",
        "compiled" => "M6 3h8l4 4v14H6zM14 3v5h5M9 13h6M9 17h6",
        "open" => "M3 7h6l2 2h10v10H3zM3 7V5h7l2 2",
        "new" => "M6 3h8l4 4v14H6zM14 3v5h5M12 11v7M8.5 14.5h7",
        "add" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "key" => "M14 9a5 5 0 1 1-1 6l-4 4H6v-3H3v-3l4-4a5 5 0 0 1 7 0z",
        "lock" => "M7 11V8a5 5 0 0 1 10 0v3M5 11h14v10H5z",
        "unlock" => "M8 11V8a4 4 0 0 1 7.7-1.5M5 11h14v10H5z",
        "download" => "M12 3v12M7 10l5 5 5-5M5 20h14",
        "settings" => "M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M6 14v6",
        "flow" => "M4 7h6l3 5 3-5h4M4 17h5l3-5 3 5h5",
        _ => "M4 12h16",
    };
    rsx! {
        svg { class: "compiled-glyph", view_box: "0 0 24 24",
            path { d: path, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_compiled_text_without_requiring_an_extension() {
        let encoded = b"QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB";
        assert_eq!(detect_source_kind("data", encoded), SourceKind::Compiled);
        assert_eq!(detect_source_kind("data.txt", b"hello"), SourceKind::Plain);
        assert_eq!(
            detect_source_kind("data.compiled", b"not base64"),
            SourceKind::Compiled
        );
    }

    #[test]
    fn output_names_are_stable() {
        assert_eq!(plain_output_name("data.txt.compiled"), "data.txt");
        assert_eq!(compiled_output_name("data.txt"), "data.txt.compiled");
    }
}
