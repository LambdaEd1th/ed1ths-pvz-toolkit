use std::collections::BTreeMap;
use std::sync::Arc;

use bnk_archive::{
    BankChunk, ChunkId, EmbeddedMediaLocation, HierarchyBody, HierarchyKind, HierarchyObject,
    SoundBank,
};
use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use toolkit_ui::{
    DropIndicator, InlineNotice, ToolPage, ToolPageToolbar, WorkspaceCard, push_application_log,
};

use crate::{BnkWemOpenRequest, platform};

const BNK_PAGE_CSS: Asset = asset!("/assets/bnk/page.css");
const BNK_FILE_ACCEPT: &str = ".bnk,application/octet-stream";
const ROW_PAGE_SIZE: usize = 160;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ParseMode {
    #[default]
    Strict,
    LosslessFallback,
}

impl ParseMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Strict => "严格模式",
            Self::LosslessFallback => "保留模式",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BrowserSection {
    #[default]
    Overview,
    Chunks,
    Hierarchy,
    Media,
}

impl BrowserSection {
    const fn label(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Chunks => "数据块",
            Self::Hierarchy => "HIRC 对象",
            Self::Media => "内嵌媒体",
        }
    }

    const fn glyph(self) -> &'static str {
        match self {
            Self::Overview => "bank",
            Self::Chunks => "chunks",
            Self::Hierarchy => "hierarchy",
            Self::Media => "audio",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BankSelection {
    #[default]
    Overview,
    Chunk(usize),
    Hierarchy {
        chunk_index: usize,
        object_index: usize,
    },
    Media {
        index_chunk: usize,
        entry_index: usize,
    },
}

#[derive(Clone, Debug)]
struct ChunkSummary {
    index: usize,
    title: String,
    detail: String,
    search: String,
}

#[derive(Clone, Debug)]
struct HierarchySummary {
    chunk_index: usize,
    object_index: usize,
    type_code: u8,
    kind: HierarchyKind,
    title: String,
    detail: String,
    search: String,
}

#[derive(Clone, Debug)]
struct MediaSummary {
    index_chunk: usize,
    entry_index: usize,
    id: u32,
    offset: u32,
    size: u32,
    data_chunk: Option<usize>,
    search: String,
}

#[derive(Clone, Debug)]
struct BankDocument {
    name: String,
    source_bytes: Arc<[u8]>,
    bank: Arc<SoundBank>,
    parse_mode: ParseMode,
    strict_error: Option<String>,
    chunks: Vec<ChunkSummary>,
    hierarchy: Vec<HierarchySummary>,
    media: Vec<MediaSummary>,
}

impl PartialEq for BankDocument {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && Arc::ptr_eq(&self.source_bytes, &other.source_bytes)
            && Arc::ptr_eq(&self.bank, &other.bank)
            && self.parse_mode == other.parse_mode
            && self.strict_error == other.strict_error
    }
}

impl BankDocument {
    fn new(
        name: String,
        source_bytes: Arc<[u8]>,
        bank: SoundBank,
        parse_mode: ParseMode,
        strict_error: Option<String>,
    ) -> Self {
        let chunks = summarize_chunks(&bank);
        let hierarchy = summarize_hierarchy(&bank);
        let media = summarize_media(&bank);
        Self {
            name,
            source_bytes,
            bank: Arc::new(bank),
            parse_mode,
            strict_error,
            chunks,
            hierarchy,
            media,
        }
    }

    fn with_bank(&self, bank: SoundBank) -> Self {
        Self::new(
            self.name.clone(),
            self.source_bytes.clone(),
            bank,
            self.parse_mode,
            self.strict_error.clone(),
        )
    }

    fn with_saved_bytes(&self, bytes: Arc<[u8]>) -> Self {
        Self::new(
            self.name.clone(),
            bytes,
            (*self.bank).clone(),
            self.parse_mode,
            self.strict_error.clone(),
        )
    }
}

#[derive(Clone, Debug)]
struct BankTab {
    id: u64,
    document: Arc<BankDocument>,
    section: BrowserSection,
    selection: BankSelection,
    query: String,
    page: usize,
    dirty: bool,
}

impl PartialEq for BankTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && Arc::ptr_eq(&self.document, &other.document)
            && self.section == other.section
            && self.selection == other.selection
            && self.query == other.query
            && self.page == other.page
            && self.dirty == other.dirty
    }
}

impl BankTab {
    fn opened(id: u64, document: BankDocument) -> Self {
        Self {
            id,
            document: Arc::new(document),
            section: BrowserSection::Overview,
            selection: BankSelection::Overview,
            query: String::new(),
            page: 0,
            dirty: false,
        }
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
            Self::Success => "ok",
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

#[derive(Clone, Debug)]
struct RowModel {
    key: String,
    selection: BankSelection,
    glyph: &'static str,
    title: String,
    detail: String,
    meta: String,
}

#[component]
pub fn BnkArchivePage(on_open_wem: Option<EventHandler<BnkWemOpenRequest>>) -> Element {
    let tabs = use_signal(Vec::<BankTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);

    let tabs_snapshot = tabs();
    let active_id_snapshot = active_tab_id();
    let active_tab_snapshot = active_id_snapshot
        .and_then(|active_id| tabs_snapshot.iter().find(|tab| tab.id == active_id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: BNK_PAGE_CSS }
        div {
            class: if dragging() { "bnk-page-host is-dragging" } else { "bnk-page-host" },
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
            ondrop: move |event| {
                event.prevent_default();
                dragging.set(false);
                let files = event.files();
                if !files.is_empty() {
                    load_bank_files(
                        files,
                        tabs,
                        active_tab_id,
                        next_tab_id,
                        busy,
                        status,
                    );
                }
            },
            ToolPage { namespace: "bnk", class: "bnk-page",
                ToolPageToolbar {
                    class: "bnk-page-toolbar",
                    actions: rsx! {
                        div { class: "bnk-toolbar-actions",
                            label {
                                class: "bnk-icon-button primary",
                                title: "打开 BNK",
                                aria_label: "打开 BNK",
                                Glyph { name: "open" }
                                input {
                                    class: "bnk-file-input",
                                    r#type: "file",
                                    accept: BNK_FILE_ACCEPT,
                                    multiple: true,
                                    disabled: busy(),
                                    onchange: move |event| {
                                        let files = event.files();
                                        if !files.is_empty() {
                                            load_bank_files(
                                                files,
                                                tabs,
                                                active_tab_id,
                                                next_tab_id,
                                                busy,
                                                status,
                                            );
                                        }
                                    }
                                }
                            }
                            button {
                                r#type: "button",
                                class: "bnk-icon-button",
                                title: "校验当前 BNK",
                                aria_label: "校验当前 BNK",
                                disabled: active_tab_snapshot.is_none() || busy(),
                                onclick: move |_| validate_active_bank(tabs, active_tab_id, status),
                                Glyph { name: "validate" }
                            }
                            button {
                                r#type: "button",
                                class: if active_tab_snapshot.as_ref().is_some_and(|tab| tab.dirty) {
                                    "bnk-icon-button has-changes"
                                } else {
                                    "bnk-icon-button"
                                },
                                title: "重建并另存当前 BNK",
                                aria_label: "重建并另存当前 BNK",
                                disabled: active_tab_snapshot.is_none() || busy(),
                                onclick: move |_| export_active_bank(
                                    tabs,
                                    active_tab_id,
                                    busy,
                                    status,
                                ),
                                Glyph { name: "save" }
                                if active_tab_snapshot.as_ref().is_some_and(|tab| tab.dirty) {
                                    span { class: "bnk-unsaved-dot", aria_hidden: "true" }
                                }
                            }
                            span { class: "bnk-toolbar-spacer" }
                            span { class: "bnk-experimental-pill",
                                span { aria_hidden: "true", "!" }
                                "Experimental"
                            }
                        }
                    }
                }

                BankTabStrip {
                    tabs: tabs_snapshot,
                    active_tab_id: active_id_snapshot,
                    busy: busy(),
                    on_activate: move |tab_id| {
                        active_tab_id.set(Some(tab_id));
                        status.set(AppStatus::default());
                    },
                    on_close: move |tab_id| close_tab(tabs, active_tab_id, tab_id),
                    on_files: move |files| load_bank_files(
                        files,
                        tabs,
                        active_tab_id,
                        next_tab_id,
                        busy,
                        status,
                    ),
                }

                WorkspaceCard { class: "bnk-workspace-card", aria_label: "BNK Archive（实验性）",
                    if let Some(tab) = active_tab_snapshot {
                        BankWorkspace {
                            tab,
                            tabs,
                            busy,
                            status,
                            on_open_wem,
                        }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| load_bank_files(
                                files,
                                tabs,
                                active_tab_id,
                                next_tab_id,
                                busy,
                                status,
                            ),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "bnk-status-notice",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "拖放一个或多个 BNK 以打开".to_string() }
                    }
                    if busy() {
                        div { class: "bnk-busy-indicator", role: "status",
                            span {}
                            "正在处理 SoundBank…"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BankTabStrip(
    tabs: Vec<BankTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "bnk-tab-strip ui-document-tab-strip",
            div {
                class: "bnk-tab-list ui-document-tab-list",
                role: "tablist",
                aria_label: "打开的 BNK",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: bank_tab_class(Some(tab.id) == active_tab_id, tab.dirty),
                        button {
                            r#type: "button",
                            class: "bnk-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            tabindex: if Some(tab.id) == active_tab_id { "0" } else { "-1" },
                            title: "{tab.document.name}",
                            disabled: busy,
                            onclick: {
                                let tab_id = tab.id;
                                move |_| on_activate.call(tab_id)
                            },
                            span {
                                class: "bnk-tab-dot ui-document-tab-dot",
                                title: if tab.dirty { "有尚未保存的更改" } else { "" }
                            }
                            span { class: "bnk-tab-name ui-document-tab-name", "{tab.document.name}" }
                            if tab.document.parse_mode == ParseMode::LosslessFallback {
                                span { class: "bnk-tab-warning", title: "以保留模式打开", "!" }
                            }
                        }
                        button {
                            r#type: "button",
                            class: "bnk-tab-close ui-document-tab-close",
                            title: "关闭 {tab.document.name}",
                            aria_label: "关闭 {tab.document.name}",
                            disabled: busy,
                            onclick: {
                                let tab_id = tab.id;
                                move |_| on_close.call(tab_id)
                            },
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: if busy {
                        "bnk-tab-add ui-document-new-tab is-disabled"
                    } else {
                        "bnk-tab-add ui-document-new-tab"
                    },
                    title: "添加 BNK",
                    aria_label: "添加 BNK",
                    Glyph { name: "add" }
                    input {
                        class: "bnk-file-input",
                        r#type: "file",
                        accept: BNK_FILE_ACCEPT,
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() {
                                on_files.call(files);
                            }
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
        div { class: "bnk-empty-workspace",
            div { class: "bnk-empty-mark", Glyph { name: "bank" } }
            span { class: "bnk-empty-kicker", "EXPERIMENTAL SOUNDBANK WORKSPACE" }
            h1 { "打开 Wwise BNK" }
            p { "浏览并编辑 BKHD、版本化 HIRC 结构和内嵌 WEM，再重建为新的 SoundBank。" }
            div { class: "bnk-empty-warning",
                Glyph { name: "warning" }
                span {
                    strong { "请保留原文件备份" }
                    small { "部分插件私有数据只能保留，尚不能进行语义编辑。" }
                }
            }
            label { class: "bnk-open-button",
                Glyph { name: "open" }
                "选择 BNK"
                input {
                    class: "bnk-file-input",
                    r#type: "file",
                    accept: BNK_FILE_ACCEPT,
                    multiple: true,
                    disabled: busy,
                    onchange: move |event| {
                        let files = event.files();
                        if !files.is_empty() {
                            on_files.call(files);
                        }
                    }
                }
            }
            span { class: "bnk-empty-drop-hint", "也可以把一个或多个 .bnk 文件拖到这里" }
        }
    }
}

#[component]
fn BankWorkspace(
    tab: BankTab,
    tabs: Signal<Vec<BankTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
    on_open_wem: Option<EventHandler<BnkWemOpenRequest>>,
) -> Element {
    let document = tab.document.clone();
    let version = document.bank.version().number();
    let chunk_count = document.chunks.len();
    let hierarchy_count = document.hierarchy.len();
    let media_count = document.media.len();

    rsx! {
        div { class: "bnk-document",
            section { class: "bnk-experimental-notice",
                span { class: "bnk-experimental-notice-icon", Glyph { name: "warning" } }
                div {
                    strong { "BNK Archive 目前是实验性功能" }
                    p { "编辑后会完整重建 BNK，而不是原位修改。未知块会保留；保存前仍请备份并校验游戏内行为。" }
                }
                span { class: "bnk-parse-mode bnk-parse-mode--{parse_mode_class(document.parse_mode)}",
                    "{document.parse_mode.label()}"
                }
            }

            div { class: "bnk-summary-grid",
                SummaryCard { label: "Wwise 版本", value: version.to_string(), glyph: "version" }
                SummaryCard { label: "数据块", value: chunk_count.to_string(), glyph: "chunks" }
                SummaryCard { label: "HIRC 对象", value: hierarchy_count.to_string(), glyph: "hierarchy" }
                SummaryCard { label: "内嵌 WEM", value: media_count.to_string(), glyph: "audio" }
            }

            div { class: "bnk-browser-grid",
                BrowserNavigation {
                    tab: tab.clone(),
                    tabs,
                }
                BrowserList {
                    tab: tab.clone(),
                    tabs,
                }
                Inspector {
                    tab,
                    tabs,
                    busy,
                    status,
                    on_open_wem,
                }
            }
        }
    }
}

#[component]
fn SummaryCard(label: &'static str, value: String, glyph: &'static str) -> Element {
    rsx! {
        article { class: "bnk-summary-card",
            span { class: "bnk-summary-icon", Glyph { name: glyph } }
            div {
                small { "{label}" }
                strong { "{value}" }
            }
        }
    }
}

#[component]
fn BrowserNavigation(tab: BankTab, tabs: Signal<Vec<BankTab>>) -> Element {
    let sections = [
        (BrowserSection::Overview, 1_usize),
        (BrowserSection::Chunks, tab.document.chunks.len()),
        (BrowserSection::Hierarchy, tab.document.hierarchy.len()),
        (BrowserSection::Media, tab.document.media.len()),
    ];
    rsx! {
        aside { class: "bnk-panel bnk-navigation-panel",
            PanelHeading { eyebrow: "BANK", title: "结构".to_string() }
            nav { class: "bnk-section-list", aria_label: "BNK 结构",
                for (section, count) in sections {
                    button {
                        r#type: "button",
                        class: if tab.section == section { "is-active" } else { "" },
                        aria_current: if tab.section == section { "page" } else { "false" },
                        onclick: {
                            let tab_id = tab.id;
                            move |_| set_section(tabs, tab_id, section)
                        },
                        span { class: "bnk-section-icon", Glyph { name: section.glyph() } }
                        span { class: "bnk-section-copy",
                            strong { "{section.label()}" }
                            small {
                                if section == BrowserSection::Overview {
                                    "SoundBank 信息"
                                } else {
                                    "{count} 项"
                                }
                            }
                        }
                        span { class: "bnk-section-chevron", Glyph { name: "chevron" } }
                    }
                }
            }
            div { class: "bnk-bank-identity",
                span { "BKHD" }
                div {
                    strong { title: "{tab.document.name}", "{tab.document.name}" }
                    small { "ID {tab.document.bank.header.id} · {format_bytes(tab.document.source_bytes.len())}" }
                }
            }
        }
    }
}

#[component]
fn BrowserList(tab: BankTab, tabs: Signal<Vec<BankTab>>) -> Element {
    let (rows, total) = visible_rows(&tab);
    let page_count = total.div_ceil(ROW_PAGE_SIZE).max(1);
    let page = tab.page.min(page_count.saturating_sub(1));
    let page_label = format!("第 {} / {} 页 · {} 项", page + 1, page_count, total);
    let has_search = tab.section != BrowserSection::Overview;

    rsx! {
        section { class: "bnk-panel bnk-list-panel",
            div { class: "bnk-list-heading",
                div {
                    span { class: "bnk-panel-eyebrow", "BROWSER" }
                    h2 { "{tab.section.label()}" }
                }
                if has_search {
                    label { class: "bnk-search-field",
                        Glyph { name: "search" }
                        input {
                            r#type: "search",
                            value: "{tab.query}",
                            placeholder: "搜索 ID、类型或块…",
                            oninput: {
                                let tab_id = tab.id;
                                move |event| set_query(tabs, tab_id, event.value())
                            }
                        }
                    }
                }
            }
            if tab.section == BrowserSection::Overview {
                BankOverview { document: tab.document.clone() }
            } else if rows.is_empty() {
                div { class: "bnk-list-empty",
                    Glyph { name: "search" }
                    strong { "没有匹配项" }
                    span { "尝试清除搜索文本或切换分类。" }
                }
            } else {
                div { class: "bnk-row-list",
                    for row in rows {
                        button {
                            key: "{row.key}",
                            r#type: "button",
                            class: if tab.selection == row.selection { "bnk-row is-active" } else { "bnk-row" },
                            onclick: {
                                let tab_id = tab.id;
                                let selection = row.selection;
                                move |_| set_selection(tabs, tab_id, selection)
                            },
                            span { class: "bnk-row-icon", Glyph { name: row.glyph } }
                            span { class: "bnk-row-copy",
                                strong { title: "{row.title}", "{row.title}" }
                                small { title: "{row.detail}", "{row.detail}" }
                            }
                            span { class: "bnk-row-meta", "{row.meta}" }
                        }
                    }
                }
                if page_count > 1 {
                    footer { class: "bnk-pagination",
                        button {
                            r#type: "button",
                            disabled: page == 0,
                            aria_label: "上一页",
                            onclick: {
                                let tab_id = tab.id;
                                move |_| set_page(tabs, tab_id, page.saturating_sub(1))
                            },
                            Glyph { name: "left" }
                        }
                        span { "{page_label}" }
                        button {
                            r#type: "button",
                            disabled: page + 1 >= page_count,
                            aria_label: "下一页",
                            onclick: {
                                let tab_id = tab.id;
                                move |_| set_page(tabs, tab_id, page + 1)
                            },
                            Glyph { name: "right" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BankOverview(document: Arc<BankDocument>) -> Element {
    let mut kinds = BTreeMap::<String, usize>::new();
    for object in &document.hierarchy {
        *kinds
            .entry(hierarchy_kind_label(object.kind).to_string())
            .or_default() += 1;
    }
    let prominent_kinds = kinds.into_iter().take(12).collect::<Vec<_>>();
    rsx! {
        div { class: "bnk-overview-content",
            section { class: "bnk-overview-block",
                span { class: "bnk-overview-label", "SoundBank header" }
                div { class: "bnk-overview-properties",
                    PropertyRow { label: "文件", value: document.name.clone() }
                    PropertyRow { label: "Bank ID", value: document.bank.header.id.to_string() }
                    PropertyRow { label: "语言", value: document.bank.header.language.to_string() }
                    PropertyRow { label: "Wwise 版本", value: document.bank.version().number().to_string() }
                    PropertyRow { label: "源文件大小", value: format_bytes(document.source_bytes.len()) }
                    PropertyRow { label: "解析策略", value: document.parse_mode.label().to_string() }
                }
            }
            section { class: "bnk-overview-block",
                span { class: "bnk-overview-label", "HIRC composition" }
                if prominent_kinds.is_empty() {
                    p { class: "bnk-overview-empty", "该 SoundBank 不包含 HIRC 对象。" }
                } else {
                    div { class: "bnk-kind-cloud",
                        for (kind, count) in prominent_kinds {
                            span { strong { "{count}" } "{kind}" }
                        }
                    }
                }
            }
            if let Some(error) = &document.strict_error {
                section { class: "bnk-fallback-detail",
                    strong { "严格解析没有通过" }
                    p { "data-text-selectable": "true", "{error}" }
                    small { "当前内容以保留模式载入；未知或异常块会按原始字节保留。" }
                }
            }
        }
    }
}

#[component]
fn Inspector(
    tab: BankTab,
    tabs: Signal<Vec<BankTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
    on_open_wem: Option<EventHandler<BnkWemOpenRequest>>,
) -> Element {
    let document = tab.document.clone();
    rsx! {
        aside { class: "bnk-panel bnk-inspector-panel",
            PanelHeading { eyebrow: "INSPECTOR", title: inspector_title(&tab).to_string() }
            div { class: "bnk-inspector-scroll",
                match tab.selection {
                    BankSelection::Overview => rsx! {
                        InspectorSection { label: "SoundBank",
                            PropertyRow { label: "名称", value: document.name.clone() }
                            PropertyRow { label: "版本", value: document.bank.version().number().to_string() }
                            PropertyRow { label: "Header 扩展", value: format_bytes(document.bank.header.header_expand.len()) }
                            PropertyRow { label: "解析模式", value: document.parse_mode.label().to_string() }
                        }
                        HeaderEditor {
                            tab_id: tab.id,
                            document: document.clone(),
                            tabs,
                            busy,
                            status,
                        }
                        InspectorNotice {
                            "BKHD 更改会进入当前标签的待保存状态；版本和 Header 扩展暂不允许从 GUI 修改。"
                        }
                    },
                    BankSelection::Chunk(index) => {
                        if let Some(summary) = document.chunks.get(index) {
                            rsx! {
                                InspectorSection { label: "Chunk",
                                    PropertyRow { label: "标识", value: summary.title.clone() }
                                    PropertyRow { label: "顺序", value: format!("#{}", index + 1) }
                                    PropertyRow { label: "内容", value: summary.detail.clone() }
                                }
                                ChunkInspectorBody { bank: document.bank.clone(), index }
                            }
                        } else {
                            rsx! { MissingSelection {} }
                        }
                    },
                    BankSelection::Hierarchy { chunk_index, object_index } => {
                        if let Some(object) = hierarchy_object(&document.bank, chunk_index, object_index) {
                            let json = hierarchy_json(object);
                            rsx! {
                                InspectorSection { label: "HIRC object",
                                    PropertyRow { label: "对象 ID", value: object.id.to_string() }
                                    PropertyRow { label: "类型", value: hierarchy_kind_label(object.kind).to_string() }
                                    PropertyRow { label: "类型码", value: format!("0x{:02X}", object.type_code) }
                                    PropertyRow {
                                        label: "摘要",
                                        value: hierarchy_object_detail(object, document.bank.version())
                                    }
                                }
                                HierarchyEditor {
                                    tab_id: tab.id,
                                    chunk_index,
                                    object_index,
                                    initial_json: json,
                                    tabs,
                                    busy,
                                    status,
                                }
                            }
                        } else {
                            rsx! { MissingSelection {} }
                        }
                    },
                    BankSelection::Media { index_chunk, entry_index } => {
                        if let Some(media) = document.media.iter().find(|media| {
                            media.index_chunk == index_chunk && media.entry_index == entry_index
                        }).cloned() {
                            let can_open = media.data_chunk.is_some() && media.size > 0;
                            rsx! {
                                InspectorSection { label: "Embedded WEM",
                                    PropertyRow { label: "媒体 ID", value: media.id.to_string() }
                                    PropertyRow { label: "DATA 偏移", value: media.offset.to_string() }
                                    PropertyRow { label: "大小", value: format_bytes(media.size as usize) }
                                    PropertyRow {
                                        label: "数据状态",
                                        value: if media.data_chunk.is_some() {
                                            "可读取".to_string()
                                        } else {
                                            "缺少相邻 DATA".to_string()
                                        }
                                    }
                                }
                                div { class: "bnk-media-actions",
                                    button {
                                        r#type: "button",
                                        class: "primary",
                                        disabled: !can_open || on_open_wem.is_none(),
                                        onclick: {
                                            let document = document.clone();
                                            let media = media.clone();
                                            let on_open_wem = on_open_wem;
                                            move |_| open_media_in_wem(
                                                document.clone(),
                                                media.clone(),
                                                on_open_wem,
                                                status,
                                            )
                                        },
                                        Glyph { name: "play" }
                                        "在 WEM Audio 中打开"
                                    }
                                    button {
                                        r#type: "button",
                                        disabled: !can_open,
                                        onclick: {
                                            let document = document.clone();
                                            let media = media.clone();
                                            move |_| export_media(document.clone(), media.clone(), status)
                                        },
                                        Glyph { name: "save" }
                                        "提取 WEM"
                                    }
                                    label {
                                        class: if can_open && !busy() {
                                            "bnk-replace-media"
                                        } else {
                                            "bnk-replace-media is-disabled"
                                        },
                                        title: "选择一个现成 WEM 替换该媒体",
                                        Glyph { name: "replace" }
                                        "替换 WEM"
                                        input {
                                            class: "bnk-file-input",
                                            r#type: "file",
                                            accept: ".wem,application/octet-stream",
                                            disabled: !can_open || busy(),
                                            onchange: {
                                                let media = media.clone();
                                                let tab_id = tab.id;
                                                move |event| {
                                                    if let Some(file) = event.files().into_iter().next() {
                                                        replace_media_file(
                                                            file,
                                                            tabs,
                                                            tab_id,
                                                            media.clone(),
                                                            busy,
                                                            status,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                InspectorNotice {
                                    "替换时仅重建该 DIDX/DATA 对，并采用 16 字节 WEM 对齐；其他块和媒体顺序保持不变。"
                                }
                            }
                        } else {
                            rsx! { MissingSelection {} }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn HeaderEditor(
    tab_id: u64,
    document: Arc<BankDocument>,
    tabs: Signal<Vec<BankTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
) -> Element {
    let mut bank_id = use_signal(|| document.bank.header.id.to_string());
    let mut language = use_signal(|| document.bank.header.language.to_string());
    let header = (document.bank.header.id, document.bank.header.language);
    use_effect(use_reactive(&header, move |(id, language_id)| {
        bank_id.set(id.to_string());
        language.set(language_id.to_string());
    }));
    rsx! {
        form {
            class: "bnk-editor-section",
            onsubmit: move |event| {
                event.prevent_default();
                apply_header_edit(tab_id, bank_id(), language(), tabs, status);
            },
            span { class: "bnk-inspector-label", "Editable BKHD" }
            label { class: "bnk-editor-field",
                span { "Bank ID" }
                input {
                    r#type: "number",
                    min: "0",
                    max: u32::MAX.to_string(),
                    value: bank_id(),
                    disabled: busy(),
                    oninput: move |event| bank_id.set(event.value()),
                }
            }
            label { class: "bnk-editor-field",
                span { "语言 / Language ID" }
                input {
                    r#type: "number",
                    min: "0",
                    max: u32::MAX.to_string(),
                    value: language(),
                    disabled: busy(),
                    oninput: move |event| language.set(event.value()),
                }
            }
            button {
                r#type: "submit",
                class: "bnk-editor-apply",
                disabled: busy(),
                Glyph { name: "apply" }
                "应用 BKHD 更改"
            }
        }
    }
}

#[component]
fn HierarchyEditor(
    tab_id: u64,
    chunk_index: usize,
    object_index: usize,
    initial_json: String,
    tabs: Signal<Vec<BankTab>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
) -> Element {
    let mut draft = use_signal(|| initial_json.clone());
    let current_json = initial_json.clone();
    use_effect(use_reactive(&current_json, move |json| draft.set(json)));
    rsx! {
        section { class: "bnk-json-section bnk-hirc-editor",
            span { "Version-aware structure" }
            textarea {
                "data-text-selectable": "true",
                aria_label: "HIRC JSON 编辑器",
                value: draft(),
                disabled: busy(),
                oninput: move |event| draft.set(event.value()),
            }
            div { class: "bnk-editor-actions",
                button {
                    r#type: "button",
                    disabled: busy(),
                    onclick: move |_| draft.set(initial_json.clone()),
                    "重置"
                }
                button {
                    r#type: "button",
                    class: "primary",
                    disabled: busy(),
                    onclick: move |_| apply_hierarchy_json(
                        tab_id,
                        chunk_index,
                        object_index,
                        draft(),
                        tabs,
                        status,
                    ),
                    Glyph { name: "apply" }
                    "应用 JSON"
                }
            }
            small { "对象会先反序列化并通过 bnk-archive 结构校验，成功后才进入待保存状态。" }
        }
    }
}

#[component]
fn ChunkInspectorBody(bank: Arc<SoundBank>, index: usize) -> Element {
    let Some(chunk) = bank.chunks.get(index) else {
        return rsx! { MissingSelection {} };
    };
    match chunk {
        BankChunk::MediaIndex(entries) => rsx! {
            InspectorSection { label: "Media index",
                PropertyRow { label: "条目", value: entries.len().to_string() }
                PropertyRow {
                    label: "载荷总量",
                    value: format_bytes(entries.iter().map(|entry| entry.size as usize).sum())
                }
            }
        },
        BankChunk::MediaData(data) => rsx! {
            InspectorSection { label: "Media data",
                PropertyRow { label: "原始数据", value: format_bytes(data.len()) }
            }
        },
        BankChunk::Plugins(plugins) => rsx! {
            InspectorSection { label: "Plugins",
                PropertyRow { label: "插件", value: plugins.len().to_string() }
                for plugin in plugins.iter().take(8) {
                    PropertyRow { label: "ID", value: format!("{} · {}", plugin.id, plugin.library) }
                }
            }
        },
        BankChunk::GameSynchronization(settings) => rsx! {
            InspectorSection { label: "Game synchronization",
                PropertyRow { label: "State groups", value: settings.state_groups.len().to_string() }
                PropertyRow { label: "Switch groups", value: settings.switch_groups.len().to_string() }
                PropertyRow { label: "Game parameters", value: settings.game_parameters.len().to_string() }
                PropertyRow { label: "最大 voices", value: settings.maximum_voice_instances.to_string() }
            }
        },
        BankChunk::Hierarchy(objects) => rsx! {
            InspectorSection { label: "Hierarchy",
                PropertyRow { label: "对象", value: objects.len().to_string() }
                PropertyRow {
                    label: "未知类型",
                    value: objects.iter().filter(|object| object.kind == HierarchyKind::Unknown).count().to_string()
                }
            }
        },
        BankChunk::References(references) => rsx! {
            InspectorSection { label: "References",
                PropertyRow { label: "记录", value: references.entries.len().to_string() }
                PropertyRow { label: "标记", value: references.marker.to_string() }
            }
        },
        BankChunk::Environments(_) => rsx! {
            InspectorNotice { "ENVS 包含 obstruction 与 occlusion 曲线设置。" }
        },
        BankChunk::Platform(platform) => rsx! {
            InspectorSection { label: "Platform",
                PropertyRow { label: "平台", value: platform.name.clone() }
            }
        },
        BankChunk::Unknown(raw) => rsx! {
            InspectorSection { label: "Raw chunk",
                PropertyRow { label: "原始字节", value: format_bytes(raw.data.len()) }
            }
            InspectorNotice { "未知块会原样保留，实验性界面不会修改其内容。" }
        },
    }
}

#[component]
fn InspectorSection(label: &'static str, children: Element) -> Element {
    rsx! {
        section { class: "bnk-inspector-section",
            span { class: "bnk-inspector-label", "{label}" }
            div { class: "bnk-property-list", {children} }
        }
    }
}

#[component]
fn InspectorNotice(children: Element) -> Element {
    rsx! {
        p { class: "bnk-inspector-notice", {children} }
    }
}

#[component]
fn MissingSelection() -> Element {
    rsx! {
        div { class: "bnk-inspector-empty",
            Glyph { name: "inspect" }
            strong { "选择内容以检查" }
            span { "当前项目已经不存在或尚未选择。" }
        }
    }
}

#[component]
fn PropertyRow(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "bnk-property-row",
            dt { "{label}" }
            dd { title: "{value}", "{value}" }
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: String) -> Element {
    rsx! {
        header { class: "bnk-panel-heading",
            span { class: "bnk-panel-eyebrow", "{eyebrow}" }
            h2 { title: "{title}", "{title}" }
        }
    }
}

#[component]
fn Glyph(name: &'static str) -> Element {
    let paths: &[&str] = match name {
        "open" => &["M3 7h5l2 2h11v10H3V7", "M3 7V5h6l2 2"],
        "save" => &["M12 3v12", "m7-5 5 5 5-5", "M4 20h16"],
        "validate" => &["m5 12 4 4L19 6"],
        "apply" => &["m5 12 4 4L19 6", "M4 4h16v16H4V4"],
        "replace" => &[
            "M20 7h-7a4 4 0 0 0-4 4v1",
            "m17 4 3 3-3 3",
            "M4 17h7a4 4 0 0 0 4-4v-1",
            "m7 20-3-3 3-3",
        ],
        "warning" => &["M12 3 2 21h20L12 3", "M12 9v5", "M12 17h.01"],
        "bank" => &["M4 7h16v13H4V7", "M8 7V4h8v3", "M8 11h8", "M8 15h5"],
        "chunks" => &[
            "M4 4h7v7H4V4",
            "M13 4h7v7h-7V4",
            "M4 13h7v7H4v-7",
            "M13 13h7v7h-7v-7",
        ],
        "hierarchy" => &[
            "M12 4v5",
            "M6 14v-2h12v2",
            "M4 14h4v6H4v-6",
            "M10 14h4v6h-4v-6",
            "M16 14h4v6h-4v-6",
        ],
        "audio" => &[
            "M9 18V5l10-2v13",
            "M9 9l10-2",
            "M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6",
            "M16 19a3 3 0 1 0 0-6 3 3 0 0 0 0 6",
        ],
        "version" => &["M12 3a9 9 0 1 0 9 9", "M12 7v5l3 2"],
        "search" => &["M21 21l-4.35-4.35", "M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0"],
        "close" => &["m6 6 12 12", "M18 6 6 18"],
        "add" => &["M12 5v14", "M5 12h14"],
        "chevron" | "right" => &["m9 18 6-6-6-6"],
        "left" => &["m15 18-6-6 6-6"],
        "play" => &["m8 5 11 7-11 7V5"],
        "inspect" => &[
            "M12 5c5 0 9 7 9 7s-4 7-9 7-9-7-9-7 4-7 9-7",
            "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6",
        ],
        _ => &[],
    };
    rsx! {
        svg {
            class: "bnk-glyph",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            for path in paths {
                path { d: "{path}" }
            }
        }
    }
}

fn summarize_chunks(bank: &SoundBank) -> Vec<ChunkSummary> {
    bank.chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let title = chunk_id_label(chunk.id());
            let detail = chunk_detail(chunk);
            ChunkSummary {
                index,
                search: format!("{} {}", title.to_lowercase(), detail.to_lowercase()),
                title,
                detail,
            }
        })
        .collect()
}

fn summarize_hierarchy(bank: &SoundBank) -> Vec<HierarchySummary> {
    let mut summaries = Vec::new();
    for (chunk_index, chunk) in bank.chunks.iter().enumerate() {
        let BankChunk::Hierarchy(objects) = chunk else {
            continue;
        };
        for (object_index, object) in objects.iter().enumerate() {
            let kind = hierarchy_kind_label(object.kind);
            let title = format!("{kind} #{}", object.id);
            let detail = hierarchy_object_detail(object, bank.version());
            summaries.push(HierarchySummary {
                chunk_index,
                object_index,
                type_code: object.type_code,
                kind: object.kind,
                search: format!(
                    "{} {} {:02x} {}",
                    kind.to_lowercase(),
                    object.id,
                    object.type_code,
                    detail.to_lowercase()
                ),
                title,
                detail,
            });
        }
    }
    summaries
}

fn summarize_media(bank: &SoundBank) -> Vec<MediaSummary> {
    let mut summaries = Vec::new();
    for (index_chunk, chunk) in bank.chunks.iter().enumerate() {
        let BankChunk::MediaIndex(entries) = chunk else {
            continue;
        };
        let data_chunk = matches!(
            bank.chunks.get(index_chunk + 1),
            Some(BankChunk::MediaData(_))
        )
        .then_some(index_chunk + 1);
        for (entry_index, entry) in entries.iter().enumerate() {
            summaries.push(MediaSummary {
                index_chunk,
                entry_index,
                id: entry.id,
                offset: entry.offset,
                size: entry.size,
                data_chunk,
                search: format!("{} wem {:08x}", entry.id, entry.id),
            });
        }
    }
    summaries
}

fn chunk_detail(chunk: &BankChunk) -> String {
    match chunk {
        BankChunk::MediaIndex(entries) => format!("{} 个媒体索引", entries.len()),
        BankChunk::MediaData(data) => format!("{} 媒体数据", format_bytes(data.len())),
        BankChunk::Plugins(plugins) => format!("{} 个插件引用", plugins.len()),
        BankChunk::GameSynchronization(settings) => format!(
            "{} state · {} switch · {} parameter",
            settings.state_groups.len(),
            settings.switch_groups.len(),
            settings.game_parameters.len()
        ),
        BankChunk::Hierarchy(objects) => format!("{} 个层级对象", objects.len()),
        BankChunk::References(references) => {
            format!("{} 个 SoundBank 引用", references.entries.len())
        }
        BankChunk::Environments(_) => "环境曲线设置".to_string(),
        BankChunk::Platform(platform) => format!("平台 {}", platform.name),
        BankChunk::Unknown(raw) => format!("{} 未知数据", format_bytes(raw.data.len())),
    }
}

fn hierarchy_kind_label(kind: HierarchyKind) -> &'static str {
    match kind {
        HierarchyKind::Unknown => "Unknown",
        HierarchyKind::StatefulPropertySetting => "Stateful Property",
        HierarchyKind::EventAction => "Event Action",
        HierarchyKind::Event => "Event",
        HierarchyKind::DialogueEvent => "Dialogue Event",
        HierarchyKind::Attenuation => "Attenuation",
        HierarchyKind::LowFrequencyOscillatorModulator => "LFO Modulator",
        HierarchyKind::EnvelopeModulator => "Envelope Modulator",
        HierarchyKind::TimeModulator => "Time Modulator",
        HierarchyKind::Effect => "Effect",
        HierarchyKind::Source => "Source",
        HierarchyKind::AudioDevice => "Audio Device",
        HierarchyKind::AudioBus => "Audio Bus",
        HierarchyKind::AuxiliaryAudioBus => "Auxiliary Bus",
        HierarchyKind::Sound => "Sound",
        HierarchyKind::SoundPlaylistContainer => "Playlist Container",
        HierarchyKind::SoundSwitchContainer => "Switch Container",
        HierarchyKind::SoundBlendContainer => "Blend Container",
        HierarchyKind::ActorMixer => "Actor Mixer",
        HierarchyKind::MusicTrack => "Music Track",
        HierarchyKind::MusicSegment => "Music Segment",
        HierarchyKind::MusicPlaylistContainer => "Music Playlist",
        HierarchyKind::MusicSwitchContainer => "Music Switch",
    }
}

fn hierarchy_object_detail(object: &HierarchyObject, version: bnk_archive::BankVersion) -> String {
    match &object.body {
        HierarchyBody::StatefulPropertySetting(value) => format!("{} 个状态值", value.values.len()),
        HierarchyBody::EventAction(value) => format!(
            "{} → target {}",
            value.action_type_name(version).unwrap_or("Action"),
            value.target
        ),
        HierarchyBody::Event(value) => format!("{} 个 action 引用", value.actions.len()),
        HierarchyBody::DialogueEvent(value) => {
            format!("{} 个关联字段", value.association.fields.len())
        }
        HierarchyBody::Sound(value) => format!(
            "{:?} · resource {} · plugin {}",
            value.source.source_type, value.source.resource, value.source.plugin
        ),
        HierarchyBody::Effect(value)
        | HierarchyBody::Source(value)
        | HierarchyBody::AudioDevice(value) => {
            format!(
                "plugin {} · {} 个字段",
                value.plugin,
                value.settings.fields.len()
            )
        }
        HierarchyBody::AudioBus(value) | HierarchyBody::AuxiliaryAudioBus(value) => {
            format!(
                "parent {} · {} 个字段",
                value.parent,
                value.settings.fields.len()
            )
        }
        HierarchyBody::Attenuation(fields)
        | HierarchyBody::LowFrequencyOscillatorModulator(fields)
        | HierarchyBody::EnvelopeModulator(fields)
        | HierarchyBody::TimeModulator(fields)
        | HierarchyBody::SoundPlaylistContainer(fields)
        | HierarchyBody::SoundSwitchContainer(fields)
        | HierarchyBody::SoundBlendContainer(fields)
        | HierarchyBody::ActorMixer(fields)
        | HierarchyBody::MusicTrack(fields)
        | HierarchyBody::MusicSegment(fields)
        | HierarchyBody::MusicPlaylistContainer(fields)
        | HierarchyBody::MusicSwitchContainer(fields) => {
            format!("{} 个结构化字段", fields.fields.len())
        }
        HierarchyBody::Raw(bytes) => format!("{} 原始载荷", format_bytes(bytes.len())),
    }
}

fn visible_rows(tab: &BankTab) -> (Vec<RowModel>, usize) {
    let query = tab.query.trim().to_lowercase();
    let start = tab.page.saturating_mul(ROW_PAGE_SIZE);
    let end = start.saturating_add(ROW_PAGE_SIZE);
    let mut total = 0;
    let mut rows = Vec::new();

    match tab.section {
        BrowserSection::Overview => {}
        BrowserSection::Chunks => {
            for summary in &tab.document.chunks {
                if !query.is_empty() && !summary.search.contains(&query) {
                    continue;
                }
                if (start..end).contains(&total) {
                    rows.push(RowModel {
                        key: format!("chunk-{}", summary.index),
                        selection: BankSelection::Chunk(summary.index),
                        glyph: "chunks",
                        title: summary.title.clone(),
                        detail: summary.detail.clone(),
                        meta: format!("#{}", summary.index + 1),
                    });
                }
                total += 1;
            }
        }
        BrowserSection::Hierarchy => {
            for summary in &tab.document.hierarchy {
                if !query.is_empty() && !summary.search.contains(&query) {
                    continue;
                }
                if (start..end).contains(&total) {
                    rows.push(RowModel {
                        key: format!("hirc-{}-{}", summary.chunk_index, summary.object_index),
                        selection: BankSelection::Hierarchy {
                            chunk_index: summary.chunk_index,
                            object_index: summary.object_index,
                        },
                        glyph: "hierarchy",
                        title: summary.title.clone(),
                        detail: summary.detail.clone(),
                        meta: format!("0x{:02X}", summary.type_code),
                    });
                }
                total += 1;
            }
        }
        BrowserSection::Media => {
            for summary in &tab.document.media {
                if !query.is_empty() && !summary.search.contains(&query) {
                    continue;
                }
                if (start..end).contains(&total) {
                    rows.push(RowModel {
                        key: format!("media-{}-{}", summary.index_chunk, summary.entry_index),
                        selection: BankSelection::Media {
                            index_chunk: summary.index_chunk,
                            entry_index: summary.entry_index,
                        },
                        glyph: "audio",
                        title: format!("{}.wem", summary.id),
                        detail: format!("DATA + {}", summary.offset),
                        meta: format_bytes(summary.size as usize),
                    });
                }
                total += 1;
            }
        }
    }
    (rows, total)
}

fn hierarchy_object(
    bank: &SoundBank,
    chunk_index: usize,
    object_index: usize,
) -> Option<&HierarchyObject> {
    let BankChunk::Hierarchy(objects) = bank.chunks.get(chunk_index)? else {
        return None;
    };
    objects.get(object_index)
}

fn media_bytes<'a>(document: &'a BankDocument, media: &MediaSummary) -> Result<&'a [u8], String> {
    let BankChunk::MediaIndex(entries) = document
        .bank
        .chunks
        .get(media.index_chunk)
        .ok_or_else(|| "DIDX 块已经不存在".to_string())?
    else {
        return Err("所选块不再是 DIDX".to_string());
    };
    let entry = entries
        .get(media.entry_index)
        .ok_or_else(|| "媒体索引已经不存在".to_string())?;
    if entry.id == 0 && entry.offset == 1 && entry.size == 0 {
        return Ok(&[]);
    }
    let data_chunk = media
        .data_chunk
        .ok_or_else(|| "DIDX 后没有相邻 DATA 块".to_string())?;
    let BankChunk::MediaData(data) = document
        .bank
        .chunks
        .get(data_chunk)
        .ok_or_else(|| "DATA 块已经不存在".to_string())?
    else {
        return Err("所选数据块不再是 DATA".to_string());
    };
    let begin = entry.offset as usize;
    let end = begin
        .checked_add(entry.size as usize)
        .ok_or_else(|| "媒体范围溢出".to_string())?;
    data.get(begin..end)
        .ok_or_else(|| "媒体范围超出 DATA 块".to_string())
}

fn commit_bank_edit(
    mut tabs: Signal<Vec<BankTab>>,
    tab_id: u64,
    bank: SoundBank,
) -> Result<(), String> {
    bank.validate().map_err(|error| error.to_string())?;
    let mut tabs = tabs.write();
    let tab = tabs
        .iter_mut()
        .find(|tab| tab.id == tab_id)
        .ok_or_else(|| "当前 BNK 标签已经关闭".to_string())?;
    tab.document = Arc::new(tab.document.with_bank(bank));
    tab.dirty = true;
    Ok(())
}

fn apply_header_edit(
    tab_id: u64,
    bank_id: String,
    language: String,
    tabs: Signal<Vec<BankTab>>,
    mut status: Signal<AppStatus>,
) {
    let bank_id = match bank_id.trim().parse::<u32>() {
        Ok(value) => value,
        Err(error) => {
            status.set(AppStatus::new(
                format!("Bank ID 必须是 0–{} 的整数：{error}", u32::MAX),
                StatusTone::Error,
            ));
            return;
        }
    };
    let language = match language.trim().parse::<u32>() {
        Ok(value) => value,
        Err(error) => {
            status.set(AppStatus::new(
                format!("Language ID 必须是 0–{} 的整数：{error}", u32::MAX),
                StatusTone::Error,
            ));
            return;
        }
    };
    let Some(mut bank) = tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| (*tab.document.bank).clone())
    else {
        return;
    };
    bank.header.id = bank_id;
    bank.header.language = language;
    match commit_bank_edit(tabs, tab_id, bank) {
        Ok(()) => {
            status.set(AppStatus::new(
                "BKHD 更改已应用，当前 BNK 尚未保存。",
                StatusTone::Success,
            ));
            push_application_log(
                "BNK",
                "INFO",
                "EDIT_HEADER",
                format!("Updated BKHD: bank_id={bank_id}, language={language}"),
            );
        }
        Err(error) => status.set(AppStatus::new(
            format!("无法应用 BKHD 更改：{error}"),
            StatusTone::Error,
        )),
    }
}

fn apply_hierarchy_json(
    tab_id: u64,
    chunk_index: usize,
    object_index: usize,
    json: String,
    tabs: Signal<Vec<BankTab>>,
    mut status: Signal<AppStatus>,
) {
    let object = match serde_json::from_str::<HierarchyObject>(&json) {
        Ok(object) => object,
        Err(error) => {
            status.set(AppStatus::new(
                format!("HIRC JSON 无法解析：{error}"),
                StatusTone::Error,
            ));
            return;
        }
    };
    let Some(mut bank) = tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| (*tab.document.bank).clone())
    else {
        return;
    };
    let Some(BankChunk::Hierarchy(objects)) = bank.chunks.get_mut(chunk_index) else {
        status.set(AppStatus::new(
            "所选 HIRC 块已经不存在。",
            StatusTone::Error,
        ));
        return;
    };
    let Some(slot) = objects.get_mut(object_index) else {
        status.set(AppStatus::new(
            "所选 HIRC 对象已经不存在。",
            StatusTone::Error,
        ));
        return;
    };
    let object_id = object.id;
    *slot = object;
    match commit_bank_edit(tabs, tab_id, bank) {
        Ok(()) => {
            status.set(AppStatus::new(
                format!("HIRC 对象 #{object_id} 已更新，当前 BNK 尚未保存。"),
                StatusTone::Success,
            ));
            push_application_log(
                "BNK",
                "INFO",
                "EDIT_HIRC",
                format!("Updated HIRC object {object_id}"),
            );
        }
        Err(error) => status.set(AppStatus::new(
            format!("HIRC 对象未通过写入校验：{error}"),
            StatusTone::Error,
        )),
    }
}

fn replace_media_file(
    file: FileData,
    tabs: Signal<Vec<BankTab>>,
    tab_id: u64,
    media: MediaSummary,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() {
        return;
    }
    let name = file.name();
    busy.set(true);
    status.set(AppStatus::new(
        format!("正在读取替换媒体 {name}…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let result = async {
            let bytes = file
                .read_bytes()
                .await
                .map_err(|error| format!("无法读取 {name}：{error}"))?;
            if !looks_like_wem(&bytes) {
                return Err("所选文件不是 RIFF/RIFX WEM 音频".to_string());
            }
            let Some(mut bank) = tabs
                .peek()
                .iter()
                .find(|tab| tab.id == tab_id)
                .map(|tab| (*tab.document.bank).clone())
            else {
                return Err("当前 BNK 标签已经关闭".to_string());
            };
            let data_chunk = media
                .data_chunk
                .ok_or_else(|| "所选 DIDX 后没有相邻 DATA 块".to_string())?;
            bank.replace_embedded_media(
                EmbeddedMediaLocation {
                    index_chunk: media.index_chunk,
                    data_chunk,
                    entry_index: media.entry_index,
                },
                bytes.to_vec(),
                16,
            )
            .map_err(|error| error.to_string())?;
            commit_bank_edit(tabs, tab_id, bank)?;
            Ok::<usize, String>(bytes.len())
        }
        .await;

        match result {
            Ok(size) => {
                status.set(AppStatus::new(
                    format!("{}.wem 已替换为 {name}，当前 BNK 尚未保存。", media.id),
                    StatusTone::Success,
                ));
                push_application_log(
                    "BNK",
                    "INFO",
                    "REPLACE_WEM",
                    format!(
                        "Replaced embedded media {} with {name} ({size} bytes)",
                        media.id
                    ),
                );
            }
            Err(error) => status.set(AppStatus::new(
                format!("无法替换 WEM：{error}"),
                StatusTone::Error,
            )),
        }
        busy.set(false);
    });
}

fn looks_like_wem(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && matches!(&bytes[..4], b"RIFF" | b"RIFX") && &bytes[8..12] == b"WAVE"
}

fn set_section(mut tabs: Signal<Vec<BankTab>>, tab_id: u64, section: BrowserSection) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.section = section;
        tab.selection = if section == BrowserSection::Overview {
            BankSelection::Overview
        } else {
            match section {
                BrowserSection::Chunks => tab
                    .document
                    .chunks
                    .first()
                    .map(|chunk| BankSelection::Chunk(chunk.index))
                    .unwrap_or_default(),
                BrowserSection::Hierarchy => tab
                    .document
                    .hierarchy
                    .first()
                    .map(|object| BankSelection::Hierarchy {
                        chunk_index: object.chunk_index,
                        object_index: object.object_index,
                    })
                    .unwrap_or_default(),
                BrowserSection::Media => tab
                    .document
                    .media
                    .first()
                    .map(|media| BankSelection::Media {
                        index_chunk: media.index_chunk,
                        entry_index: media.entry_index,
                    })
                    .unwrap_or_default(),
                BrowserSection::Overview => BankSelection::Overview,
            }
        };
        tab.query.clear();
        tab.page = 0;
    }
}

fn set_selection(mut tabs: Signal<Vec<BankTab>>, tab_id: u64, selection: BankSelection) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.selection = selection;
    }
}

fn set_query(mut tabs: Signal<Vec<BankTab>>, tab_id: u64, query: String) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.query = query;
        tab.page = 0;
    }
}

fn set_page(mut tabs: Signal<Vec<BankTab>>, tab_id: u64, page: usize) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.page = page;
    }
}

fn close_tab(mut tabs: Signal<Vec<BankTab>>, mut active_tab_id: Signal<Option<u64>>, tab_id: u64) {
    let mut tabs = tabs.write();
    let Some(index) = tabs.iter().position(|tab| tab.id == tab_id) else {
        return;
    };
    let was_active = active_tab_id() == Some(tab_id);
    tabs.remove(index);
    if was_active {
        active_tab_id.set(
            tabs.get(index)
                .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
                .map(|tab| tab.id),
        );
    }
}

fn load_bank_files(
    files: Vec<FileData>,
    tabs: Signal<Vec<BankTab>>,
    active_tab_id: Signal<Option<u64>>,
    next_tab_id: Signal<u64>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() {
        return;
    }
    busy.set(true);
    status.set(AppStatus::new(
        "正在读取并解析 SoundBank…",
        StatusTone::Neutral,
    ));
    spawn(async move {
        for file in files {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => {
                    open_bank_bytes(
                        name,
                        Arc::from(bytes.to_vec()),
                        tabs,
                        active_tab_id,
                        next_tab_id,
                        status,
                    )
                    .await;
                }
                Err(error) => {
                    status.set(AppStatus::new(
                        format!("无法读取 {name}：{error}"),
                        StatusTone::Error,
                    ));
                }
            }
        }
        busy.set(false);
    });
}

async fn open_bank_bytes(
    name: String,
    bytes: Arc<[u8]>,
    mut tabs: Signal<Vec<BankTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    mut next_tab_id: Signal<u64>,
    mut status: Signal<AppStatus>,
) {
    match platform::decode_bank(bytes.clone()).await {
        Ok(decoded) => {
            let id = next_tab_id();
            next_tab_id.set(id.wrapping_add(1).max(1));
            let parse_mode = if decoded.permissive {
                ParseMode::LosslessFallback
            } else {
                ParseMode::Strict
            };
            let document = BankDocument::new(
                name.clone(),
                bytes.clone(),
                decoded.bank,
                parse_mode,
                decoded.strict_error,
            );
            let chunks = document.chunks.len();
            let objects = document.hierarchy.len();
            let media = document.media.len();
            tabs.write().push(BankTab::opened(id, document));
            active_tab_id.set(Some(id));
            let tone = if parse_mode == ParseMode::Strict {
                StatusTone::Success
            } else {
                StatusTone::Warning
            };
            let mode_note = if parse_mode == ParseMode::Strict {
                ""
            } else {
                "，已回退到保留模式"
            };
            status.set(AppStatus::new(
                format!(
                    "已打开 {name}{mode_note}：{chunks} 个块、{objects} 个 HIRC、{media} 个 WEM。"
                ),
                tone,
            ));
            push_application_log(
                "BNK",
                if parse_mode == ParseMode::Strict {
                    "INFO"
                } else {
                    "WARN"
                },
                "OPEN",
                format!(
                    "Opened {name} ({} bytes, {chunks} chunks, {objects} HIRC, {media} media, {})",
                    bytes.len(),
                    parse_mode.label()
                ),
            );
        }
        Err(error) => {
            push_application_log(
                "BNK",
                "ERROR",
                "OPEN",
                format!("Open {name} failed: {error}"),
            );
            status.set(AppStatus::new(
                format!("无法打开 {name}：{error}"),
                StatusTone::Error,
            ));
        }
    }
}

fn validate_active_bank(
    tabs: Signal<Vec<BankTab>>,
    active_tab_id: Signal<Option<u64>>,
    mut status: Signal<AppStatus>,
) {
    let Some(tab_id) = active_tab_id() else {
        return;
    };
    let Some(document) = tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| tab.document.clone())
    else {
        return;
    };
    match (
        document.bank.validate(),
        document.bank.validate_twinning_compatibility(),
    ) {
        (Ok(()), Ok(())) => {
            status.set(AppStatus::new(
                "BNK 已通过基础校验与严格兼容性校验。",
                StatusTone::Success,
            ));
            push_application_log(
                "BNK",
                "INFO",
                "VALIDATE",
                format!("Validated {}", document.name),
            );
        }
        (Ok(()), Err(error)) => {
            status.set(AppStatus::new(
                format!(
                    "基础校验通过，但未满足严格块顺序：{}",
                    user_facing_validation_error(&error.to_string())
                ),
                StatusTone::Warning,
            ));
        }
        (Err(error), _) => {
            let error = user_facing_validation_error(&error.to_string());
            status.set(AppStatus::new(
                format!("BNK 校验失败：{error}"),
                StatusTone::Error,
            ));
            push_application_log(
                "BNK",
                "ERROR",
                "VALIDATE",
                format!("Validation failed for {}: {error}", document.name),
            );
        }
    }
}

fn user_facing_validation_error(message: &str) -> String {
    message
        .replace("Twinning chunk sequence", "strict chunk sequence")
        .replace("Twinning's model", "the strict format model")
        .replace("Twinning requires", "the format specification requires")
        .replace("Twinning's", "the format specification's")
        .replace("Twinning", "the format specification")
}

fn export_active_bank(
    tabs: Signal<Vec<BankTab>>,
    active_tab_id: Signal<Option<u64>>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    let Some(tab_id) = active_tab_id() else {
        return;
    };
    let Some(document) = tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| tab.document.clone())
    else {
        return;
    };
    if busy() {
        return;
    }
    busy.set(true);
    status.set(AppStatus::new("正在校验并重建 BNK…", StatusTone::Neutral));
    spawn(async move {
        let result = platform::encode_bank(document.bank.clone()).await;
        match result {
            Ok(bytes) => {
                let name = normalized_bnk_name(&document.name);
                match platform::save_bytes(&name, "Wwise SoundBank", &["bnk"], &bytes).await {
                    Ok(true) => {
                        mark_tab_saved(tabs, tab_id, Arc::from(bytes.clone()));
                        status.set(AppStatus::new(
                            format!("{name} 已通过结构校验并重建保存。"),
                            StatusTone::Success,
                        ));
                        push_application_log(
                            "BNK",
                            "INFO",
                            "WRITE_BACK",
                            format!("Rebuilt {name} ({} bytes)", bytes.len()),
                        );
                    }
                    Ok(false) => status.set(AppStatus::default()),
                    Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
                }
            }
            Err(error) => status.set(AppStatus::new(
                format!("无法重建 BNK；当前更改未保存：{error}"),
                StatusTone::Error,
            )),
        }
        busy.set(false);
    });
}

fn mark_tab_saved(mut tabs: Signal<Vec<BankTab>>, tab_id: u64, bytes: Arc<[u8]>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.document = Arc::new(tab.document.with_saved_bytes(bytes));
        tab.dirty = false;
    }
}

fn open_media_in_wem(
    document: Arc<BankDocument>,
    media: MediaSummary,
    on_open_wem: Option<EventHandler<BnkWemOpenRequest>>,
    mut status: Signal<AppStatus>,
) {
    let Some(on_open_wem) = on_open_wem else {
        status.set(AppStatus::new(
            "当前宿主没有提供 WEM Audio 跳转。",
            StatusTone::Warning,
        ));
        return;
    };
    match media_bytes(&document, &media) {
        Ok(bytes) if !bytes.is_empty() => {
            let name = format!("{}.wem", media.id);
            on_open_wem.call(BnkWemOpenRequest {
                name: name.clone(),
                bytes: Arc::from(bytes.to_vec()),
            });
            push_application_log(
                "BNK",
                "INFO",
                "OPEN_WEM",
                format!("Opened embedded media {name} in WEM Audio"),
            );
        }
        Ok(_) => status.set(AppStatus::new(
            "保留媒体条目没有可打开的数据。",
            StatusTone::Warning,
        )),
        Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
    }
}

fn export_media(document: Arc<BankDocument>, media: MediaSummary, mut status: Signal<AppStatus>) {
    let bytes = match media_bytes(&document, &media) {
        Ok(bytes) if !bytes.is_empty() => bytes.to_vec(),
        Ok(_) => {
            status.set(AppStatus::new(
                "保留媒体条目没有可导出的数据。",
                StatusTone::Warning,
            ));
            return;
        }
        Err(error) => {
            status.set(AppStatus::new(error, StatusTone::Error));
            return;
        }
    };
    let name = format!("{}.wem", media.id);
    status.set(AppStatus::new(
        format!("正在提取 {name}…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        match platform::save_bytes(&name, "Wwise encoded media", &["wem"], &bytes).await {
            Ok(true) => {
                status.set(AppStatus::new(
                    format!("{name} 已提取。"),
                    StatusTone::Success,
                ));
                push_application_log(
                    "BNK",
                    "INFO",
                    "EXTRACT",
                    format!("Extracted {name} ({} bytes)", bytes.len()),
                );
            }
            Ok(false) => status.set(AppStatus::default()),
            Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
        }
    });
}

fn hierarchy_json(object: &HierarchyObject) -> String {
    match serde_json::to_string_pretty(object) {
        Ok(text) => text,
        Err(error) => format!("无法生成结构化视图：{error}"),
    }
}

fn inspector_title(tab: &BankTab) -> &'static str {
    match tab.selection {
        BankSelection::Overview => "SoundBank",
        BankSelection::Chunk(_) => "数据块",
        BankSelection::Hierarchy { .. } => "HIRC 对象",
        BankSelection::Media { .. } => "内嵌媒体",
    }
}

fn parse_mode_class(mode: ParseMode) -> &'static str {
    match mode {
        ParseMode::Strict => "strict",
        ParseMode::LosslessFallback => "fallback",
    }
}

fn bank_tab_class(active: bool, dirty: bool) -> &'static str {
    match (active, dirty) {
        (true, true) => "bnk-tab ui-document-tab is-active has-changes",
        (true, false) => "bnk-tab ui-document-tab is-active",
        (false, true) => "bnk-tab ui-document-tab has-changes",
        (false, false) => "bnk-tab ui-document-tab",
    }
}

fn chunk_id_label(id: ChunkId) -> String {
    if let Some(label) = id.as_str() {
        return label.to_string();
    }
    let bytes = id.as_bytes();
    if bytes.iter().all(u8::is_ascii_graphic) {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        bytes
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn normalized_bnk_name(name: &str) -> String {
    let stem = name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name);
    format!("{stem}.bnk")
}

fn format_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bnk_archive::{
        BankHeader, BankVersion, HierarchyBody, HierarchyObject, MediaIndexEntry, RawChunk,
    };

    fn sample_bank() -> SoundBank {
        SoundBank {
            header: BankHeader {
                version: BankVersion::V140,
                id: 7,
                language: 0,
                header_expand: Vec::new(),
            },
            chunks: vec![
                BankChunk::Hierarchy(vec![HierarchyObject {
                    kind: HierarchyKind::Unknown,
                    type_code: 250,
                    id: 42,
                    body: HierarchyBody::Raw(vec![1, 2, 3]),
                }]),
                BankChunk::MediaIndex(vec![MediaIndexEntry {
                    id: 99,
                    offset: 0,
                    size: 3,
                }]),
                BankChunk::MediaData(vec![4, 5, 6]),
                BankChunk::Unknown(RawChunk {
                    id: ChunkId(*b"TEST"),
                    data: vec![8, 9],
                }),
            ],
        }
    }

    #[test]
    fn document_builds_bounded_browser_indexes() {
        let document = BankDocument::new(
            "sample.bnk".to_string(),
            Arc::from(Vec::<u8>::new()),
            sample_bank(),
            ParseMode::Strict,
            None,
        );
        assert_eq!(document.chunks.len(), 4);
        assert_eq!(document.hierarchy.len(), 1);
        assert_eq!(document.media.len(), 1);
        assert_eq!(
            media_bytes(&document, &document.media[0]).unwrap(),
            [4, 5, 6]
        );
    }

    #[test]
    fn browser_search_matches_identifiers_and_chunk_names() {
        let document = BankDocument::new(
            "sample.bnk".to_string(),
            Arc::from(Vec::<u8>::new()),
            sample_bank(),
            ParseMode::Strict,
            None,
        );
        let mut tab = BankTab::opened(1, document);
        tab.section = BrowserSection::Hierarchy;
        tab.query = "42".to_string();
        assert_eq!(visible_rows(&tab).1, 1);
        tab.query = "missing".to_string();
        assert_eq!(visible_rows(&tab).1, 0);
    }

    #[test]
    fn export_names_are_normalized_to_bnk() {
        assert_eq!(normalized_bnk_name("Init.BNK"), "Init.bnk");
        assert_eq!(normalized_bnk_name("soundbank"), "soundbank.bnk");
    }

    #[test]
    fn replacement_media_requires_a_wem_riff_container() {
        assert!(looks_like_wem(b"RIFF\x04\0\0\0WAVE"));
        assert!(looks_like_wem(b"RIFX\0\0\0\x04WAVE"));
        assert!(!looks_like_wem(b"OggS\0\0\0\0WAVE"));
        assert!(!looks_like_wem(b"RIFF"));
    }
}
