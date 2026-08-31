use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use dzip::{ArchivePathKey, Compression, RangeSettings};
use toolkit_ui::{
    DropIndicator, InlineNotice, ToolPage, ToolPageToolbar, WorkspaceCard, push_application_log,
};

use crate::model::{ArchiveSummary, BuildRequest, DraftEntry, MaterializeRequest, NamedBytes};
use crate::{platform, service::normalize_archive_name};

const DZIP_PAGE_CSS: Asset = asset!("/assets/dzip/page.css");
const DZIP_FILE_ACCEPT: &str = ".dz,.dzip,.001,.002,.003,application/octet-stream";
const ROW_PAGE_SIZE: usize = 300;

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

#[derive(Clone, Debug, PartialEq, Eq)]
enum BrowserSelection {
    Directory(String),
    Entry(u64),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BrowserSortKey {
    #[default]
    Name,
    Type,
    Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BrowserSortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BrowserSort {
    key: BrowserSortKey,
    direction: BrowserSortDirection,
}

impl BrowserSort {
    fn toggled(self, key: BrowserSortKey) -> Self {
        if self.key == key {
            Self {
                key,
                direction: match self.direction {
                    BrowserSortDirection::Ascending => BrowserSortDirection::Descending,
                    BrowserSortDirection::Descending => BrowserSortDirection::Ascending,
                },
            }
        } else {
            Self {
                key,
                direction: BrowserSortDirection::Ascending,
            }
        }
    }

    fn aria_value(self, key: BrowserSortKey) -> &'static str {
        if self.key != key {
            return "none";
        }
        match self.direction {
            BrowserSortDirection::Ascending => "ascending",
            BrowserSortDirection::Descending => "descending",
        }
    }

    fn indicator(self, key: BrowserSortKey) -> &'static str {
        if self.key != key {
            return "↕";
        }
        match self.direction {
            BrowserSortDirection::Ascending => "↑",
            BrowserSortDirection::Descending => "↓",
        }
    }

    fn apply(self, ordering: Ordering) -> Ordering {
        match self.direction {
            BrowserSortDirection::Ascending => ordering,
            BrowserSortDirection::Descending => ordering.reverse(),
        }
    }
}

#[derive(Clone, Debug)]
struct ArchiveTab {
    id: u64,
    name: String,
    session_id: Option<u64>,
    entries: std::sync::Arc<Vec<DraftEntry>>,
    directory: String,
    selection: Option<BrowserSelection>,
    query: String,
    sort: BrowserSort,
    row_limit: usize,
    next_entry_id: u64,
    dirty: bool,
    source_size: u64,
    source_complete: bool,
    original_chunk_count: usize,
    loaded_volume_count: usize,
    volume_count: usize,
    alignment: u32,
    default_compression: Compression,
    range_settings: RangeSettings,
    use_common_buffer: bool,
}

impl PartialEq for ArchiveTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.session_id == other.session_id
            && std::sync::Arc::ptr_eq(&self.entries, &other.entries)
            && self.directory == other.directory
            && self.selection == other.selection
            && self.query == other.query
            && self.sort == other.sort
            && self.row_limit == other.row_limit
            && self.dirty == other.dirty
            && self.source_size == other.source_size
            && self.volume_count == other.volume_count
            && self.alignment == other.alignment
            && self.default_compression == other.default_compression
            && self.use_common_buffer == other.use_common_buffer
    }
}

impl Eq for ArchiveTab {}

impl ArchiveTab {
    fn opened(id: u64, summary: ArchiveSummary) -> Self {
        let next_entry_id = summary.entries.len() as u64 + 1;
        Self {
            id,
            name: summary.name,
            session_id: Some(summary.session_id),
            entries: std::sync::Arc::new(
                summary
                    .entries
                    .into_iter()
                    .map(DraftEntry::from_summary)
                    .collect(),
            ),
            directory: String::new(),
            selection: None,
            query: String::new(),
            sort: BrowserSort::default(),
            row_limit: ROW_PAGE_SIZE,
            next_entry_id,
            dirty: false,
            source_size: summary.source_size,
            source_complete: summary.source_complete,
            original_chunk_count: summary.chunk_count,
            loaded_volume_count: summary.loaded_volume_count,
            volume_count: summary.volume_count.max(1),
            alignment: 0,
            default_compression: Compression::Dz,
            range_settings: summary.range_settings,
            use_common_buffer: summary.use_common_buffer,
        }
    }

    fn empty(id: u64) -> Self {
        Self {
            id,
            name: "archive.dz".to_string(),
            session_id: None,
            entries: std::sync::Arc::new(Vec::new()),
            directory: String::new(),
            selection: None,
            query: String::new(),
            sort: BrowserSort::default(),
            row_limit: ROW_PAGE_SIZE,
            next_entry_id: 1,
            dirty: false,
            source_size: 0,
            source_complete: true,
            original_chunk_count: 0,
            loaded_volume_count: 0,
            volume_count: 1,
            alignment: 0,
            default_compression: Compression::Dz,
            range_settings: RangeSettings::default(),
            use_common_buffer: false,
        }
    }

    fn replace_with_summary(&mut self, summary: ArchiveSummary) -> Option<u64> {
        let previous = self.session_id.replace(summary.session_id);
        self.name = summary.name;
        self.entries = std::sync::Arc::new(
            summary
                .entries
                .into_iter()
                .map(DraftEntry::from_summary)
                .collect(),
        );
        self.next_entry_id = self.entries.len() as u64 + 1;
        self.selection = None;
        self.dirty = false;
        self.source_size = summary.source_size;
        self.source_complete = summary.source_complete;
        self.original_chunk_count = summary.chunk_count;
        self.loaded_volume_count = summary.loaded_volume_count;
        self.volume_count = summary.volume_count.max(1);
        self.range_settings = summary.range_settings;
        self.use_common_buffer = summary.use_common_buffer;
        previous
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BrowserRowKind {
    Directory,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrowserRow {
    key: String,
    selection: BrowserSelection,
    kind: BrowserRowKind,
    name: String,
    full_path: String,
    size: u64,
    packed_size: Option<u64>,
    compression: Option<Compression>,
    volume: Option<u16>,
    detail: String,
}

#[derive(Clone, Debug, PartialEq)]
struct ContextMenuState {
    target: Option<BrowserSelection>,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EditDialog {
    Rename {
        target: BrowserSelection,
        value: String,
    },
    Delete {
        target: BrowserSelection,
        label: String,
    },
    ArchiveProperties {
        name: String,
        alignment: String,
        volumes: String,
        default_compression: String,
        common_buffer: bool,
    },
    CloseDirty {
        tab_id: u64,
        name: String,
    },
}

#[derive(Clone, Copy, PartialEq)]
struct WorkspaceSignals {
    tabs: Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
}

#[component]
pub fn DzipArchivePage() -> Element {
    let tabs = use_signal(Vec::<ArchiveTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status =
        use_signal(|| AppStatus::new("打开、拖入或新建一个 DZip 归档", StatusTone::Neutral));
    let mut tree_visible = use_signal(|| true);
    let mut inspector_visible = use_signal(|| true);
    let mut dialog = use_signal(|| None::<EditDialog>);
    let mut context_menu = use_signal(|| None::<ContextMenuState>);
    let signals = WorkspaceSignals {
        tabs,
        active_tab_id,
        busy,
        status,
    };
    let tabs_snapshot = tabs();
    let active_id_snapshot = active_tab_id();
    let active_tab_snapshot = active_id_snapshot
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: DZIP_PAGE_CSS }
        div {
            class: if dragging() { "dzip-page-host is-dragging" } else { "dzip-page-host" },
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
                if files.is_empty() { return; }
                if files.iter().any(|file| is_main_archive_name(&file.name())) {
                    load_archive_files(files, signals, next_tab_id);
                } else {
                    import_files(files, signals);
                }
            },
            onclick: move |_| context_menu.set(None),
            ToolPage { namespace: "dzip", class: "dzip-page",
                ToolPageToolbar { class: "dzip-page-toolbar",
                    actions: rsx! {
                        ArchiveToolbar {
                            tab: active_tab_snapshot.clone(),
                            busy: busy(),
                            tree_visible: tree_visible(),
                            inspector_visible: inspector_visible(),
                            on_files: move |files| load_archive_files(files, signals, next_tab_id),
                            on_new: move |_| create_empty_tab(signals, next_tab_id),
                            on_up: move |_| navigate_up(tabs, active_tab_id),
                            on_extract: move |_| extract_current(signals),
                            on_add_files: move |files| import_files(files, signals),
                            on_save: move |_| save_active_archive(signals),
                            on_properties: move |_| {
                                if let Some(tab) = active_tab(&signals.tabs, signals.active_tab_id) {
                                    dialog.set(Some(EditDialog::ArchiveProperties {
                                        name: tab.name,
                                        alignment: tab.alignment.to_string(),
                                        volumes: tab.volume_count.to_string(),
                                        default_compression: compression_code(tab.default_compression).to_string(),
                                        common_buffer: tab.use_common_buffer,
                                    }));
                                }
                            },
                            on_validate: move |_| validate_active(signals),
                            on_toggle_tree: move |_| tree_visible.set(!tree_visible()),
                            on_toggle_inspector: move |_| inspector_visible.set(!inspector_visible()),
                        }
                    }
                }

                ArchiveTabStrip {
                    tabs: tabs_snapshot,
                    active_tab_id: active_id_snapshot,
                    busy: busy(),
                    on_activate: move |id| {
                        active_tab_id.set(Some(id));
                        status.set(AppStatus::default());
                        context_menu.set(None);
                    },
                    on_close: move |id| request_close_tab(tabs, active_tab_id, id, dialog),
                    on_files: move |files| load_archive_files(files, signals, next_tab_id),
                }

                WorkspaceCard { class: "dzip-workspace-card", aria_label: "DZip Archive",
                    if let Some(tab) = active_tab_snapshot.clone() {
                        ArchiveWorkspace {
                            tab,
                            signals,
                            tree_visible: tree_visible(),
                            inspector_visible: inspector_visible(),
                            context_menu,
                            dialog,
                        }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| load_archive_files(files, signals, next_tab_id),
                            on_new: move |_| create_empty_tab(signals, next_tab_id),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "dzip-status-notice",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: if active_tab_snapshot.is_some() { "拖放 DZip 归档或要添加的文件".to_string() } else { "拖放 DZip 主文件及其分卷".to_string() } }
                    }
                    if busy() {
                        div { class: "dzip-busy-indicator", role: "status",
                            span {}
                            "正在处理 DZip…"
                        }
                    }
                }
            }
            if let Some(menu) = context_menu() {
                ArchiveContextMenu {
                    menu,
                    tab: active_tab_snapshot.clone(),
                    signals,
                    on_close: move |_| context_menu.set(None),
                    on_rename: move |target| {
                        if let Some(value) = target_name(&target, &tabs, active_tab_id) {
                            dialog.set(Some(EditDialog::Rename { target, value }));
                        }
                    },
                    on_delete: move |target| {
                        let label = target_name(&target, &tabs, active_tab_id)
                            .unwrap_or_else(|| "所选项目".to_string());
                        dialog.set(Some(EditDialog::Delete { target, label }));
                    },
                }
            }
            if let Some(current_dialog) = dialog() {
                EditDialogView {
                    dialog: current_dialog,
                    busy: busy(),
                    on_cancel: move |_| dialog.set(None),
                    on_change: move |next| dialog.set(Some(next)),
                    on_confirm: move |value| {
                        apply_dialog(value, signals);
                        dialog.set(None);
                    },
                }
            }
        }
    }
}

#[component]
fn ArchiveToolbar(
    tab: Option<ArchiveTab>,
    busy: bool,
    tree_visible: bool,
    inspector_visible: bool,
    on_files: EventHandler<Vec<FileData>>,
    on_new: EventHandler<()>,
    on_up: EventHandler<()>,
    on_extract: EventHandler<()>,
    on_add_files: EventHandler<Vec<FileData>>,
    on_save: EventHandler<()>,
    on_properties: EventHandler<()>,
    on_validate: EventHandler<()>,
    on_toggle_tree: EventHandler<()>,
    on_toggle_inspector: EventHandler<()>,
) -> Element {
    let has_archive = tab.is_some();
    let can_up = tab.as_ref().is_some_and(|tab| !tab.directory.is_empty());
    let dirty = tab.as_ref().is_some_and(|tab| tab.dirty);
    let can_save = tab.as_ref().is_some_and(|tab| !tab.entries.is_empty());
    rsx! {
        div { class: "ui-island ui-tool-page-actions dzip-page-actions",
            button { class: if tree_visible { "dzip-icon-button is-active" } else { "dzip-icon-button" }, title: "目录", aria_label: "显示或隐藏目录", disabled: !has_archive, onclick: move |_| on_toggle_tree.call(()), Glyph { name: "tree" } }
            label { class: if busy { "dzip-icon-button primary is-disabled" } else { "dzip-icon-button primary" }, title: "打开 DZip", aria_label: "打开 DZip",
                Glyph { name: "open" }
                input { class: "dzip-file-input", r#type: "file", accept: DZIP_FILE_ACCEPT, multiple: true, disabled: busy,
                    onchange: move |event| { let files = event.files(); if !files.is_empty() { on_files.call(files); } }
                }
            }
            button { class: "dzip-icon-button", title: "返回上级", aria_label: "返回上级", disabled: !can_up || busy, onclick: move |_| on_up.call(()), Glyph { name: "up" } }
            button { class: "dzip-icon-button", title: "提取", aria_label: "提取", disabled: !has_archive || busy, onclick: move |_| on_extract.call(()), Glyph { name: "extract" } }
            span { class: "dzip-toolbar-divider" }
            label { class: if has_archive && !busy { "dzip-icon-button" } else { "dzip-icon-button is-disabled" }, title: "添加文件", aria_label: "添加文件",
                Glyph { name: "add-file" }
                input { class: "dzip-file-input", r#type: "file", multiple: true, disabled: !has_archive || busy,
                    onchange: move |event| { let files = event.files(); if !files.is_empty() { on_add_files.call(files); } }
                }
            }
            button { class: if dirty { "dzip-icon-button has-changes" } else { "dzip-icon-button" }, title: "保存", aria_label: "保存", disabled: !can_save || busy, onclick: move |_| on_save.call(()), Glyph { name: "save" } if dirty { span { class: "dzip-unsaved-dot" } } }
            span { class: "dzip-toolbar-spacer" }
            if let Some(tab) = tab {
                span { class: "dzip-format-pill", "{tab.volume_count} VOL · {compression_label(tab.default_compression)}" }
            }
            button { class: "dzip-icon-button", title: "新建归档", aria_label: "新建归档", onclick: move |_| on_new.call(()), Glyph { name: "new" } }
            button { class: "dzip-icon-button", title: "检查归档", aria_label: "检查归档", disabled: !has_archive, onclick: move |_| on_validate.call(()), Glyph { name: "validate" } }
            button { class: "dzip-icon-button", title: "归档设置", aria_label: "归档设置", disabled: !has_archive, onclick: move |_| on_properties.call(()), Glyph { name: "settings" } }
            button { class: if inspector_visible { "dzip-icon-button is-active" } else { "dzip-icon-button" }, title: "属性", aria_label: "显示或隐藏属性", disabled: !has_archive, onclick: move |_| on_toggle_inspector.call(()), Glyph { name: "inspector" } }
        }
    }
}

#[component]
fn ArchiveTabStrip(
    tabs: Vec<ArchiveTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "dzip-tab-strip ui-document-tab-strip",
            div { class: "dzip-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 DZip",
                for tab in tabs {
                    div { key: "{tab.id}", class: tab_class(Some(tab.id) == active_tab_id, tab.dirty),
                        button { class: "dzip-tab-select ui-document-tab-label", role: "tab", aria_selected: Some(tab.id) == active_tab_id, title: "{tab.name}", disabled: busy,
                            onclick: { let id = tab.id; move |_| on_activate.call(id) },
                            span { class: "dzip-tab-dot ui-document-tab-dot" }
                            span { class: "dzip-tab-name ui-document-tab-name", "{tab.name}" }
                        }
                        button { class: "dzip-tab-close ui-document-tab-close", title: "关闭 {tab.name}", aria_label: "关闭 {tab.name}", disabled: busy,
                            onclick: { let id = tab.id; move |event| { event.stop_propagation(); on_close.call(id); } },
                            Glyph { name: "close" }
                        }
                    }
                }
                label { class: if busy { "dzip-tab-add ui-document-new-tab is-disabled" } else { "dzip-tab-add ui-document-new-tab" }, title: "打开 DZip", aria_label: "打开 DZip",
                    Glyph { name: "plus" }
                    input { class: "dzip-file-input", r#type: "file", accept: DZIP_FILE_ACCEPT, multiple: true, disabled: busy,
                        onchange: move |event| { let files = event.files(); if !files.is_empty() { on_files.call(files); } }
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
    on_new: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "dzip-empty-workspace",
            div { class: "dzip-empty-mark", Glyph { name: "archive" } }
            span { class: "dzip-empty-kicker", "DZIP ARCHIVE" }
            h2 { "打开或创建归档" }
            p { "选择主 .dz/.dzip 与所有数字分卷；文件只在本地解析和重建。" }
            div { class: "dzip-empty-actions",
                label { class: if busy { "dzip-primary-action is-disabled" } else { "dzip-primary-action" }, Glyph { name: "open" } "选择归档与分卷"
                    input { class: "dzip-file-input", r#type: "file", accept: DZIP_FILE_ACCEPT, multiple: true, disabled: busy,
                        onchange: move |event| { let files = event.files(); if !files.is_empty() { on_files.call(files); } }
                    }
                }
                button { class: "dzip-secondary-action", disabled: busy, onclick: move |_| on_new.call(()), Glyph { name: "new" } "新建归档" }
            }
            small { "支持 DZ · Zlib · BZip · LZMA · Copy · Zero" }
        }
    }
}

#[component]
fn ArchiveWorkspace(
    tab: ArchiveTab,
    signals: WorkspaceSignals,
    tree_visible: bool,
    inspector_visible: bool,
    context_menu: Signal<Option<ContextMenuState>>,
    dialog: Signal<Option<EditDialog>>,
) -> Element {
    let mut rows = browser_rows(&tab);
    let total_rows = rows.len();
    rows.truncate(tab.row_limit);
    let visible_rows = rows.len();
    let directories = all_directories(&tab.entries);
    let total_bytes = tab.entries.iter().map(|entry| entry.size).sum::<u64>();
    let packed_bytes = tab.entries.iter().try_fold(0_u64, |total, entry| {
        entry.packed_size.map(|size| total + size)
    });
    let selected = tab.selection.clone();
    let grid_class = match (tree_visible, inspector_visible) {
        (true, true) => "dzip-browser-grid",
        (true, false) => "dzip-browser-grid no-inspector",
        (false, true) => "dzip-browser-grid no-tree",
        (false, false) => "dzip-browser-grid no-tree no-inspector",
    };

    rsx! {
        div { class: "dzip-document",
            div { class: "dzip-summary-grid",
                SummaryCard { label: "文件", value: tab.entries.len().to_string(), glyph: "file" }
                SummaryCard { label: "解压大小", value: format_bytes(total_bytes), glyph: "size" }
                SummaryCard { label: "压缩大小", value: packed_bytes.map(format_bytes).unwrap_or_else(|| "待重建".to_string()), glyph: "packed" }
                SummaryCard { label: "分卷", value: format!("{} / {}", tab.loaded_volume_count, tab.volume_count), glyph: "volumes" }
            }
            if !tab.source_complete {
                div { class: "dzip-volume-warning", Glyph { name: "warning" } "当前只加载了部分分卷；浏览可用，但读取相关文件时可能失败。" }
            }
            div { class: "{grid_class}",
                if tree_visible {
                    DirectoryPanel { tab: tab.clone(), directories, signals }
                }
                section { class: "dzip-list-panel",
                    div { class: "dzip-list-toolbar",
                        div { class: "dzip-breadcrumb", aria_label: "当前位置",
                            button { class: if tab.directory.is_empty() { "is-current" } else { "" }, onclick: move |_| navigate_to(signals.tabs, signals.active_tab_id, String::new()), Glyph { name: "archive" } "{tab.name}" }
                            for crumb in breadcrumbs(&tab.directory) {
                                span { "/" }
                                button { class: if crumb.1 == tab.directory { "is-current" } else { "" },
                                    onclick: { let path = crumb.1.clone(); move |_| navigate_to(signals.tabs, signals.active_tab_id, path.clone()) },
                                    "{crumb.0}"
                                }
                            }
                        }
                        label { class: "dzip-search", Glyph { name: "search" }
                            input { value: "{tab.query}", placeholder: "搜索归档", aria_label: "搜索归档",
                                oninput: move |event| update_active_tab(signals.tabs, signals.active_tab_id, |tab| { tab.query = event.value(); tab.selection = None; tab.row_limit = ROW_PAGE_SIZE; })
                            }
                        }
                    }
                    div { class: "dzip-table-head",
                        SortableTableHeader {
                            label: "名称",
                            sort_key: BrowserSortKey::Name,
                            sort: tab.sort,
                            on_sort: move |key| update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                tab.sort = tab.sort.toggled(key);
                            }),
                        }
                        SortableTableHeader {
                            label: "原始",
                            class: "dzip-sort-header--numeric",
                            sort_key: BrowserSortKey::Size,
                            sort: tab.sort,
                            on_sort: move |key| update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                tab.sort = tab.sort.toggled(key);
                            }),
                        }
                        span { "压缩" }
                        SortableTableHeader {
                            label: "算法",
                            class: "dzip-sort-header--numeric",
                            sort_key: BrowserSortKey::Type,
                            sort: tab.sort,
                            on_sort: move |key| update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                tab.sort = tab.sort.toggled(key);
                            }),
                        }
                        span { "分卷" }
                    }
                    div {
                        class: "dzip-table-body",
                        oncontextmenu: move |event| {
                            event.prevent_default();
                            let point = event.client_coordinates();
                            context_menu.set(Some(ContextMenuState { target: None, x: point.x, y: point.y }));
                        },
                        if rows.is_empty() {
                            div { class: "dzip-list-empty", Glyph { name: if tab.query.is_empty() { "folder" } else { "search" } } strong { if tab.query.is_empty() { "此目录为空" } else { "没有匹配项" } } small { if tab.query.is_empty() { "可添加文件到当前目录" } else { "尝试其他搜索关键词" } } }
                        }
                        for row in rows {
                            ArchiveRow {
                                key: "{row.key}",
                                row: row.clone(),
                                selected: selected.as_ref() == Some(&row.selection),
                                on_select: move |selection| select_item(signals.tabs, signals.active_tab_id, selection),
                                on_open: move |selection| open_selection(selection, signals),
                                on_context: move |menu| context_menu.set(Some(menu)),
                            }
                        }
                        if visible_rows < total_rows {
                            button { class: "dzip-load-more", onclick: move |event| { event.stop_propagation(); update_active_tab(signals.tabs, signals.active_tab_id, |tab| tab.row_limit = tab.row_limit.saturating_add(ROW_PAGE_SIZE)); },
                                "再显示 {usize::min(ROW_PAGE_SIZE, total_rows - visible_rows)} 项"
                                small { "{visible_rows} / {total_rows}" }
                            }
                        }
                    }
                }
                if inspector_visible {
                    InspectorPanel {
                        tab: tab.clone(),
                        signals,
                        on_extract: move |_| extract_current(signals),
                        on_rename: move |target| {
                            if let Some(value) = target_name(&target, &signals.tabs, signals.active_tab_id) {
                                dialog.set(Some(EditDialog::Rename { target, value }));
                            }
                        },
                        on_delete: move |target| {
                            let label = target_name(&target, &signals.tabs, signals.active_tab_id).unwrap_or_else(|| "所选项目".to_string());
                            dialog.set(Some(EditDialog::Delete { target, label }));
                        },
                        on_replace: move |files| replace_selected(files, signals),
                        on_properties: move |_| dialog.set(Some(EditDialog::ArchiveProperties {
                            name: tab.name.clone(),
                            alignment: tab.alignment.to_string(),
                            volumes: tab.volume_count.to_string(),
                            default_compression: compression_code(tab.default_compression).to_string(),
                            common_buffer: tab.use_common_buffer,
                        })),
                    }
                }
            }
        }
    }
}

#[component]
fn SummaryCard(label: &'static str, value: String, glyph: &'static str) -> Element {
    rsx! {
        article { class: "dzip-summary-card",
            span { class: "dzip-summary-icon", Glyph { name: glyph } }
            div { small { "{label}" } strong { "{value}" } }
        }
    }
}

#[component]
fn SortableTableHeader(
    label: &'static str,
    #[props(default)] class: &'static str,
    sort_key: BrowserSortKey,
    sort: BrowserSort,
    on_sort: EventHandler<BrowserSortKey>,
) -> Element {
    let active = sort.key == sort_key;
    let class = if active {
        format!("dzip-sort-header {class} is-active")
    } else {
        format!("dzip-sort-header {class}")
    };
    let direction_label = if active && sort.aria_value(sort_key) == "ascending" {
        "倒序"
    } else {
        "正序"
    };
    rsx! {
        button {
            r#type: "button",
            class,
            role: "columnheader",
            aria_sort: sort.aria_value(sort_key),
            title: "按{label}{direction_label}排列",
            onclick: move |_| on_sort.call(sort_key),
            span { "{label}" }
            span {
                class: if active { "dzip-sort-indicator is-active" } else { "dzip-sort-indicator" },
                aria_hidden: "true",
                {sort.indicator(sort_key)}
            }
        }
    }
}

#[component]
fn DirectoryPanel(tab: ArchiveTab, directories: Vec<String>, signals: WorkspaceSignals) -> Element {
    rsx! {
        aside { class: "dzip-tree-panel",
            div { class: "dzip-panel-heading", span { Glyph { name: "tree" } "目录" } small { "{directories.len()}" } }
            nav { class: "dzip-tree-list", aria_label: "DZip 目录",
                button { class: if tab.directory.is_empty() { "dzip-tree-row is-active" } else { "dzip-tree-row" }, onclick: move |_| navigate_to(signals.tabs, signals.active_tab_id, String::new()),
                    span { class: "dzip-tree-glyph", Glyph { name: "archive" } }
                    span { class: "dzip-tree-name", "归档根目录" }
                }
                for path in directories {
                    button { class: if tab.directory == path { "dzip-tree-row is-active" } else { "dzip-tree-row" }, style: "--depth: {path_depth(&path)};", title: "{path}",
                        onclick: { let path = path.clone(); move |_| navigate_to(signals.tabs, signals.active_tab_id, path.clone()) },
                        span { class: "dzip-tree-glyph", Glyph { name: "folder" } }
                        span { class: "dzip-tree-name", "{file_name(&path)}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ArchiveRow(
    row: BrowserRow,
    selected: bool,
    on_select: EventHandler<BrowserSelection>,
    on_open: EventHandler<BrowserSelection>,
    on_context: EventHandler<ContextMenuState>,
) -> Element {
    let selection = row.selection.clone();
    let packed_label = row
        .packed_size
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_string());
    let compression_label = row.compression.map(compression_label).unwrap_or("目录");
    let volume_label = row
        .volume
        .map(|volume| volume.to_string())
        .unwrap_or_else(|| "—".to_string());
    rsx! {
        div {
            class: if selected { "dzip-table-row is-selected" } else { "dzip-table-row" },
            role: "row",
            tabindex: "0",
            onclick: { let selection = selection.clone(); move |event| { event.stop_propagation(); on_select.call(selection.clone()); } },
            ondoubleclick: { let selection = selection.clone(); move |event| { event.stop_propagation(); on_open.call(selection.clone()); } },
            oncontextmenu: { let selection = selection.clone(); move |event| { event.prevent_default(); event.stop_propagation(); let point = event.client_coordinates(); on_context.call(ContextMenuState { target: Some(selection.clone()), x: point.x, y: point.y }); } },
            div { class: "dzip-row-name", span { class: if row.kind == BrowserRowKind::Directory { "dzip-row-icon directory" } else { "dzip-row-icon file" }, Glyph { name: if row.kind == BrowserRowKind::Directory { "folder" } else { "file" } } } div { strong { "{row.name}" } small { "{row.full_path}" } } }
            span { class: "dzip-row-size", "{format_bytes(row.size)}" }
            span { class: "dzip-row-packed", "{packed_label}" }
            span { class: "dzip-row-codec", "{compression_label}" }
            span { class: "dzip-row-volume", "{volume_label}" }
        }
    }
}

#[component]
fn InspectorPanel(
    tab: ArchiveTab,
    signals: WorkspaceSignals,
    on_extract: EventHandler<()>,
    on_rename: EventHandler<BrowserSelection>,
    on_delete: EventHandler<BrowserSelection>,
    on_replace: EventHandler<Vec<FileData>>,
    on_properties: EventHandler<()>,
) -> Element {
    let selected_entry = tab
        .selection
        .as_ref()
        .and_then(|selection| match selection {
            BrowserSelection::Entry(id) => {
                tab.entries.iter().find(|entry| entry.id == *id).cloned()
            }
            BrowserSelection::Directory(_) => None,
        });
    rsx! {
        aside { class: "dzip-inspector-panel",
            div { class: "dzip-panel-heading", span { Glyph { name: "inspector" } "属性" } }
            if let Some(entry) = selected_entry {
                div { class: "dzip-inspector-hero", span { class: "dzip-inspector-icon", Glyph { name: "file" } } div { strong { "{file_name(&entry.path)}" } small { "条目 #{entry.source_id.map(|id| id + 1).unwrap_or(entry.id as usize)}" } } }
                dl { class: "dzip-properties",
                    div { dt { "路径" } dd { title: "{entry.path}", "{entry.path}" } }
                    div { dt { "原始大小" } dd { "{format_bytes(entry.size)}" } }
                    div { dt { "压缩大小" } dd { "{format_optional_bytes(entry.packed_size)}" } }
                    div { dt { "数据块" } dd { "{entry.segments.as_ref().map(Vec::len).unwrap_or(1)}" } }
                }
                label { class: "dzip-field-label", "压缩算法"
                    select { value: "{compression_code(entry.compression)}", onchange: { let id = entry.id; move |event| { if let Some(compression) = parse_compression(&event.value()) { change_entry_compression(signals, id, compression); } } },
                        for compression in Compression::ALL { option { value: "{compression_code(compression)}", "{compression_label(compression)}" } }
                    }
                }
                label { class: "dzip-field-label", "分卷"
                    select { value: "{entry.volume}", onchange: { let id = entry.id; move |event| { if let Ok(volume) = event.value().parse::<u16>() { change_entry_volume(signals, id, volume); } } },
                        for volume in 0..tab.volume_count { option { value: "{volume}", "Volume {volume}" } }
                    }
                }
                div { class: "dzip-inspector-actions",
                    button { onclick: move |_| on_extract.call(()), Glyph { name: "extract" } "提取" }
                    label { Glyph { name: "replace" } "替换"
                        input { class: "dzip-file-input", r#type: "file", onchange: move |event| { let files = event.files(); if !files.is_empty() { on_replace.call(files); } } }
                    }
                    button { onclick: { let target = BrowserSelection::Entry(entry.id); move |_| on_rename.call(target.clone()) }, Glyph { name: "rename" } "重命名" }
                    button { class: "danger", onclick: { let target = BrowserSelection::Entry(entry.id); move |_| on_delete.call(target.clone()) }, Glyph { name: "delete" } "删除" }
                }
            } else if let Some(BrowserSelection::Directory(path)) = tab.selection.as_ref() {
                div { class: "dzip-inspector-hero", span { class: "dzip-inspector-icon", Glyph { name: "folder" } } div { strong { "{file_name(path)}" } small { "目录" } } }
                dl { class: "dzip-properties",
                    div { dt { "路径" } dd { "{path}" } }
                    div { dt { "文件" } dd { "{entries_in_directory(&tab.entries, path).len()}" } }
                }
                div { class: "dzip-inspector-actions",
                    button { onclick: move |_| on_extract.call(()), Glyph { name: "extract" } "提取" }
                    button { onclick: { let target = BrowserSelection::Directory(path.clone()); move |_| on_rename.call(target.clone()) }, Glyph { name: "rename" } "重命名" }
                    button { class: "danger", onclick: { let target = BrowserSelection::Directory(path.clone()); move |_| on_delete.call(target.clone()) }, Glyph { name: "delete" } "删除" }
                }
            } else {
                div { class: "dzip-inspector-overview",
                    span { class: "dzip-inspector-icon", Glyph { name: "archive" } }
                    h3 { "{tab.name}" }
                    p { if tab.session_id.is_some() { "已打开的 DZip 归档" } else { "新建 DZip 归档" } }
                    dl { class: "dzip-properties",
                        div { dt { "文件" } dd { "{tab.entries.len()}" } }
                        div { dt { "数据块" } dd { "{tab.original_chunk_count}" } }
                        div { dt { "源大小" } dd { "{format_bytes(tab.source_size)}" } }
                        div { dt { "默认算法" } dd { "{compression_label(tab.default_compression)}" } }
                        div { dt { "对齐" } dd { if tab.alignment == 0 { "无" } else { "{tab.alignment} B" } } }
                    }
                    button { class: "dzip-wide-action", onclick: move |_| on_properties.call(()), Glyph { name: "settings" } "归档设置" }
                }
            }
        }
    }
}

#[component]
fn ArchiveContextMenu(
    menu: ContextMenuState,
    tab: Option<ArchiveTab>,
    signals: WorkspaceSignals,
    on_close: EventHandler<()>,
    on_rename: EventHandler<BrowserSelection>,
    on_delete: EventHandler<BrowserSelection>,
) -> Element {
    let target = menu.target.clone();
    let can_edit = target.is_some();
    let can_open = matches!(target, Some(BrowserSelection::Directory(_)));
    rsx! {
        div { class: "dzip-context-menu", style: "left: {menu.x}px; top: {menu.y}px;", role: "menu", onclick: move |event| event.stop_propagation(),
            if can_open {
                button { onclick: { let target = target.clone(); move |_| { if let Some(target) = target.clone() { open_selection(target, signals); } on_close.call(()); } }, Glyph { name: "open" } "打开" }
            }
            button { onclick: move |_| { extract_current(signals); on_close.call(()); }, disabled: tab.is_none(), Glyph { name: "extract" } "提取" }
            div { class: "dzip-context-separator" }
            button { disabled: !can_edit, onclick: { let target = target.clone(); move |_| { if let Some(target) = target.clone() { on_rename.call(target); } on_close.call(()); } }, Glyph { name: "rename" } "重命名" }
            button { class: "danger", disabled: !can_edit, onclick: { let target = target.clone(); move |_| { if let Some(target) = target.clone() { on_delete.call(target); } on_close.call(()); } }, Glyph { name: "delete" } "删除" }
        }
    }
}

#[component]
fn EditDialogView(
    dialog: EditDialog,
    busy: bool,
    on_cancel: EventHandler<()>,
    on_change: EventHandler<EditDialog>,
    on_confirm: EventHandler<EditDialog>,
) -> Element {
    let (title, confirm_label, danger) = match &dialog {
        EditDialog::Rename { .. } => ("重命名", "应用", false),
        EditDialog::Delete { .. } => ("删除项目", "删除", true),
        EditDialog::ArchiveProperties { .. } => ("归档设置", "应用", false),
        EditDialog::CloseDirty { .. } => ("尚未保存", "放弃更改", true),
    };
    rsx! {
        div { class: "dzip-dialog-layer", role: "presentation",
            button { class: "dzip-dialog-backdrop", aria_label: "关闭", onclick: move |_| on_cancel.call(()) }
            section { class: "dzip-dialog", role: "dialog", aria_modal: "true", aria_label: "{title}",
                header { div { span { class: "dzip-dialog-kicker", "DZIP ARCHIVE" } h2 { "{title}" } } button { aria_label: "关闭", onclick: move |_| on_cancel.call(()), Glyph { name: "close" } } }
                div { class: "dzip-dialog-body",
                    match dialog.clone() {
                        EditDialog::Rename { target, value } => rsx! {
                            label { span { "新名称" } input { autofocus: true, value: "{value}", oninput: move |event| on_change.call(EditDialog::Rename { target: target.clone(), value: event.value() }) } }
                            p { class: "dzip-dialog-hint", "名称不能包含路径分隔符；目录重命名会更新其全部子文件路径。" }
                        },
                        EditDialog::Delete { target: _, label } => rsx! {
                            div { class: "dzip-delete-warning", Glyph { name: "delete" } p { "确定删除 " strong { "{label}" } "？目录中的全部文件也会被删除。" } }
                        },
                        EditDialog::ArchiveProperties { name, alignment, volumes, default_compression, common_buffer } => rsx! {
                            ArchivePropertiesEditor { name, alignment, volumes, default_compression, common_buffer, on_change }
                        },
                        EditDialog::CloseDirty { tab_id: _, name } => rsx! {
                            div { class: "dzip-delete-warning", Glyph { name: "warning" } p { strong { "{name}" } " 包含尚未保存的更改，关闭后无法恢复。" } }
                        },
                    }
                }
                footer {
                    button { class: "secondary", onclick: move |_| on_cancel.call(()), "取消" }
                    button { class: if danger { "primary danger" } else { "primary" }, disabled: busy, onclick: { let dialog = dialog.clone(); move |_| on_confirm.call(dialog.clone()) }, "{confirm_label}" }
                }
            }
        }
    }
}

#[component]
fn ArchivePropertiesEditor(
    name: String,
    alignment: String,
    volumes: String,
    default_compression: String,
    common_buffer: bool,
    on_change: EventHandler<EditDialog>,
) -> Element {
    let alignment_for_name = alignment.clone();
    let volumes_for_name = volumes.clone();
    let compression_for_name = default_compression.clone();
    let name_for_alignment = name.clone();
    let volumes_for_alignment = volumes.clone();
    let compression_for_alignment = default_compression.clone();
    let name_for_volumes = name.clone();
    let alignment_for_volumes = alignment.clone();
    let compression_for_volumes = default_compression.clone();
    let name_for_compression = name.clone();
    let alignment_for_compression = alignment.clone();
    let volumes_for_compression = volumes.clone();
    let name_for_common = name.clone();
    let alignment_for_common = alignment.clone();
    let volumes_for_common = volumes.clone();
    let compression_for_common = default_compression.clone();
    rsx! {
        label { span { "归档文件名" }
            input { value: "{name}", placeholder: "archive.dz", oninput: move |event| on_change.call(EditDialog::ArchiveProperties {
                name: event.value(), alignment: alignment_for_name.clone(), volumes: volumes_for_name.clone(), default_compression: compression_for_name.clone(), common_buffer,
            }) }
        }
        div { class: "dzip-form-grid",
            label { span { "数据对齐" }
                select { value: "{alignment}", onchange: move |event| on_change.call(EditDialog::ArchiveProperties {
                    name: name_for_alignment.clone(), alignment: event.value(), volumes: volumes_for_alignment.clone(), default_compression: compression_for_alignment.clone(), common_buffer,
                }),
                    option { value: "0", "无" }
                    option { value: "512", "512 B" }
                    option { value: "2048", "2048 B" }
                    option { value: "4096", "4096 B" }
                }
            }
            label { span { "分卷数量" }
                input { r#type: "number", min: "1", max: "65535", value: "{volumes}", oninput: move |event| on_change.call(EditDialog::ArchiveProperties {
                    name: name_for_volumes.clone(), alignment: alignment_for_volumes.clone(), volumes: event.value(), default_compression: compression_for_volumes.clone(), common_buffer,
                }) }
            }
        }
        label { span { "新增文件默认算法" }
            select { value: "{default_compression}", onchange: move |event| on_change.call(EditDialog::ArchiveProperties {
                name: name_for_compression.clone(), alignment: alignment_for_compression.clone(), volumes: volumes_for_compression.clone(), default_compression: event.value(), common_buffer,
            }),
                for compression in Compression::ALL { option { value: "{compression_code(compression)}", "{compression_label(compression)}" } }
            }
        }
        label { class: "dzip-toggle-row",
            div { strong { "DZ Common Buffer" } small { "让多个 DZ 文件共享参考数据；更慢但可能提高整体压缩率。" } }
            input { r#type: "checkbox", checked: common_buffer, onchange: move |event| on_change.call(EditDialog::ArchiveProperties {
                name: name_for_common.clone(), alignment: alignment_for_common.clone(), volumes: volumes_for_common.clone(), default_compression: compression_for_common.clone(), common_buffer: event.checked(),
            }) }
        }
        p { class: "dzip-dialog-hint", "调整文件压缩算法或分卷后，该文件原有的多数据块布局会在重建时合并。" }
    }
}

fn load_archive_files(
    files: Vec<FileData>,
    mut signals: WorkspaceSignals,
    mut next_tab_id: Signal<u64>,
) {
    if (signals.busy)() {
        return;
    }
    let mut busy = signals.busy;
    busy.set(true);
    set_status(
        signals.status,
        AppStatus::new("正在读取并索引 DZip…", StatusTone::Neutral),
    );
    spawn(async move {
        match prepare_archive_files(files).await {
            Ok((main_name, main_bytes, auxiliary)) => {
                match platform::open_archive(main_name.clone(), main_bytes, auxiliary).await {
                    Ok(summary) => {
                        let id = next_tab_id();
                        next_tab_id.set(id.wrapping_add(1).max(1));
                        let count = summary.entries.len();
                        let volumes = summary.volume_count;
                        signals.tabs.write().push(ArchiveTab::opened(id, summary));
                        signals.active_tab_id.set(Some(id));
                        set_status(
                            signals.status,
                            AppStatus::new(
                                format!("已打开 {main_name} · {count} 个文件 · {volumes} 个分卷"),
                                StatusTone::Success,
                            ),
                        );
                    }
                    Err(error) => set_status(
                        signals.status,
                        AppStatus::new(format!("无法打开 {main_name}：{error}"), StatusTone::Error),
                    ),
                }
            }
            Err(error) => set_status(signals.status, AppStatus::new(error, StatusTone::Error)),
        }
        busy.set(false);
    });
}

async fn prepare_archive_files(
    files: Vec<FileData>,
) -> Result<(String, Vec<u8>, Vec<NamedBytes>), String> {
    let mut main = Vec::new();
    let mut auxiliary = Vec::new();
    for file in files {
        let name = file.name();
        let bytes = file
            .read_bytes()
            .await
            .map_err(|error| format!("无法读取 {name}：{error}"))?
            .to_vec();
        if is_main_archive_name(&name) {
            main.push((name, bytes));
        } else if is_volume_name(&name) {
            auxiliary.push(NamedBytes { name, bytes });
        }
    }
    if main.is_empty() {
        return Err("请选择一个 .dz 或 .dzip 主文件".to_string());
    }
    if main.len() > 1 {
        return Err("一次只能打开一个 DZip 主文件；请分别打开多个归档".to_string());
    }
    auxiliary.sort_by_key(|file| file.name.to_ascii_lowercase());
    let (name, bytes) = main.pop().expect("checked one main archive");
    Ok((name, bytes, auxiliary))
}

fn create_empty_tab(mut signals: WorkspaceSignals, mut next_tab_id: Signal<u64>) {
    if (signals.busy)() {
        return;
    }
    let id = next_tab_id();
    next_tab_id.set(id.wrapping_add(1).max(1));
    signals.tabs.write().push(ArchiveTab::empty(id));
    signals.active_tab_id.set(Some(id));
    set_status(
        signals.status,
        AppStatus::new("已创建空白 DZip；请添加文件", StatusTone::Neutral),
    );
}

fn import_files(files: Vec<FileData>, signals: WorkspaceSignals) {
    if (signals.busy)() || (signals.active_tab_id)().is_none() {
        return;
    }
    let Some(tab) = active_tab(&signals.tabs, signals.active_tab_id) else {
        return;
    };
    let directory = tab.directory;
    let default_compression = tab.default_compression;
    let mut busy = signals.busy;
    busy.set(true);
    set_status(
        signals.status,
        AppStatus::new("正在添加文件…", StatusTone::Neutral),
    );
    spawn(async move {
        let mut added = Vec::new();
        for file in files {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => added.push((name, bytes.to_vec())),
                Err(error) => {
                    set_status(
                        signals.status,
                        AppStatus::new(format!("无法读取 {name}：{error}"), StatusTone::Error),
                    );
                    busy.set(false);
                    return;
                }
            }
        }
        let count = added.len();
        update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
            let entries = std::sync::Arc::make_mut(&mut tab.entries);
            for (name, bytes) in added {
                let path = join_path(&directory, &name);
                if let Some(existing) = entries.iter_mut().find(|entry| {
                    ArchivePathKey::from_archive_str(&entry.path)
                        == ArchivePathKey::from_archive_str(&path)
                }) {
                    existing.replace_bytes(bytes);
                } else {
                    let id = tab.next_entry_id;
                    tab.next_entry_id = tab.next_entry_id.wrapping_add(1).max(1);
                    entries.push(DraftEntry::replacement(
                        id,
                        path,
                        bytes,
                        default_compression,
                    ));
                }
            }
            tab.dirty = true;
        });
        set_status(
            signals.status,
            AppStatus::new(
                format!("已添加或替换 {count} 个文件 · 尚未保存"),
                StatusTone::Warning,
            ),
        );
        busy.set(false);
    });
}

fn replace_selected(files: Vec<FileData>, signals: WorkspaceSignals) {
    let Some(BrowserSelection::Entry(entry_id)) =
        active_selection(&signals.tabs, signals.active_tab_id)
    else {
        return;
    };
    let Some(file) = files.into_iter().next() else {
        return;
    };
    let name = file.name();
    let mut busy = signals.busy;
    busy.set(true);
    spawn(async move {
        match file.read_bytes().await {
            Ok(bytes) => {
                update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                    if let Some(entry) = std::sync::Arc::make_mut(&mut tab.entries)
                        .iter_mut()
                        .find(|entry| entry.id == entry_id)
                    {
                        entry.replace_bytes(bytes.to_vec());
                        tab.dirty = true;
                    }
                });
                set_status(
                    signals.status,
                    AppStatus::new(
                        format!("已使用 {name} 替换文件 · 尚未保存"),
                        StatusTone::Warning,
                    ),
                );
            }
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("无法读取 {name}：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn save_active_archive(mut signals: WorkspaceSignals) {
    let Some(tab) = active_tab(&signals.tabs, signals.active_tab_id) else {
        return;
    };
    if (signals.busy)() || tab.entries.is_empty() {
        return;
    }
    let tab_id = tab.id;
    let request = BuildRequest {
        source_session: tab.session_id,
        archive_name: normalize_archive_name(&tab.name),
        volume_count: tab.volume_count,
        alignment: tab.alignment,
        range_settings: tab.range_settings,
        use_common_buffer: tab.use_common_buffer,
        entries: tab.entries.as_ref().clone(),
    };
    let mut busy = signals.busy;
    busy.set(true);
    set_status(
        signals.status,
        AppStatus::new("正在压缩并重建 DZip…", StatusTone::Neutral),
    );
    spawn(async move {
        match platform::build_archive(request).await {
            Ok(built) => {
                let new_session = built.summary.session_id;
                let volume_count = built.volumes.len();
                match platform::save_volumes(built.volumes).await {
                    Ok(Some(location)) => {
                        let mut old_to_close = None;
                        if let Some(tab) =
                            signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
                        {
                            old_to_close = tab.replace_with_summary(built.summary);
                        }
                        if let Some(session) =
                            old_to_close.filter(|session| *session != new_session)
                        {
                            platform::close_archive(session).await;
                        }
                        set_status(
                            signals.status,
                            AppStatus::new(
                                format!("已保存 {volume_count} 个分卷 · {location}"),
                                StatusTone::Success,
                            ),
                        );
                    }
                    Ok(None) => {
                        platform::close_archive(new_session).await;
                        set_status(signals.status, AppStatus::default());
                    }
                    Err(error) => {
                        platform::close_archive(new_session).await;
                        set_status(signals.status, AppStatus::new(error, StatusTone::Error));
                    }
                }
            }
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("无法重建 DZip：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn extract_current(signals: WorkspaceSignals) {
    let Some(tab) = active_tab(&signals.tabs, signals.active_tab_id) else {
        return;
    };
    if (signals.busy)() {
        return;
    }
    let entries = match &tab.selection {
        Some(BrowserSelection::Entry(id)) => tab
            .entries
            .iter()
            .filter(|entry| entry.id == *id)
            .cloned()
            .collect(),
        Some(BrowserSelection::Directory(path)) => entries_in_directory(&tab.entries, path),
        None => tab.entries.as_ref().clone(),
    };
    if entries.is_empty() {
        set_status(
            signals.status,
            AppStatus::new("没有可提取的文件", StatusTone::Warning),
        );
        return;
    }
    let count = entries.len();
    let request = MaterializeRequest {
        source_session: tab.session_id,
        entries,
    };
    let mut busy = signals.busy;
    busy.set(true);
    set_status(
        signals.status,
        AppStatus::new(format!("正在提取 {count} 个文件…"), StatusTone::Neutral),
    );
    spawn(async move {
        match platform::materialize_entries(request).await {
            Ok(files) => match platform::export_files(files).await {
                Ok(Some(message)) => {
                    set_status(signals.status, AppStatus::new(message, StatusTone::Success))
                }
                Ok(None) => set_status(signals.status, AppStatus::default()),
                Err(error) => set_status(
                    signals.status,
                    AppStatus::new(format!("提取失败：{error}"), StatusTone::Error),
                ),
            },
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("解码失败：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn validate_active(signals: WorkspaceSignals) {
    let Some(tab) = active_tab(&signals.tabs, signals.active_tab_id) else {
        return;
    };
    let mut paths = BTreeSet::new();
    let duplicate = tab.entries.iter().find_map(|entry| {
        let key = ArchivePathKey::from_archive_str(&entry.path)
            .as_bytes()
            .to_vec();
        (!paths.insert(key)).then_some(entry.path.clone())
    });
    let invalid_volume = tab
        .entries
        .iter()
        .find(|entry| usize::from(entry.volume) >= tab.volume_count);
    let value = if let Some(path) = duplicate {
        AppStatus::new(format!("检查失败：重复路径 {path}"), StatusTone::Error)
    } else if let Some(entry) = invalid_volume {
        AppStatus::new(
            format!("检查失败：{} 指向不存在的分卷 {}", entry.path, entry.volume),
            StatusTone::Error,
        )
    } else if tab.entries.is_empty() {
        AppStatus::new("归档为空，请先添加文件", StatusTone::Warning)
    } else {
        AppStatus::new(
            format!("{} 已通过路径、分卷和编辑状态检查", tab.name),
            StatusTone::Success,
        )
    };
    set_status(signals.status, value);
}

fn apply_dialog(dialog: EditDialog, signals: WorkspaceSignals) {
    match dialog {
        EditDialog::Rename { target, value } => rename_target(target, value, signals),
        EditDialog::Delete { target, .. } => delete_target(target, signals),
        EditDialog::ArchiveProperties {
            name,
            alignment,
            volumes,
            default_compression,
            common_buffer,
        } => apply_archive_properties(
            name,
            alignment,
            volumes,
            default_compression,
            common_buffer,
            signals,
        ),
        EditDialog::CloseDirty { tab_id, .. } => {
            close_tab(signals.tabs, signals.active_tab_id, tab_id)
        }
    }
}

fn rename_target(target: BrowserSelection, value: String, signals: WorkspaceSignals) {
    let value = value.trim();
    if value.is_empty() || value.contains(['/', '\\']) {
        set_status(
            signals.status,
            AppStatus::new("名称不能为空，也不能包含路径分隔符", StatusTone::Error),
        );
        return;
    }
    let mut changed = false;
    update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
        let entries = std::sync::Arc::make_mut(&mut tab.entries);
        match &target {
            BrowserSelection::Entry(id) => {
                if let Some(entry) = entries.iter_mut().find(|entry| entry.id == *id) {
                    entry.path = replace_file_name(&entry.path, value);
                    changed = true;
                }
            }
            BrowserSelection::Directory(path) => {
                let parent = parent_directory(path);
                let replacement = join_path(&parent, value);
                let prefix = format!("{}/", path.trim_matches('/'));
                for entry in entries.iter_mut() {
                    if let Some(suffix) = entry.path.trim_matches('/').strip_prefix(&prefix) {
                        entry.path = join_path(&replacement, suffix);
                        changed = true;
                    }
                }
                if tab.directory == *path || tab.directory.starts_with(&prefix) {
                    let suffix = tab.directory.strip_prefix(path).unwrap_or_default();
                    tab.directory = format!("{replacement}{suffix}")
                        .trim_matches('/')
                        .to_string();
                }
            }
        }
        if changed {
            tab.selection = None;
            tab.dirty = true;
        }
    });
    if changed {
        set_status(
            signals.status,
            AppStatus::new("名称已更新 · 尚未保存", StatusTone::Warning),
        );
    }
}

fn delete_target(target: BrowserSelection, signals: WorkspaceSignals) {
    let mut removed = 0_usize;
    update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
        let entries = std::sync::Arc::make_mut(&mut tab.entries);
        let before = entries.len();
        match &target {
            BrowserSelection::Entry(id) => entries.retain(|entry| entry.id != *id),
            BrowserSelection::Directory(path) => {
                let prefix = format!("{}/", path.trim_matches('/'));
                entries.retain(|entry| !entry.path.trim_matches('/').starts_with(&prefix));
                if tab.directory == *path || tab.directory.starts_with(&prefix) {
                    tab.directory = parent_directory(path);
                }
            }
        }
        removed = before - entries.len();
        if removed > 0 {
            tab.selection = None;
            tab.dirty = true;
        }
    });
    if removed > 0 {
        set_status(
            signals.status,
            AppStatus::new(
                format!("已删除 {removed} 个文件 · 尚未保存"),
                StatusTone::Warning,
            ),
        );
    }
}

fn apply_archive_properties(
    name: String,
    alignment: String,
    volumes: String,
    default_compression: String,
    common_buffer: bool,
    signals: WorkspaceSignals,
) {
    let Ok(alignment) = alignment.parse::<u32>() else {
        set_status(
            signals.status,
            AppStatus::new("无效的数据对齐值", StatusTone::Error),
        );
        return;
    };
    let Ok(volume_count) = volumes.parse::<usize>() else {
        set_status(
            signals.status,
            AppStatus::new("无效的分卷数量", StatusTone::Error),
        );
        return;
    };
    if !(1..=u16::MAX as usize).contains(&volume_count) {
        set_status(
            signals.status,
            AppStatus::new("分卷数量必须位于 1..=65535", StatusTone::Error),
        );
        return;
    }
    let Some(default_compression) = parse_compression(&default_compression) else {
        set_status(
            signals.status,
            AppStatus::new("无效的压缩算法", StatusTone::Error),
        );
        return;
    };
    let normalized_name = normalize_archive_name(&name);
    let mut rejected = None;
    update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
        if let Some(entry) = tab
            .entries
            .iter()
            .find(|entry| usize::from(entry.volume) >= volume_count)
        {
            rejected = Some(format!(
                "{} 位于 Volume {}，不能将分卷数量减少到 {}",
                entry.path, entry.volume, volume_count
            ));
            return;
        }
        let changed = tab.name != normalized_name
            || tab.alignment != alignment
            || tab.volume_count != volume_count
            || tab.default_compression != default_compression
            || tab.use_common_buffer != common_buffer;
        tab.name = normalized_name.clone();
        tab.alignment = alignment;
        tab.volume_count = volume_count;
        tab.default_compression = default_compression;
        tab.use_common_buffer = common_buffer;
        tab.dirty |= changed;
    });
    if let Some(error) = rejected {
        set_status(signals.status, AppStatus::new(error, StatusTone::Error));
    } else {
        set_status(
            signals.status,
            AppStatus::new("归档设置已更新 · 尚未保存", StatusTone::Warning),
        );
    }
}

fn change_entry_compression(signals: WorkspaceSignals, entry_id: u64, compression: Compression) {
    update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
        if let Some(entry) = std::sync::Arc::make_mut(&mut tab.entries)
            .iter_mut()
            .find(|entry| entry.id == entry_id)
        {
            let previous = entry.compression;
            entry.replace_compression(compression);
            tab.dirty |= previous != compression;
        }
    });
    set_status(
        signals.status,
        AppStatus::new("文件压缩算法已更新 · 尚未保存", StatusTone::Warning),
    );
}

fn change_entry_volume(signals: WorkspaceSignals, entry_id: u64, volume: u16) {
    update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
        if usize::from(volume) >= tab.volume_count {
            return;
        }
        if let Some(entry) = std::sync::Arc::make_mut(&mut tab.entries)
            .iter_mut()
            .find(|entry| entry.id == entry_id)
        {
            let previous = entry.volume;
            entry.replace_volume(volume);
            tab.dirty |= previous != volume;
        }
    });
    set_status(
        signals.status,
        AppStatus::new("文件分卷已更新 · 尚未保存", StatusTone::Warning),
    );
}

fn browser_rows(tab: &ArchiveTab) -> Vec<BrowserRow> {
    let query = tab.query.trim().to_ascii_lowercase();
    if !query.is_empty() {
        let mut files = tab
            .entries
            .iter()
            .filter(|entry| entry.path.to_ascii_lowercase().contains(&query))
            .map(entry_row)
            .collect::<Vec<_>>();
        sort_browser_rows(&mut files, tab.sort);
        return files;
    }

    #[derive(Default)]
    struct FolderAggregate {
        count: usize,
        size: u64,
        packed: Option<u64>,
        volume: Option<u16>,
    }

    let directory = tab.directory.trim_matches('/');
    let prefix = if directory.is_empty() {
        String::new()
    } else {
        format!("{directory}/")
    };
    let mut folders = BTreeMap::<String, FolderAggregate>::new();
    let mut files = Vec::new();
    for entry in tab.entries.iter() {
        let path = entry.path.trim_matches('/');
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        if let Some((folder, _)) = relative.split_once('/') {
            if folder.is_empty() {
                continue;
            }
            let aggregate = folders
                .entry(folder.to_string())
                .or_insert_with(|| FolderAggregate {
                    packed: Some(0),
                    volume: Some(entry.volume),
                    ..FolderAggregate::default()
                });
            aggregate.count += 1;
            aggregate.size = aggregate.size.saturating_add(entry.size);
            aggregate.packed = match (aggregate.packed, entry.packed_size) {
                (Some(total), Some(size)) => Some(total.saturating_add(size)),
                _ => None,
            };
            if aggregate.volume != Some(entry.volume) {
                aggregate.volume = None;
            }
        } else if !relative.is_empty() {
            files.push(entry_row(entry));
        }
    }
    let mut rows = folders
        .into_iter()
        .map(|(name, aggregate)| {
            let full_path = join_path(directory, &name);
            BrowserRow {
                key: format!("d:{full_path}"),
                selection: BrowserSelection::Directory(full_path.clone()),
                kind: BrowserRowKind::Directory,
                name,
                full_path,
                size: aggregate.size,
                packed_size: aggregate.packed,
                compression: None,
                volume: aggregate.volume,
                detail: format!("{} 个文件", aggregate.count),
            }
        })
        .collect::<Vec<_>>();
    rows.extend(files);
    sort_browser_rows(&mut rows, tab.sort);
    rows
}

fn sort_browser_rows(rows: &mut [BrowserRow], sort: BrowserSort) {
    rows.sort_by(|left, right| {
        match (&left.kind, &right.kind) {
            (BrowserRowKind::Directory, BrowserRowKind::File) => return Ordering::Less,
            (BrowserRowKind::File, BrowserRowKind::Directory) => return Ordering::Greater,
            _ => {}
        }

        let primary = match sort.key {
            BrowserSortKey::Name => compare_text(&left.name, &right.name),
            BrowserSortKey::Type => compare_text(
                left.compression.map(compression_label).unwrap_or("目录"),
                right.compression.map(compression_label).unwrap_or("目录"),
            ),
            BrowserSortKey::Size => left.size.cmp(&right.size),
        };
        sort.apply(primary)
            .then_with(|| compare_text(&left.name, &right.name))
            .then_with(|| left.key.cmp(&right.key))
    });
}

fn compare_text(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
        .then_with(|| left.cmp(right))
}

fn entry_row(entry: &DraftEntry) -> BrowserRow {
    BrowserRow {
        key: format!("e:{}", entry.id),
        selection: BrowserSelection::Entry(entry.id),
        kind: BrowserRowKind::File,
        name: file_name(&entry.path).to_string(),
        full_path: entry.path.clone(),
        size: entry.size,
        packed_size: entry.packed_size,
        compression: Some(entry.compression),
        volume: Some(entry.volume),
        detail: format!(
            "{} · Volume {}",
            compression_label(entry.compression),
            entry.volume
        ),
    }
}

fn all_directories(entries: &[DraftEntry]) -> Vec<String> {
    let mut directories = BTreeSet::new();
    for entry in entries {
        let components = path_components(&entry.path);
        for length in 1..components.len() {
            directories.insert(components[..length].join("/"));
        }
    }
    directories.into_iter().collect()
}

fn entries_in_directory(entries: &[DraftEntry], directory: &str) -> Vec<DraftEntry> {
    let directory = directory.trim_matches('/');
    if directory.is_empty() {
        return entries.to_vec();
    }
    let prefix = format!("{directory}/");
    entries
        .iter()
        .filter(|entry| entry.path.trim_matches('/').starts_with(&prefix))
        .cloned()
        .collect()
}

fn open_selection(selection: BrowserSelection, signals: WorkspaceSignals) {
    match selection {
        BrowserSelection::Directory(path) => navigate_to(signals.tabs, signals.active_tab_id, path),
        BrowserSelection::Entry(id) => select_item(
            signals.tabs,
            signals.active_tab_id,
            BrowserSelection::Entry(id),
        ),
    }
}

fn navigate_to(tabs: Signal<Vec<ArchiveTab>>, active_tab_id: Signal<Option<u64>>, path: String) {
    update_active_tab(tabs, active_tab_id, |tab| {
        tab.directory = path;
        tab.selection = None;
        tab.query.clear();
        tab.row_limit = ROW_PAGE_SIZE;
    });
}

fn navigate_up(tabs: Signal<Vec<ArchiveTab>>, active_tab_id: Signal<Option<u64>>) {
    update_active_tab(tabs, active_tab_id, |tab| {
        tab.directory = parent_directory(&tab.directory);
        tab.selection = None;
        tab.query.clear();
        tab.row_limit = ROW_PAGE_SIZE;
    });
}

fn select_item(
    tabs: Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
    selection: BrowserSelection,
) {
    update_active_tab(tabs, active_tab_id, |tab| tab.selection = Some(selection));
}

fn update_active_tab(
    mut tabs: Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
    update: impl FnOnce(&mut ArchiveTab),
) {
    let Some(id) = active_tab_id() else {
        return;
    };
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == id) {
        update(tab);
    }
}

fn active_tab(
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<ArchiveTab> {
    let id = active_tab_id()?;
    tabs.peek().iter().find(|tab| tab.id == id).cloned()
}

fn active_selection(
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<BrowserSelection> {
    active_tab(tabs, active_tab_id).and_then(|tab| tab.selection)
}

fn request_close_tab(
    tabs: Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
    id: u64,
    mut dialog: Signal<Option<EditDialog>>,
) {
    if let Some(tab) = tabs.peek().iter().find(|tab| tab.id == id)
        && tab.dirty
    {
        dialog.set(Some(EditDialog::CloseDirty {
            tab_id: id,
            name: tab.name.clone(),
        }));
    } else {
        close_tab(tabs, active_tab_id, id);
    }
}

fn close_tab(mut tabs: Signal<Vec<ArchiveTab>>, mut active_tab_id: Signal<Option<u64>>, id: u64) {
    let mut tabs = tabs.write();
    let Some(index) = tabs.iter().position(|tab| tab.id == id) else {
        return;
    };
    let was_active = active_tab_id() == Some(id);
    let session = tabs.remove(index).session_id;
    if was_active {
        active_tab_id.set(
            tabs.get(index)
                .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
                .map(|tab| tab.id),
        );
    }
    drop(tabs);
    if let Some(session_id) = session {
        spawn(async move { platform::close_archive(session_id).await });
    }
}

fn target_name(
    target: &BrowserSelection,
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<String> {
    let tab = active_tab(tabs, active_tab_id)?;
    match target {
        BrowserSelection::Directory(path) => Some(file_name(path).to_string()),
        BrowserSelection::Entry(id) => tab
            .entries
            .iter()
            .find(|entry| entry.id == *id)
            .map(|entry| file_name(&entry.path).to_string()),
    }
}

fn set_status(mut status: Signal<AppStatus>, value: AppStatus) {
    let level = match value.tone {
        StatusTone::Neutral | StatusTone::Success => "INFO",
        StatusTone::Warning => "WARN",
        StatusTone::Error => "ERROR",
    };
    if !value.message.is_empty() {
        push_application_log("DZIP", level, "ARCHIVE", &value.message);
    }
    status.set(value);
}

fn is_main_archive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".dz") || name.ends_with(".dzip")
}

fn is_volume_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.len() >= 3 && extension.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn path_components(path: &str) -> Vec<String> {
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn path_depth(path: &str) -> usize {
    path_components(path).len()
}

fn parent_directory(path: &str) -> String {
    path.trim_matches('/')
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn file_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

fn replace_file_name(path: &str, name: &str) -> String {
    let parent = parent_directory(&path.replace('\\', "/"));
    join_path(&parent, name)
}

fn join_path(directory: &str, name: &str) -> String {
    let directory = directory.trim_matches(['/', '\\']);
    let name = name.trim_matches(['/', '\\']);
    if directory.is_empty() {
        name.to_string()
    } else if name.is_empty() {
        directory.to_string()
    } else {
        format!("{directory}/{name}")
    }
}

fn breadcrumbs(path: &str) -> Vec<(String, String)> {
    let mut current = String::new();
    path_components(path)
        .into_iter()
        .map(|part| {
            current = join_path(&current, &part);
            (part, current.clone())
        })
        .collect()
}

fn compression_label(compression: Compression) -> &'static str {
    match compression {
        Compression::Dz => "DZ",
        Compression::Zlib => "Zlib",
        Compression::Bzip => "BZip",
        Compression::Lzma => "LZMA",
        Compression::Copy => "Copy",
        Compression::Zero => "Zero",
    }
}

fn compression_code(compression: Compression) -> &'static str {
    compression.name()
}

fn parse_compression(value: &str) -> Option<Compression> {
    match value {
        "dz" => Some(Compression::Dz),
        "zlib" => Some(Compression::Zlib),
        "bzip" => Some(Compression::Bzip),
        "lzma" => Some(Compression::Lzma),
        "copy" => Some(Compression::Copy),
        "zero" => Some(Compression::Zero),
        _ => None,
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0_usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn format_optional_bytes(bytes: Option<u64>) -> String {
    bytes
        .map(format_bytes)
        .unwrap_or_else(|| "重建时计算".to_string())
}

fn tab_class(active: bool, dirty: bool) -> &'static str {
    match (active, dirty) {
        (true, true) => "dzip-tab ui-document-tab is-active is-dirty",
        (true, false) => "dzip-tab ui-document-tab is-active",
        (false, true) => "dzip-tab ui-document-tab is-dirty",
        (false, false) => "dzip-tab ui-document-tab",
    }
}

#[component]
fn Glyph(#[props(into)] name: String) -> Element {
    let path = match name.as_str() {
        "archive" => "M4 7h16v13H4z M3 3h18v4H3z M9 11h6",
        "tree" => "M6 3v12 M6 7h6 M6 13h6 M12 5h6v4h-6z M12 11h6v4h-6z M6 17h6v4H6z",
        "open" => "M3 7h6l2 2h10l-2 10H5z M5 7V4h5l2 2h6v3",
        "up" => "M12 19V5 M6 11l6-6 6 6",
        "extract" => "M12 3v12 M7 10l5 5 5-5 M4 19h16",
        "add-file" => "M6 3h8l4 4v14H6z M14 3v5h5 M12 11v6 M9 14h6",
        "save" => "M5 3h12l3 3v15H4V3z M8 3v6h8V3 M8 14h8v7H8z",
        "more" => "M5 12h.01 M12 12h.01 M19 12h.01",
        "inspector" => "M4 4h16v16H4z M14 4v16 M7 8h4 M7 12h4 M7 16h3",
        "close" => "M6 6l12 12 M18 6 6 18",
        "plus" | "new" => "M12 5v14 M5 12h14",
        "file" => "M6 2h8l4 4v16H6z M14 2v5h5",
        "folder" => "M3 6h7l2 2h9v11H3z",
        "size" => "M5 19V9 M12 19V5 M19 19v-7",
        "packed" => "M4 7l8-4 8 4-8 4z M4 7v10l8 4 8-4V7 M12 11v10",
        "volumes" => "M5 4h14v5H5z M5 10h14v5H5z M5 16h14v4H5z",
        "search" => "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14z M16 16l5 5",
        "replace" => "M4 8h12 M13 5l3 3-3 3 M20 16H8 M11 13l-3 3 3 3",
        "rename" => "M4 20h4L19 9l-4-4L4 16z M13 7l4 4",
        "delete" => "M4 7h16 M9 7V4h6v3 M7 7l1 14h8l1-14 M10 11v6 M14 11v6",
        "validate" => "M5 12l4 4L19 6",
        "settings" => "M4 7h10 M18 7h2 M4 17h2 M10 17h10 M14 4v6 M6 14v6",
        "warning" => "M12 3 2 21h20z M12 9v5 M12 18h.01",
        _ => "M4 12h16",
    };
    rsx! {
        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "{path}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_helpers_use_portable_archive_separators() {
        assert_eq!(join_path("Data", "Images/logo.png"), "Data/Images/logo.png");
        assert_eq!(parent_directory("Data/Images"), "Data");
        assert_eq!(replace_file_name("Data/old.bin", "new.bin"), "Data/new.bin");
        assert_eq!(breadcrumbs("Data/Images")[1].1, "Data/Images");
    }

    #[test]
    fn dzip_input_classification_is_strict() {
        assert!(is_main_archive_name("GAME.DZ"));
        assert!(is_main_archive_name("game.dzip"));
        assert!(is_volume_name("game.001"));
        assert!(!is_volume_name("game.txt"));
    }

    #[test]
    fn browser_sort_keeps_directories_first_and_uses_raw_sizes() {
        let mut tab = ArchiveTab::empty(1);
        tab.entries = std::sync::Arc::new(vec![
            DraftEntry::replacement(1, "z-small.bin".to_string(), vec![0], Compression::Dz),
            DraftEntry::replacement(
                2,
                "a-large.bin".to_string(),
                vec![0; 2_048],
                Compression::Dz,
            ),
            DraftEntry::replacement(
                3,
                "folder/item.bin".to_string(),
                vec![0; 16],
                Compression::Dz,
            ),
        ]);
        tab.sort = BrowserSort {
            key: BrowserSortKey::Size,
            direction: BrowserSortDirection::Descending,
        };

        let rows = browser_rows(&tab);
        assert_eq!(rows[0].kind, BrowserRowKind::Directory);
        assert_eq!(rows[1].name, "a-large.bin");
        assert_eq!(rows[2].name, "z-small.bin");
    }

    #[test]
    fn browser_sort_toggles_active_key_direction() {
        let sort = BrowserSort::default().toggled(BrowserSortKey::Name);
        assert_eq!(sort.direction, BrowserSortDirection::Descending);
        let sort = sort.toggled(BrowserSortKey::Type);
        assert_eq!(sort.key, BrowserSortKey::Type);
        assert_eq!(sort.direction, BrowserSortDirection::Ascending);
    }
}
