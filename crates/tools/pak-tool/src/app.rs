use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use pak_archive::{
    CompressionLevel, EncodeOptions, FlatProfile, MetadataConversion, PakArchive, PakCompression,
    PakEntry, PakEntryKind, PakFormat, PakTimestamp, PathSeparator, ZipCompression,
};
use toolkit_ui::{
    DropIndicator, InlineNotice, ToolPage, ToolPageToolbar, WorkspaceCard, push_application_log,
};

use crate::{PakNewtonOpenRequest, PakRtonOpenRequest, PakWemOpenRequest, platform};

const PAK_PAGE_CSS: Asset = asset!("/assets/pak/page.css");
const PAK_FILE_ACCEPT: &str = ".pak,.zip,application/zip,application/octet-stream";
const ROW_PAGE_SIZE: usize = 240;

type ArchivePath = Vec<String>;

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
    Directory(ArchivePath),
    Entry(usize),
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
    archive: Arc<PakArchive>,
    directory: ArchivePath,
    selection: Option<BrowserSelection>,
    query: String,
    sort: BrowserSort,
    row_limit: usize,
    dirty: bool,
}

impl PartialEq for ArchiveTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && Arc::ptr_eq(&self.archive, &other.archive)
            && self.directory == other.directory
            && self.selection == other.selection
            && self.query == other.query
            && self.sort == other.sort
            && self.row_limit == other.row_limit
            && self.dirty == other.dirty
    }
}

impl ArchiveTab {
    fn opened(id: u64, name: String, archive: PakArchive) -> Self {
        Self {
            id,
            name,
            archive: Arc::new(archive),
            directory: Vec::new(),
            selection: None,
            query: String::new(),
            sort: BrowserSort::default(),
            row_limit: ROW_PAGE_SIZE,
            dirty: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrowserRow {
    key: String,
    selection: BrowserSelection,
    name: String,
    full_path: String,
    kind: PakEntryKind,
    size: u64,
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
    NewFolder {
        value: String,
    },
    Rename {
        target: BrowserSelection,
        value: String,
    },
    Delete {
        target: BrowserSelection,
        label: String,
    },
    ArchiveProperties {
        format: String,
        compression_level: String,
        separator: String,
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
pub fn PakArchivePage(
    on_open_newton: Option<EventHandler<PakNewtonOpenRequest>>,
    on_open_rton: Option<EventHandler<PakRtonOpenRequest>>,
    on_open_wem: Option<EventHandler<PakWemOpenRequest>>,
) -> Element {
    let tabs = use_signal(Vec::<ArchiveTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(|| AppStatus::new("打开或拖入一个 PAK 归档", StatusTone::Neutral));
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
    toolkit_ui::use_tool_open(toolkit_ui::ToolKind::Pak, busy, move |files| {
        let files = files
            .into_iter()
            .map(toolkit_ui::ToolFile::into_file_data)
            .collect();
        load_archive_files(files, signals, next_tab_id);
    });
    let tabs_snapshot = tabs();
    let active_id_snapshot = active_tab_id();
    let active_tab_snapshot = active_id_snapshot
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: PAK_PAGE_CSS }
        div {
            class: if dragging() { "pak-page-host is-dragging" } else { "pak-page-host" },
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
                    load_archive_files(files, signals, next_tab_id);
                }
            },
            onclick: move |_| context_menu.set(None),
            ToolPage { namespace: "pak", class: "pak-page",
                ToolPageToolbar {
                    class: "pak-page-toolbar",
                    actions: rsx! {
                        ArchiveToolbar {
                            tab: active_tab_snapshot.clone(),
                            busy: busy(),
                            tree_visible: tree_visible(),
                            inspector_visible: inspector_visible(),
                            on_files: move |files| load_archive_files(files, signals, next_tab_id),
                            on_up: move |_| navigate_up(tabs, active_tab_id),
                            on_extract: move |_| extract_current(signals),
                            on_add_files: move |files| import_files(files, signals),
                            on_new_folder: move |_| dialog.set(Some(EditDialog::NewFolder { value: String::new() })),
                            on_save: move |_| save_active_archive(signals),
                            on_validate: move |_| validate_active(signals),
                            on_properties: move |_| {
                                if let Some((_id, _name, archive)) = active_archive(&signals.tabs, signals.active_tab_id) {
                                    let options = archive.options();
                                    dialog.set(Some(EditDialog::ArchiveProperties {
                                        format: format_code(options.format).to_string(),
                                        compression_level: compression_level_code(options.compression_level).to_string(),
                                        separator: separator_code(options.path_separator).to_string(),
                                    }));
                                }
                            },
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

                WorkspaceCard { class: "pak-workspace-card", aria_label: "PAK Archive",
                    if let Some(tab) = active_tab_snapshot.clone() {
                        ArchiveWorkspace {
                            tab,
                            signals,
                            tree_visible: tree_visible(),
                            inspector_visible: inspector_visible(),
                            context_menu,
                            dialog,
                            on_open_newton,
                            on_open_rton,
                            on_open_wem,
                        }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| load_archive_files(files, signals, next_tab_id),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "pak-status-notice",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "拖放一个或多个 PAK 以打开".to_string() }
                    }
                    if busy() {
                        div { class: "pak-busy-indicator", role: "status",
                            span {}
                            "正在处理归档…"
                        }
                    }
                }
            }
            if let Some(menu) = context_menu() {
                ArchiveContextMenu {
                    menu,
                    tab: active_tab_snapshot.clone(),
                    signals,
                    on_open_newton,
                    on_open_rton,
                    on_open_wem,
                    on_close: move |_| context_menu.set(None),
                    on_rename: move |target| {
                        if let Some(value) = target_name(&target, &tabs, active_tab_id) {
                            dialog.set(Some(EditDialog::Rename { target, value }));
                        }
                    },
                    on_delete: move |target| {
                        let label = target_name(&target, &tabs, active_tab_id).unwrap_or_else(|| "所选项目".to_string());
                        dialog.set(Some(EditDialog::Delete { target, label }));
                    },
                    on_new_folder: move |_| dialog.set(Some(EditDialog::NewFolder { value: String::new() })),
                }
            }
            if let Some(current_dialog) = dialog() {
                EditDialogView {
                    dialog: current_dialog,
                    signals,
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
    on_up: EventHandler<()>,
    on_extract: EventHandler<()>,
    on_add_files: EventHandler<Vec<FileData>>,
    on_new_folder: EventHandler<()>,
    on_save: EventHandler<()>,
    on_validate: EventHandler<()>,
    on_properties: EventHandler<()>,
    on_toggle_tree: EventHandler<()>,
    on_toggle_inspector: EventHandler<()>,
) -> Element {
    let has_archive = tab.is_some();
    let can_up = tab.as_ref().is_some_and(|tab| !tab.directory.is_empty());
    let dirty = tab.as_ref().is_some_and(|tab| tab.dirty);
    rsx! {
        div { class: "ui-island ui-tool-page-actions pak-page-actions",
            button {
                class: if tree_visible { "pak-icon-button is-active" } else { "pak-icon-button" },
                title: "目录",
                aria_label: "显示或隐藏目录",
                disabled: !has_archive,
                onclick: move |_| on_toggle_tree.call(()),
                Glyph { name: "tree" }
            }
            label { class: if busy { "pak-icon-button primary is-disabled" } else { "pak-icon-button primary" }, title: "打开 PAK", aria_label: "打开 PAK",
                Glyph { name: "open" }
                input { class: "pak-file-input", r#type: "file", accept: PAK_FILE_ACCEPT, multiple: true, disabled: busy,
                    onchange: move |event| {
                        let files = event.files();
                        if !files.is_empty() { on_files.call(files); }
                    }
                }
            }
            button { class: "pak-icon-button", title: "返回上级", aria_label: "返回上级", disabled: !can_up || busy, onclick: move |_| on_up.call(()), Glyph { name: "up" } }
            button { class: "pak-icon-button", title: "提取", aria_label: "提取", disabled: !has_archive || busy, onclick: move |_| on_extract.call(()), Glyph { name: "extract" } }
            span { class: "pak-toolbar-divider" }
            label { class: if has_archive && !busy { "pak-icon-button" } else { "pak-icon-button is-disabled" }, title: "添加文件", aria_label: "添加文件",
                Glyph { name: "add-file" }
                input { class: "pak-file-input", r#type: "file", multiple: true, disabled: !has_archive || busy,
                    onchange: move |event| {
                        let files = event.files();
                        if !files.is_empty() { on_add_files.call(files); }
                    }
                }
            }
            button { class: "pak-icon-button", title: "新建文件夹", aria_label: "新建文件夹", disabled: !has_archive || busy, onclick: move |_| on_new_folder.call(()), Glyph { name: "folder-add" } }
            button { class: if dirty { "pak-icon-button has-changes" } else { "pak-icon-button" }, title: "另存为", aria_label: "另存为", disabled: !has_archive || busy, onclick: move |_| on_save.call(()), Glyph { name: "save" } if dirty { span { class: "pak-unsaved-dot" } } }
            span { class: "pak-toolbar-spacer" }
            if let Some(tab) = tab {
                span { class: "pak-format-pill", "{format_label(tab.archive.options().format)}" }
            }
            button { class: "pak-icon-button", title: "校验归档", aria_label: "校验归档", disabled: !has_archive, onclick: move |_| on_validate.call(()), Glyph { name: "validate" } }
            button { class: "pak-icon-button", title: "归档属性", aria_label: "归档属性", disabled: !has_archive, onclick: move |_| on_properties.call(()), Glyph { name: "properties" } }
            button { class: if inspector_visible { "pak-icon-button is-active" } else { "pak-icon-button" }, title: "属性", aria_label: "显示或隐藏属性", disabled: !has_archive, onclick: move |_| on_toggle_inspector.call(()), Glyph { name: "inspector" } }
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
        nav { class: "pak-tab-strip ui-document-tab-strip",
            div { class: "pak-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 PAK",
                for tab in tabs {
                    div { key: "{tab.id}", class: tab_class(Some(tab.id) == active_tab_id, tab.dirty),
                        button { class: "pak-tab-select ui-document-tab-label", role: "tab", aria_selected: Some(tab.id) == active_tab_id, title: "{tab.name}", disabled: busy,
                            onclick: { let id = tab.id; move |_| on_activate.call(id) },
                            span { class: "pak-tab-dot ui-document-tab-dot" }
                            span { class: "pak-tab-name ui-document-tab-name", "{tab.name}" }
                        }
                        button { class: "pak-tab-close ui-document-tab-close", title: "关闭 {tab.name}", aria_label: "关闭 {tab.name}", disabled: busy,
                            onclick: { let id = tab.id; move |event| { event.stop_propagation(); on_close.call(id); } },
                            Glyph { name: "close" }
                        }
                    }
                }
                label { class: if busy { "pak-tab-add ui-document-new-tab is-disabled" } else { "pak-tab-add ui-document-new-tab" }, title: "添加 PAK", aria_label: "添加 PAK",
                    Glyph { name: "add" }
                    input { class: "pak-file-input", r#type: "file", accept: PAK_FILE_ACCEPT, multiple: true, disabled: busy,
                        onchange: move |event| { let files = event.files(); if !files.is_empty() { on_files.call(files); } }
                    }
                }
            }
        }
    }
}

#[component]
fn EmptyWorkspace(busy: bool, on_files: EventHandler<Vec<FileData>>) -> Element {
    rsx! {
        div { class: "pak-empty-workspace",
            div { class: "pak-empty-mark", Glyph { name: "archive" } }
            span { class: "pak-empty-kicker", "POPCAP ARCHIVE WORKSPACE" }
            h1 { "打开 PopCap PAK" }
            p { "浏览 PC XOR、Plain、Xbox 360 与 TV ZIP 归档，编辑文件后以校验过的格式重新保存。" }
            label { class: "pak-open-button", Glyph { name: "open" } "选择 PAK"
                input { class: "pak-file-input", r#type: "file", accept: PAK_FILE_ACCEPT, multiple: true, disabled: busy,
                    onchange: move |event| { let files = event.files(); if !files.is_empty() { on_files.call(files); } }
                }
            }
            span { class: "pak-empty-drop-hint", "也可以把一个或多个 .pak / TV ZIP 文件拖到这里" }
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
    on_open_newton: Option<EventHandler<PakNewtonOpenRequest>>,
    on_open_rton: Option<EventHandler<PakRtonOpenRequest>>,
    on_open_wem: Option<EventHandler<PakWemOpenRequest>>,
) -> Element {
    let mut rows = browser_rows(&tab);
    let total_rows = rows.len();
    rows.truncate(tab.row_limit);
    let visible_rows = rows.len();
    let directories = all_directories(&tab.archive);
    let total_bytes = tab
        .archive
        .entries()
        .iter()
        .filter(|entry| entry.kind() != PakEntryKind::Directory)
        .map(|entry| entry.data().len() as u64)
        .sum::<u64>();
    let selected = tab.selection.clone();
    let grid_class = match (tree_visible, inspector_visible) {
        (true, true) => "pak-browser-grid",
        (true, false) => "pak-browser-grid no-inspector",
        (false, true) => "pak-browser-grid no-tree",
        (false, false) => "pak-browser-grid no-tree no-inspector",
    };

    rsx! {
        div { class: "pak-document",
            div { class: "pak-summary-grid",
                SummaryCard { label: "格式", value: format_label(tab.archive.options().format).to_string(), glyph: "archive" }
                SummaryCard { label: "文件", value: file_count(&tab.archive).to_string(), glyph: "file" }
                SummaryCard { label: "目录", value: directories.len().to_string(), glyph: "folder" }
                SummaryCard { label: "原始大小", value: format_bytes(total_bytes), glyph: "size" }
            }
            div { class: "{grid_class}",
                if tree_visible {
                    DirectoryPanel { tab: tab.clone(), directories, signals }
                }
                section { class: "pak-list-panel",
                    div { class: "pak-list-toolbar",
                        div { class: "pak-breadcrumb", aria_label: "当前位置",
                            button { class: if tab.directory.is_empty() { "is-current" } else { "" }, onclick: move |_| navigate_to(signals.tabs, signals.active_tab_id, Vec::new()), Glyph { name: "archive" } "{tab.name}" }
                            for (index, component) in tab.directory.iter().enumerate() {
                                span { "/" }
                                button { class: if index + 1 == tab.directory.len() { "is-current" } else { "" },
                                    onclick: { let path = tab.directory[..=index].to_vec(); move |_| navigate_to(signals.tabs, signals.active_tab_id, path.clone()) },
                                    "{component}"
                                }
                            }
                        }
                        label { class: "pak-search", Glyph { name: "search" }
                            input { value: "{tab.query}", placeholder: "搜索归档", aria_label: "搜索归档",
                                oninput: move |event| update_active_tab(signals.tabs, signals.active_tab_id, |tab| { tab.query = event.value(); tab.selection = None; tab.row_limit = ROW_PAGE_SIZE; })
                            }
                        }
                    }
                    div { class: "pak-table-head",
                        SortableTableHeader {
                            label: "名称",
                            sort_key: BrowserSortKey::Name,
                            sort: tab.sort,
                            on_sort: move |key| update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                tab.sort = tab.sort.toggled(key);
                            }),
                        }
                        SortableTableHeader {
                            label: "类型",
                            sort_key: BrowserSortKey::Type,
                            sort: tab.sort,
                            on_sort: move |key| update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                tab.sort = tab.sort.toggled(key);
                            }),
                        }
                        SortableTableHeader {
                            label: "大小",
                            class: "pak-sort-header--numeric",
                            sort_key: BrowserSortKey::Size,
                            sort: tab.sort,
                            on_sort: move |key| update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                tab.sort = tab.sort.toggled(key);
                            }),
                        }
                        span { "属性" }
                    }
                    div { class: "pak-table-body",
                        oncontextmenu: move |event| {
                            event.prevent_default();
                            let point = event.client_coordinates();
                            context_menu.set(Some(ContextMenuState { target: None, x: point.x, y: point.y }));
                        },
                        if rows.is_empty() {
                            div { class: "pak-list-empty", Glyph { name: "folder" } strong { if tab.query.is_empty() { "此目录为空" } else { "没有匹配项" } } small { if tab.query.is_empty() { "可添加文件或新建文件夹" } else { "尝试其他搜索关键词" } } }
                        }
                        for row in rows {
                            ArchiveRow {
                                key: "{row.key}",
                                row: row.clone(),
                                selected: selected.as_ref() == Some(&row.selection),
                                on_select: move |selection| select_item(signals.tabs, signals.active_tab_id, selection),
                                on_open: move |selection| open_selection(selection, signals, on_open_newton, on_open_rton, on_open_wem),
                                on_context: move |menu| context_menu.set(Some(menu)),
                            }
                        }
                        if visible_rows < total_rows {
                            button {
                                class: "pak-load-more",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                                        tab.row_limit = tab.row_limit.saturating_add(ROW_PAGE_SIZE);
                                    });
                                },
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
                    }
                }
            }
        }
    }
}

#[component]
fn SummaryCard(label: &'static str, value: String, glyph: &'static str) -> Element {
    rsx! { article { class: "pak-summary-card", span { class: "pak-summary-icon", Glyph { name: glyph } } div { small { "{label}" } strong { "{value}" } } } }
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
        format!("pak-sort-header {class} is-active")
    } else {
        format!("pak-sort-header {class}")
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
                class: if active { "pak-sort-indicator is-active" } else { "pak-sort-indicator" },
                aria_hidden: "true",
                {sort.indicator(sort_key)}
            }
        }
    }
}

#[component]
fn DirectoryPanel(
    tab: ArchiveTab,
    directories: Vec<ArchivePath>,
    signals: WorkspaceSignals,
) -> Element {
    rsx! {
        aside { class: "pak-tree-panel",
            div { class: "pak-panel-heading", span { Glyph { name: "tree" } "目录" } small { "{directories.len()}" } }
            nav { class: "pak-tree-list", aria_label: "PAK 目录",
                button { class: if tab.directory.is_empty() { "pak-tree-row is-active" } else { "pak-tree-row" }, onclick: move |_| navigate_to(signals.tabs, signals.active_tab_id, Vec::new()),
                    span { class: "pak-tree-glyph", Glyph { name: "archive" } }
                    span { class: "pak-tree-name", "归档根目录" }
                }
                for path in directories {
                    button { class: if tab.directory == path { "pak-tree-row is-active" } else { "pak-tree-row" }, style: "--depth: {path.len()};", title: "{archive_path_label(&path)}",
                        onclick: { let path = path.clone(); move |_| navigate_to(signals.tabs, signals.active_tab_id, path.clone()) },
                        span { class: "pak-tree-glyph", Glyph { name: "folder" } }
                        span { class: "pak-tree-name", "{path.last().cloned().unwrap_or_default()}" }
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
    rsx! {
        div {
            class: if selected { "pak-table-row is-selected" } else { "pak-table-row" },
            role: "row",
            tabindex: "0",
            onclick: { let selection = selection.clone(); move |event| { event.stop_propagation(); on_select.call(selection.clone()); } },
            ondoubleclick: { let selection = selection.clone(); move |event| { event.stop_propagation(); on_open.call(selection.clone()); } },
            oncontextmenu: { let selection = selection.clone(); move |event| { event.prevent_default(); event.stop_propagation(); let point = event.client_coordinates(); on_context.call(ContextMenuState { target: Some(selection.clone()), x: point.x, y: point.y }); } },
            div { class: "pak-row-name", span { class: "pak-row-icon pak-row-icon--{kind_class(row.kind)}", Glyph { name: kind_glyph(row.kind) } } div { strong { "{row.name}" } small { "{row.full_path}" } } }
            span { class: "pak-row-kind", "{kind_label(row.kind)}" }
            span { class: "pak-row-size", "{format_bytes(row.size)}" }
            span { class: "pak-row-detail", "{row.detail}" }
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
) -> Element {
    let selected_entry = tab
        .selection
        .as_ref()
        .and_then(|selection| match selection {
            BrowserSelection::Entry(index) => {
                tab.archive.entry_at(*index).map(|entry| (*index, entry))
            }
            BrowserSelection::Directory(_) => None,
        });
    rsx! {
        aside { class: "pak-inspector-panel",
            div { class: "pak-panel-heading", span { Glyph { name: "inspector" } "属性" } }
            if let Some((index, entry)) = selected_entry {
                div { class: "pak-inspector-hero", span { class: "pak-inspector-icon", Glyph { name: kind_glyph(entry.kind()) } } div { strong { "{file_name(entry.path().to_string_lossy().as_ref())}" } small { "条目 #{index}" } } }
                dl { class: "pak-properties",
                    div { dt { "路径" } dd { title: "{entry.path()}", "{entry.path()}" } }
                    div { dt { "类型" } dd { "{kind_label(entry.kind())}" } }
                    div { dt { "大小" } dd { "{format_bytes(entry.data().len() as u64)}" } }
                    div { dt { "时间" } dd { "{timestamp_label(entry.timestamp())}" } }
                    if let Some(zip) = entry.zip_metadata() {
                        div { dt { "压缩" } dd { "{zip_compression_label(zip.compression())}" } }
                        div { dt { "权限" } dd { "{unix_mode_label(zip.unix_mode())}" } }
                    }
                }
                div { class: "pak-inspector-actions",
                    button { onclick: move |_| on_extract.call(()), Glyph { name: "extract" } "提取" }
                    if entry.kind() != PakEntryKind::Directory {
                        label { Glyph { name: "replace" } "替换"
                            input { class: "pak-file-input", r#type: "file", onchange: move |event| { let files = event.files(); if !files.is_empty() { on_replace.call(files); } } }
                        }
                    }
                    button { onclick: { let target = BrowserSelection::Entry(index); move |_| on_rename.call(target.clone()) }, Glyph { name: "rename" } "重命名" }
                    button { class: "danger", onclick: { let target = BrowserSelection::Entry(index); move |_| on_delete.call(target.clone()) }, Glyph { name: "delete" } "删除" }
                }
            } else if let Some(BrowserSelection::Directory(path)) = tab.selection.as_ref() {
                div { class: "pak-inspector-hero", span { class: "pak-inspector-icon", Glyph { name: "folder" } } div { strong { "{directory_name(path)}" } small { "目录" } } }
                dl { class: "pak-properties",
                    div { dt { "路径" } dd { "{archive_path_label(path)}" } }
                    div { dt { "条目" } dd { "{indices_for_directory(&tab.archive, path).len()}" } }
                }
                div { class: "pak-inspector-actions",
                    button { onclick: move |_| on_extract.call(()), Glyph { name: "extract" } "提取" }
                    button { onclick: { let target = BrowserSelection::Directory(path.clone()); move |_| on_rename.call(target.clone()) }, Glyph { name: "rename" } "重命名" }
                    button { class: "danger", onclick: { let target = BrowserSelection::Directory(path.clone()); move |_| on_delete.call(target.clone()) }, Glyph { name: "delete" } "删除" }
                }
            } else {
                div { class: "pak-inspector-overview",
                    span { class: "pak-inspector-icon", Glyph { name: "archive" } }
                    h3 { "{tab.name}" }
                    p { "{format_label(tab.archive.options().format)}" }
                    dl { class: "pak-properties",
                        div { dt { "文件" } dd { "{file_count(&tab.archive)}" } }
                        div { dt { "条目" } dd { "{tab.archive.len()}" } }
                        div { dt { "源格式" } dd { "{source_format_label(&tab.archive)}" } }
                    }
                    button { class: "pak-wide-action", onclick: move |_| validate_active(signals), Glyph { name: "validate" } "校验归档" }
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
    on_open_newton: Option<EventHandler<PakNewtonOpenRequest>>,
    on_open_rton: Option<EventHandler<PakRtonOpenRequest>>,
    on_open_wem: Option<EventHandler<PakWemOpenRequest>>,
    on_close: EventHandler<()>,
    on_rename: EventHandler<BrowserSelection>,
    on_delete: EventHandler<BrowserSelection>,
    on_new_folder: EventHandler<()>,
) -> Element {
    let target = menu.target.clone();
    let can_edit = target.is_some();
    let can_open = target.is_some();
    rsx! {
        div { class: "pak-context-menu", style: "left: {menu.x}px; top: {menu.y}px;", role: "menu", onclick: move |event| event.stop_propagation(),
            if can_open {
                button { onclick: { let target = target.clone(); move |_| { if let Some(target) = target.clone() { open_selection(target, signals, on_open_newton, on_open_rton, on_open_wem); } on_close.call(()); } }, Glyph { name: "open" } "打开" }
            }
            button { onclick: move |_| { extract_current(signals); on_close.call(()); }, disabled: tab.is_none(), Glyph { name: "extract" } "提取" }
            div { class: "pak-context-separator" }
            button { onclick: move |_| { on_new_folder.call(()); on_close.call(()); }, disabled: tab.is_none(), Glyph { name: "folder-add" } "新建文件夹" }
            button { disabled: !can_edit, onclick: { let target = target.clone(); move |_| { if let Some(target) = target.clone() { on_rename.call(target); } on_close.call(()); } }, Glyph { name: "rename" } "重命名" }
            button { class: "danger", disabled: !can_edit, onclick: { let target = target.clone(); move |_| { if let Some(target) = target.clone() { on_delete.call(target); } on_close.call(()); } }, Glyph { name: "delete" } "删除" }
        }
    }
}

#[component]
fn EditDialogView(
    dialog: EditDialog,
    signals: WorkspaceSignals,
    on_cancel: EventHandler<()>,
    on_change: EventHandler<EditDialog>,
    on_confirm: EventHandler<EditDialog>,
) -> Element {
    let (title, confirm_label, danger) = match &dialog {
        EditDialog::NewFolder { .. } => ("新建文件夹", "创建", false),
        EditDialog::Rename { .. } => ("重命名", "应用", false),
        EditDialog::Delete { .. } => ("删除项目", "删除", true),
        EditDialog::ArchiveProperties { .. } => ("归档属性", "应用", false),
        EditDialog::CloseDirty { .. } => ("尚未保存", "放弃更改", true),
    };
    rsx! {
        div { class: "pak-dialog-layer", role: "presentation",
            button { class: "pak-dialog-backdrop", aria_label: "关闭", onclick: move |_| on_cancel.call(()) }
            section { class: "pak-dialog", role: "dialog", aria_modal: "true", aria_label: "{title}",
                header { div { span { class: "pak-dialog-kicker", "PAK ARCHIVE" } h2 { "{title}" } } button { aria_label: "关闭", onclick: move |_| on_cancel.call(()), Glyph { name: "close" } } }
                div { class: "pak-dialog-body",
                    match dialog.clone() {
                        EditDialog::NewFolder { value } => rsx! {
                            label { span { "文件夹名称" } input { autofocus: true, value: "{value}", placeholder: "New Folder", oninput: move |event| on_change.call(EditDialog::NewFolder { value: event.value() }) } }
                            p { class: "pak-dialog-hint", "TV ZIP 会保存显式目录；Flat PAK 只有文件路径，无法保存空目录。" }
                        },
                        EditDialog::Rename { target, value } => rsx! {
                            label { span { "新名称" } input { autofocus: true, value: "{value}", oninput: move |event| on_change.call(EditDialog::Rename { target: target.clone(), value: event.value() }) } }
                        },
                        EditDialog::Delete { target: _, label } => rsx! {
                            div { class: "pak-delete-warning", Glyph { name: "delete" } p { "确定删除 " strong { "{label}" } "？目录中的全部条目也会被删除。" } }
                        },
                        EditDialog::ArchiveProperties { format, compression_level, separator } => rsx! {
                            ArchivePropertiesEditor { format, compression_level, separator, on_change }
                        },
                        EditDialog::CloseDirty { tab_id: _, name } => rsx! {
                            div { class: "pak-delete-warning", Glyph { name: "warning" } p { strong { "{name}" } " 包含尚未保存的更改。关闭后这些更改将丢失。" } }
                        },
                    }
                }
                footer { button { class: "secondary", onclick: move |_| on_cancel.call(()), "取消" } button { class: if danger { "primary danger" } else { "primary" }, disabled: (signals.busy)(), onclick: { let dialog = dialog.clone(); move |_| on_confirm.call(dialog.clone()) }, "{confirm_label}" } }
            }
        }
    }
}

#[component]
fn ArchivePropertiesEditor(
    format: String,
    compression_level: String,
    separator: String,
    on_change: EventHandler<EditDialog>,
) -> Element {
    let level_for_format = compression_level.clone();
    let separator_for_format = separator.clone();
    let format_for_level = format.clone();
    let separator_for_level = separator.clone();
    let format_for_separator = format.clone();
    let level_for_separator = compression_level.clone();
    rsx! {
        label { span { "输出格式" }
            select { value: "{format}", onchange: move |event| on_change.call(EditDialog::ArchiveProperties {
                    format: event.value(),
                    compression_level: level_for_format.clone(),
                    separator: separator_for_format.clone(),
                }),
                option { value: "pc-raw", "PC XOR · Raw" }
                option { value: "pc-zlib", "PC XOR · zlib" }
                option { value: "plain-raw", "Plain · Raw" }
                option { value: "plain-zlib", "Plain · zlib" }
                option { value: "xbox-raw", "Xbox 360 · Raw" }
                option { value: "xbox-zlib", "Xbox 360 · zlib" }
                option { value: "tv-store", "TV ZIP · Stored" }
                option { value: "tv-deflate", "TV ZIP · Deflate" }
            }
        }
        div { class: "pak-form-grid",
            label { span { "压缩级别" }
                select { value: "{compression_level}", onchange: move |event| on_change.call(EditDialog::ArchiveProperties {
                        format: format_for_level.clone(),
                        compression_level: event.value(),
                        separator: separator_for_level.clone(),
                    }),
                    option { value: "fast", "快速" }
                    option { value: "default", "默认" }
                    option { value: "best", "最佳" }
                }
            }
            label { span { "路径分隔符" }
                select { value: "{separator}", onchange: move |event| on_change.call(EditDialog::ArchiveProperties {
                        format: format_for_separator.clone(),
                        compression_level: level_for_separator.clone(),
                        separator: event.value(),
                    }),
                    option { value: "preserve", "保留" }
                    option { value: "forward", "/" }
                    option { value: "backward", "\\" }
                }
            }
        }
        p { class: "pak-dialog-hint", "为防止静默丢失时间戳、目录或 ZIP 元数据，跨容器转换会先执行无损兼容性校验。" }
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
    let status = signals.status;
    busy.set(true);
    set_status(
        status,
        AppStatus::new("正在读取并索引 PAK…", StatusTone::Neutral),
    );
    spawn(async move {
        for file in files {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => match platform::decode_archive(Arc::from(bytes.to_vec())).await {
                    Ok(archive) => {
                        let id = next_tab_id();
                        next_tab_id.set(id.wrapping_add(1).max(1));
                        let count = archive.len();
                        signals
                            .tabs
                            .write()
                            .push(ArchiveTab::opened(id, name.clone(), archive));
                        signals.active_tab_id.set(Some(id));
                        set_status(
                            status,
                            AppStatus::new(
                                format!("已打开 {name} · {count} 个条目"),
                                StatusTone::Success,
                            ),
                        );
                    }
                    Err(error) => set_status(
                        status,
                        AppStatus::new(format!("无法打开 {name}：{error}"), StatusTone::Error),
                    ),
                },
                Err(error) => set_status(
                    status,
                    AppStatus::new(format!("无法读取 {name}：{error}"), StatusTone::Error),
                ),
            }
        }
        busy.set(false);
    });
}

fn import_files(files: Vec<FileData>, signals: WorkspaceSignals) {
    let Some(tab_id) = (signals.active_tab_id)() else {
        return;
    };
    if (signals.busy)() {
        return;
    }
    let directory = signals
        .tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| tab.directory.clone())
        .unwrap_or_default();
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
        let result = edit_active_archive(signals.tabs, signals.active_tab_id, |archive| {
            for (name, data) in &added {
                let path = join_path(&directory, name);
                if let Some(index) = archive.find_entry(path.as_bytes()) {
                    archive
                        .replace_entry_data(index, data.clone())
                        .map_err(|error| error.to_string())?;
                } else {
                    archive
                        .push_entry(PakEntry::new(path, data.clone()))
                        .map_err(|error| error.to_string())?;
                }
            }
            Ok(())
        });
        match result {
            Ok(()) => set_status(
                signals.status,
                AppStatus::new(
                    format!("已添加 {} 个文件 · 尚未保存", added.len()),
                    StatusTone::Warning,
                ),
            ),
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("添加失败：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn replace_selected(files: Vec<FileData>, signals: WorkspaceSignals) {
    let Some(index) = active_selection(&signals.tabs, signals.active_tab_id).and_then(
        |selection| match selection {
            BrowserSelection::Entry(index) => Some(index),
            BrowserSelection::Directory(_) => None,
        },
    ) else {
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
            Ok(bytes) => match edit_active_archive(signals.tabs, signals.active_tab_id, |archive| {
                archive
                    .replace_entry_data(index, bytes.to_vec())
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }) {
                Ok(()) => set_status(
                    signals.status,
                    AppStatus::new(
                        format!("已使用 {name} 替换条目 · 尚未保存"),
                        StatusTone::Warning,
                    ),
                ),
                Err(error) => set_status(
                    signals.status,
                    AppStatus::new(format!("替换失败：{error}"), StatusTone::Error),
                ),
            },
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("无法读取 {name}：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn save_active_archive(mut signals: WorkspaceSignals) {
    let Some((tab_id, name, archive)) = active_archive(&signals.tabs, signals.active_tab_id) else {
        return;
    };
    if (signals.busy)() {
        return;
    }
    let mut busy = signals.busy;
    busy.set(true);
    set_status(
        signals.status,
        AppStatus::new("正在校验并重建 PAK…", StatusTone::Neutral),
    );
    spawn(async move {
        match platform::encode_archive(archive).await {
            Ok(bytes) => {
                let save_name = normalized_pak_name(&name);
                match platform::save_bytes(&save_name, "PopCap PAK", &["pak", "zip"], &bytes).await
                {
                    Ok(true) => {
                        if let Some(tab) =
                            signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
                        {
                            tab.dirty = false;
                        }
                        set_status(
                            signals.status,
                            AppStatus::new(
                                format!(
                                    "{save_name} 已保存 · {}",
                                    format_bytes(bytes.len() as u64)
                                ),
                                StatusTone::Success,
                            ),
                        );
                    }
                    Ok(false) => set_status(signals.status, AppStatus::default()),
                    Err(error) => {
                        set_status(signals.status, AppStatus::new(error, StatusTone::Error))
                    }
                }
            }
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("无法重建 PAK：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn extract_current(signals: WorkspaceSignals) {
    let Some((_tab_id, name, archive)) = active_archive(&signals.tabs, signals.active_tab_id)
    else {
        return;
    };
    if (signals.busy)() {
        return;
    }
    let selection = active_selection(&signals.tabs, signals.active_tab_id);
    let indices = match selection {
        Some(BrowserSelection::Entry(index)) => vec![index],
        Some(BrowserSelection::Directory(path)) => indices_for_directory(&archive, &path),
        None => (0..archive.len()).collect(),
    };
    let mut busy = signals.busy;
    busy.set(true);
    set_status(
        signals.status,
        AppStatus::new(
            format!("正在提取 {} 个条目…", indices.len()),
            StatusTone::Neutral,
        ),
    );
    spawn(async move {
        match platform::extract_entries(&name, archive, indices).await {
            Ok(Some(count)) => set_status(
                signals.status,
                AppStatus::new(format!("已提取 {count} 个文件"), StatusTone::Success),
            ),
            Ok(None) => set_status(signals.status, AppStatus::default()),
            Err(error) => set_status(
                signals.status,
                AppStatus::new(format!("提取失败：{error}"), StatusTone::Error),
            ),
        }
        busy.set(false);
    });
}

fn validate_active(signals: WorkspaceSignals) {
    let Some((_id, name, archive)) = active_archive(&signals.tabs, signals.active_tab_id) else {
        return;
    };
    match archive.validate() {
        Ok(()) => set_status(
            signals.status,
            AppStatus::new(
                format!("{name} 已通过结构与元数据校验"),
                StatusTone::Success,
            ),
        ),
        Err(error) => set_status(
            signals.status,
            AppStatus::new(format!("校验失败：{error}"), StatusTone::Error),
        ),
    }
}

fn apply_dialog(dialog: EditDialog, signals: WorkspaceSignals) {
    match dialog {
        EditDialog::NewFolder { value } => create_folder(&value, signals),
        EditDialog::Rename { target, value } => rename_target(&target, &value, signals),
        EditDialog::Delete { target, .. } => delete_target(&target, signals),
        EditDialog::ArchiveProperties {
            format,
            compression_level,
            separator,
        } => apply_archive_properties(&format, &compression_level, &separator, signals),
        EditDialog::CloseDirty { tab_id, .. } => {
            close_tab(signals.tabs, signals.active_tab_id, tab_id)
        }
    }
}

fn create_folder(value: &str, signals: WorkspaceSignals) {
    let value = value.trim();
    if value.is_empty() || value.contains(['/', '\\']) {
        set_status(
            signals.status,
            AppStatus::new(
                "文件夹名称不能为空，也不能包含路径分隔符",
                StatusTone::Error,
            ),
        );
        return;
    }
    let directory = active_directory(&signals.tabs, signals.active_tab_id).unwrap_or_default();
    let path = join_path(&directory, value);
    let result = edit_active_archive(signals.tabs, signals.active_tab_id, |archive| {
        if !matches!(archive.options().format, PakFormat::TvZip { .. }) {
            return Err(
                "Flat PAK 无法保存空目录；请直接添加带目录路径的文件，或转换为 TV ZIP".to_string(),
            );
        }
        archive
            .push_entry(PakEntry::directory(path))
            .map(|_| ())
            .map_err(|error| error.to_string())
    });
    match result {
        Ok(()) => set_status(
            signals.status,
            AppStatus::new("文件夹已创建 · 尚未保存", StatusTone::Warning),
        ),
        Err(error) => set_status(signals.status, AppStatus::new(error, StatusTone::Error)),
    }
}

fn rename_target(target: &BrowserSelection, value: &str, signals: WorkspaceSignals) {
    let value = value.trim();
    if value.is_empty() || value.contains(['/', '\\']) {
        set_status(
            signals.status,
            AppStatus::new("名称不能为空，也不能包含路径分隔符", StatusTone::Error),
        );
        return;
    }
    let target = target.clone();
    let result = edit_active_archive(signals.tabs, signals.active_tab_id, |archive| {
        match target.clone() {
            BrowserSelection::Entry(index) => {
                let entry = archive
                    .entry_at(index)
                    .ok_or_else(|| "条目不存在".to_string())?;
                let mut components = path_components(entry.path().as_bytes());
                if components.is_empty() {
                    return Err("条目路径为空".to_string());
                }
                *components.last_mut().expect("checked") = value.to_string();
                let mut path = components.join("/");
                if entry.kind() == PakEntryKind::Directory {
                    path.push('/');
                }
                archive
                    .rename_entry(index, path)
                    .map_err(|error| error.to_string())?;
            }
            BrowserSelection::Directory(path) => {
                if path.is_empty() {
                    return Err("不能重命名根目录".to_string());
                }
                let mut edits = Vec::new();
                for (index, entry) in archive.entries().iter().enumerate() {
                    let components = path_components(entry.path().as_bytes());
                    if components.starts_with(&path) {
                        let mut next = components;
                        next[path.len() - 1] = value.to_string();
                        let mut joined = next.join("/");
                        if entry.kind() == PakEntryKind::Directory {
                            joined.push('/');
                        }
                        edits.push((index, joined));
                    }
                }
                for (index, path) in edits {
                    archive
                        .rename_entry(index, path)
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        archive.validate().map_err(|error| error.to_string())
    });
    match result {
        Ok(()) => {
            if matches!(target, BrowserSelection::Directory(_)) {
                update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                    tab.directory.clear();
                    tab.selection = None;
                });
            }
            set_status(
                signals.status,
                AppStatus::new("名称已更新 · 尚未保存", StatusTone::Warning),
            );
        }
        Err(error) => set_status(
            signals.status,
            AppStatus::new(format!("重命名失败：{error}"), StatusTone::Error),
        ),
    }
}

fn delete_target(target: &BrowserSelection, signals: WorkspaceSignals) {
    let target = target.clone();
    let result = edit_active_archive(signals.tabs, signals.active_tab_id, |archive| {
        let mut indices = match &target {
            BrowserSelection::Entry(index) => vec![*index],
            BrowserSelection::Directory(path) => indices_for_directory(archive, path),
        };
        indices.sort_unstable_by(|left, right| right.cmp(left));
        for index in indices {
            archive
                .remove_entry(index)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    });
    match result {
        Ok(()) => {
            update_active_tab(signals.tabs, signals.active_tab_id, |tab| {
                tab.selection = None
            });
            set_status(
                signals.status,
                AppStatus::new("项目已删除 · 尚未保存", StatusTone::Warning),
            );
        }
        Err(error) => set_status(
            signals.status,
            AppStatus::new(format!("删除失败：{error}"), StatusTone::Error),
        ),
    }
}

fn apply_archive_properties(format: &str, level: &str, separator: &str, signals: WorkspaceSignals) {
    let Some(format) = parse_format(format) else {
        return;
    };
    let options = EncodeOptions {
        format,
        compression_level: parse_compression_level(level),
        path_separator: parse_separator(separator),
        preserve_zip_metadata: true,
    };
    let result = edit_active_archive(signals.tabs, signals.active_tab_id, |archive| {
        archive
            .set_options(options, MetadataConversion::RejectLossy)
            .map(|_| ())
            .map_err(|error| error.to_string())
    });
    match result {
        Ok(()) => set_status(
            signals.status,
            AppStatus::new("归档输出属性已更新 · 尚未保存", StatusTone::Warning),
        ),
        Err(error) => set_status(
            signals.status,
            AppStatus::new(format!("无法无损转换格式：{error}"), StatusTone::Error),
        ),
    }
}

fn open_selection(
    selection: BrowserSelection,
    signals: WorkspaceSignals,
    on_open_newton: Option<EventHandler<PakNewtonOpenRequest>>,
    on_open_rton: Option<EventHandler<PakRtonOpenRequest>>,
    on_open_wem: Option<EventHandler<PakWemOpenRequest>>,
) {
    match selection {
        BrowserSelection::Directory(path) => navigate_to(signals.tabs, signals.active_tab_id, path),
        BrowserSelection::Entry(index) => {
            let Some((_id, _name, archive)) = active_archive(&signals.tabs, signals.active_tab_id)
            else {
                return;
            };
            let Some(entry) = archive.entry_at(index) else {
                return;
            };
            if entry.kind() == PakEntryKind::Directory {
                navigate_to(
                    signals.tabs,
                    signals.active_tab_id,
                    path_components(entry.path().as_bytes()),
                );
                return;
            }
            let name = file_name(entry.path().to_string_lossy().as_ref()).to_string();
            let extension = name
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_lowercase());
            let bytes: Arc<[u8]> = Arc::from(entry.data().to_vec());
            match extension.as_deref() {
                Some("rton") if on_open_rton.is_some() => {
                    on_open_rton.expect("checked").call(PakRtonOpenRequest {
                        name: name.clone(),
                        bytes,
                    })
                }
                Some("wem") if on_open_wem.is_some() => {
                    on_open_wem.expect("checked").call(PakWemOpenRequest {
                        name: name.clone(),
                        bytes,
                    })
                }
                Some("newton") if on_open_newton.is_some() => {
                    on_open_newton.expect("checked").call(PakNewtonOpenRequest {
                        name: name.clone(),
                        bytes,
                    })
                }
                _ => {
                    set_status(
                        signals.status,
                        AppStatus::new("该文件没有内置预览，可使用提取或替换", StatusTone::Neutral),
                    );
                    return;
                }
            }
            set_status(
                signals.status,
                AppStatus::new(format!("已在对应工作区打开 {name}"), StatusTone::Success),
            );
        }
    }
}

fn edit_active_archive(
    mut tabs: Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
    edit: impl FnOnce(&mut PakArchive) -> Result<(), String>,
) -> Result<(), String> {
    let id = active_tab_id().ok_or_else(|| "没有打开的归档".to_string())?;
    let mut tabs = tabs.write();
    let tab = tabs
        .iter_mut()
        .find(|tab| tab.id == id)
        .ok_or_else(|| "活动标签不存在".to_string())?;
    let mut archive = (*tab.archive).clone();
    edit(&mut archive)?;
    archive.validate().map_err(|error| error.to_string())?;
    tab.archive = Arc::new(archive);
    tab.dirty = true;
    Ok(())
}

fn browser_rows(tab: &ArchiveTab) -> Vec<BrowserRow> {
    let query = tab.query.trim().to_ascii_lowercase();
    if !query.is_empty() {
        let mut rows = tab
            .archive
            .entries()
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let full_path = entry.path().to_string_lossy().replace('\\', "/");
                full_path
                    .to_ascii_lowercase()
                    .contains(&query)
                    .then(|| entry_row(index, entry, full_path))
            })
            .collect::<Vec<_>>();
        sort_browser_rows(&mut rows, tab.sort);
        return rows;
    }
    #[derive(Default)]
    struct DirectoryAggregate {
        count: usize,
        bytes: u64,
    }
    let mut folders = BTreeMap::<String, DirectoryAggregate>::new();
    let mut files = Vec::new();
    for (index, entry) in tab.archive.entries().iter().enumerate() {
        let components = path_components(entry.path().as_bytes());
        if !components.starts_with(&tab.directory) || components.len() <= tab.directory.len() {
            continue;
        }
        if components.len() > tab.directory.len() + 1 {
            let name = components[tab.directory.len()].clone();
            let aggregate = folders.entry(name).or_default();
            aggregate.count += 1;
            aggregate.bytes += entry.data().len() as u64;
        } else if entry.kind() == PakEntryKind::Directory {
            folders
                .entry(components[tab.directory.len()].clone())
                .or_default();
        } else {
            files.push(entry_row(
                index,
                entry,
                entry.path().to_string_lossy().replace('\\', "/"),
            ));
        }
    }
    let mut rows = folders
        .into_iter()
        .map(|(name, aggregate)| {
            let mut path = tab.directory.clone();
            path.push(name.clone());
            BrowserRow {
                key: format!("d:{}", path.join("/")),
                selection: BrowserSelection::Directory(path.clone()),
                name,
                full_path: path.join("/"),
                kind: PakEntryKind::Directory,
                size: aggregate.bytes,
                detail: format!("{} 个条目", aggregate.count),
            }
        })
        .collect::<Vec<_>>();
    rows.extend(files);
    sort_browser_rows(&mut rows, tab.sort);
    rows
}

fn sort_browser_rows(rows: &mut [BrowserRow], sort: BrowserSort) {
    rows.sort_by(|left, right| {
        let left_directory = left.kind == PakEntryKind::Directory;
        let right_directory = right.kind == PakEntryKind::Directory;
        match (left_directory, right_directory) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }

        let primary = match sort.key {
            BrowserSortKey::Name => compare_text(&left.name, &right.name),
            BrowserSortKey::Type => compare_text(kind_label(left.kind), kind_label(right.kind)),
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

fn entry_row(index: usize, entry: &PakEntry, full_path: String) -> BrowserRow {
    BrowserRow {
        key: format!("e:{index}"),
        selection: BrowserSelection::Entry(index),
        name: file_name(&full_path).to_string(),
        full_path,
        kind: entry.kind(),
        size: entry.data().len() as u64,
        detail: match entry.zip_metadata() {
            Some(zip) => zip_compression_label(zip.compression()).to_string(),
            None => timestamp_label(entry.timestamp()),
        },
    }
}

fn all_directories(archive: &PakArchive) -> Vec<ArchivePath> {
    let mut directories = BTreeSet::new();
    for entry in archive.entries() {
        let components = path_components(entry.path().as_bytes());
        let directory_len = if entry.kind() == PakEntryKind::Directory {
            components.len()
        } else {
            components.len().saturating_sub(1)
        };
        for len in 1..=directory_len {
            directories.insert(components[..len].to_vec());
        }
    }
    directories.into_iter().collect()
}

fn indices_for_directory(archive: &PakArchive, directory: &[String]) -> Vec<usize> {
    archive
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            path_components(entry.path().as_bytes())
                .starts_with(directory)
                .then_some(index)
        })
        .collect()
}

fn path_components(path: &[u8]) -> Vec<String> {
    path.split(|byte| matches!(byte, b'/' | b'\\'))
        .filter(|component| !component.is_empty())
        .map(|component| String::from_utf8_lossy(component).into_owned())
        .collect()
}

fn navigate_to(
    tabs: Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
    path: ArchivePath,
) {
    update_active_tab(tabs, active_tab_id, |tab| {
        tab.directory = path;
        tab.selection = None;
        tab.query.clear();
        tab.row_limit = ROW_PAGE_SIZE;
    });
}

fn navigate_up(tabs: Signal<Vec<ArchiveTab>>, active_tab_id: Signal<Option<u64>>) {
    update_active_tab(tabs, active_tab_id, |tab| {
        tab.directory.pop();
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

fn active_archive(
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<(u64, String, Arc<PakArchive>)> {
    let id = active_tab_id()?;
    tabs.peek()
        .iter()
        .find(|tab| tab.id == id)
        .map(|tab| (id, tab.name.clone(), tab.archive.clone()))
}

fn active_selection(
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<BrowserSelection> {
    let id = active_tab_id()?;
    tabs.peek()
        .iter()
        .find(|tab| tab.id == id)
        .and_then(|tab| tab.selection.clone())
}

fn active_directory(
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<ArchivePath> {
    let id = active_tab_id()?;
    tabs.peek()
        .iter()
        .find(|tab| tab.id == id)
        .map(|tab| tab.directory.clone())
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
    tabs.remove(index);
    if was_active {
        active_tab_id.set(
            tabs.get(index)
                .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
                .map(|tab| tab.id),
        );
    }
}

fn target_name(
    target: &BrowserSelection,
    tabs: &Signal<Vec<ArchiveTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Option<String> {
    let id = active_tab_id()?;
    let tabs = tabs.peek();
    let tab = tabs.iter().find(|tab| tab.id == id)?;
    match target {
        BrowserSelection::Directory(path) => path.last().cloned(),
        BrowserSelection::Entry(index) => tab
            .archive
            .entry_at(*index)
            .map(|entry| file_name(entry.path().to_string_lossy().as_ref()).to_string()),
    }
}

fn set_status(mut status: Signal<AppStatus>, value: AppStatus) {
    let level = match value.tone {
        StatusTone::Neutral | StatusTone::Success => "INFO",
        StatusTone::Warning => "WARN",
        StatusTone::Error => "ERROR",
    };
    if !value.message.is_empty() {
        push_application_log("PAK", level, "ARCHIVE", &value.message);
    }
    status.set(value);
}

fn join_path(directory: &[String], name: &str) -> String {
    if directory.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", directory.join("/"), name)
    }
}

fn file_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}
fn archive_path_label(path: &[String]) -> String {
    path.join("/")
}
fn directory_name(path: &[String]) -> &str {
    path.last().map(String::as_str).unwrap_or("根目录")
}
fn source_format_label(archive: &PakArchive) -> &'static str {
    archive
        .source_info()
        .map(|info| format_label(info.format))
        .unwrap_or("新归档")
}
fn unix_mode_label(mode: Option<u32>) -> String {
    mode.map(|mode| format!("{mode:o}"))
        .unwrap_or_else(|| "—".to_string())
}
fn file_count(archive: &PakArchive) -> usize {
    archive
        .entries()
        .iter()
        .filter(|entry| entry.kind() != PakEntryKind::Directory)
        .count()
}
fn normalized_pak_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".pak") || lower.ends_with(".zip") {
        name.to_string()
    } else {
        format!("{name}.pak")
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn format_label(format: PakFormat) -> &'static str {
    match format {
        PakFormat::Flat {
            profile: FlatProfile::PcXor,
            compression: PakCompression::None,
        } => "PC XOR · Raw",
        PakFormat::Flat {
            profile: FlatProfile::PcXor,
            compression: PakCompression::Zlib,
        } => "PC XOR · zlib",
        PakFormat::Flat {
            profile: FlatProfile::Plain,
            compression: PakCompression::None,
        } => "Plain · Raw",
        PakFormat::Flat {
            profile: FlatProfile::Plain,
            compression: PakCompression::Zlib,
        } => "Plain · zlib",
        PakFormat::Flat {
            profile: FlatProfile::Xbox360,
            compression: PakCompression::None,
        } => "Xbox 360 · Raw",
        PakFormat::Flat {
            profile: FlatProfile::Xbox360,
            compression: PakCompression::Zlib,
        } => "Xbox 360 · zlib",
        PakFormat::TvZip {
            compression: ZipCompression::Stored,
        } => "TV ZIP · Stored",
        PakFormat::TvZip {
            compression: ZipCompression::Deflated,
        } => "TV ZIP · Deflate",
    }
}

fn format_code(format: PakFormat) -> &'static str {
    match format {
        PakFormat::Flat {
            profile: FlatProfile::PcXor,
            compression: PakCompression::None,
        } => "pc-raw",
        PakFormat::Flat {
            profile: FlatProfile::PcXor,
            compression: PakCompression::Zlib,
        } => "pc-zlib",
        PakFormat::Flat {
            profile: FlatProfile::Plain,
            compression: PakCompression::None,
        } => "plain-raw",
        PakFormat::Flat {
            profile: FlatProfile::Plain,
            compression: PakCompression::Zlib,
        } => "plain-zlib",
        PakFormat::Flat {
            profile: FlatProfile::Xbox360,
            compression: PakCompression::None,
        } => "xbox-raw",
        PakFormat::Flat {
            profile: FlatProfile::Xbox360,
            compression: PakCompression::Zlib,
        } => "xbox-zlib",
        PakFormat::TvZip {
            compression: ZipCompression::Stored,
        } => "tv-store",
        PakFormat::TvZip {
            compression: ZipCompression::Deflated,
        } => "tv-deflate",
    }
}

fn parse_format(value: &str) -> Option<PakFormat> {
    Some(match value {
        "pc-raw" => PakFormat::pc(PakCompression::None),
        "pc-zlib" => PakFormat::pc(PakCompression::Zlib),
        "plain-raw" => PakFormat::plain(PakCompression::None),
        "plain-zlib" => PakFormat::plain(PakCompression::Zlib),
        "xbox-raw" => PakFormat::xbox360(PakCompression::None),
        "xbox-zlib" => PakFormat::xbox360(PakCompression::Zlib),
        "tv-store" => PakFormat::TvZip {
            compression: ZipCompression::Stored,
        },
        "tv-deflate" => PakFormat::TvZip {
            compression: ZipCompression::Deflated,
        },
        _ => return None,
    })
}

fn compression_level_code(value: CompressionLevel) -> &'static str {
    match value {
        CompressionLevel::Fast => "fast",
        CompressionLevel::Default => "default",
        CompressionLevel::Best => "best",
    }
}
fn parse_compression_level(value: &str) -> CompressionLevel {
    match value {
        "fast" => CompressionLevel::Fast,
        "best" => CompressionLevel::Best,
        _ => CompressionLevel::Default,
    }
}
fn separator_code(value: PathSeparator) -> &'static str {
    match value {
        PathSeparator::Preserve => "preserve",
        PathSeparator::ForwardSlash => "forward",
        PathSeparator::Backslash => "backward",
    }
}
fn parse_separator(value: &str) -> PathSeparator {
    match value {
        "forward" => PathSeparator::ForwardSlash,
        "backward" => PathSeparator::Backslash,
        _ => PathSeparator::Preserve,
    }
}
fn kind_label(kind: PakEntryKind) -> &'static str {
    match kind {
        PakEntryKind::File => "文件",
        PakEntryKind::Directory => "文件夹",
        PakEntryKind::Symlink => "符号链接",
    }
}
fn kind_class(kind: PakEntryKind) -> &'static str {
    match kind {
        PakEntryKind::File => "file",
        PakEntryKind::Directory => "folder",
        PakEntryKind::Symlink => "link",
    }
}
fn kind_glyph(kind: PakEntryKind) -> &'static str {
    match kind {
        PakEntryKind::File => "file",
        PakEntryKind::Directory => "folder",
        PakEntryKind::Symlink => "link",
    }
}
fn zip_compression_label(value: ZipCompression) -> &'static str {
    match value {
        ZipCompression::Stored => "Stored",
        ZipCompression::Deflated => "Deflate",
    }
}
fn timestamp_label(value: PakTimestamp) -> String {
    match value {
        PakTimestamp::None => "—".to_string(),
        PakTimestamp::PopCap(value) => format_filetime(value).unwrap_or_else(|| value.to_string()),
        PakTimestamp::ZipMsDos { date, time } => {
            let year = 1980 + u32::from(date >> 9);
            let month = u32::from((date >> 5) & 0x0f);
            let day = u32::from(date & 0x1f);
            let hour = u32::from(time >> 11);
            let minute = u32::from((time >> 5) & 0x3f);
            let second = u32::from(time & 0x1f) * 2;
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
        }
    }
}

fn format_filetime(ticks: u64) -> Option<String> {
    const WINDOWS_TO_UNIX_SECONDS: i128 = 11_644_473_600;
    let unix_seconds = i128::from(ticks) / 10_000_000 - WINDOWS_TO_UNIX_SECONDS;
    let unix_seconds = i64::try_from(unix_seconds).ok()?;
    let days = unix_seconds.div_euclid(86_400);
    let seconds = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_unix_days(days)?;
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
}

fn civil_date_from_unix_days(days: i64) -> Option<(i64, i64, i64)> {
    let shifted = days.checked_add(719_468)?;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted.checked_sub(146_096)?
    } / 146_097;
    let day_of_era = shifted.checked_sub(era.checked_mul(146_097)?)?;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era.checked_add(era.checked_mul(400)?)?;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (1..=12).contains(&month).then_some((year, month, day))
}
fn tab_class(active: bool, dirty: bool) -> &'static str {
    match (active, dirty) {
        (true, true) => "pak-tab ui-document-tab is-active has-changes",
        (true, false) => "pak-tab ui-document-tab is-active",
        (false, true) => "pak-tab ui-document-tab has-changes",
        (false, false) => "pak-tab ui-document-tab",
    }
}

#[component]
fn Glyph(name: &'static str) -> Element {
    let path = match name {
        "open" => "M3 7h6l2 2h10v10H3z M3 7V5h6l2 2",
        "archive" => "M4 4h16v4H4z M5 8h14v12H5z M9 12h6",
        "tree" => "M7 4v16 M7 8h7 M7 15h7 M14 6v4 M14 13v4",
        "up" => "M6 15l6-6 6 6",
        "extract" => "M12 3v12 M7 10l5 5 5-5 M4 20h16",
        "add-file" => "M5 3h9l5 5v13H5z M14 3v5h5 M12 11v6 M9 14h6",
        "folder-add" => "M3 7h7l2 2h9v10H3z M12 11v6 M9 14h6",
        "save" => "M5 3h12l2 2v16H5z M8 3v6h8V3 M8 14h8v7",
        "more" => "M5 12h.01 M12 12h.01 M19 12h.01",
        "inspector" => "M4 4h16v16H4z M14 4v16 M7 8h4 M7 12h4",
        "close" => "M6 6l12 12 M18 6L6 18",
        "add" => "M12 5v14 M5 12h14",
        "file" => "M6 3h8l4 4v14H6z M14 3v5h5",
        "folder" => "M3 7h7l2 2h9v10H3z",
        "size" => "M4 7h16v10H4z M8 10v4 M12 10v4 M16 10v4",
        "search" => "M11 4a7 7 0 1 0 0 14a7 7 0 0 0 0-14 M16 16l5 5",
        "link" => {
            "M10 13a5 5 0 0 0 7 0l2-2a5 5 0 0 0-7-7l-1 1 M14 11a5 5 0 0 0-7 0l-2 2a5 5 0 0 0 7 7l1-1"
        }
        "replace" => "M4 7h12l-3-3 M16 7l-3 3 M20 17H8l3 3 M8 17l3-3",
        "rename" => "M4 20h4l11-11-4-4L4 16z M13 7l4 4",
        "delete" => "M4 7h16 M9 7V4h6v3 M7 7l1 14h8l1-14 M10 11v6 M14 11v6",
        "validate" => "M12 3l8 4v5c0 5-3 8-8 10-5-2-8-5-8-10V7z M8 12l3 3 5-6",
        "properties" => "M4 5h16 M4 12h16 M4 19h16 M8 3v4 M16 10v4 M10 17v4",
        "warning" => "M12 3L2 21h20z M12 9v5 M12 18h.01",
        _ => "M4 4h16v16H4z",
    };
    rsx! { svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: path } } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_groups_nested_paths() {
        let archive = PakArchive::new(
            EncodeOptions::default(),
            vec![
                PakEntry::new("a/b.txt", b"x".to_vec()),
                PakEntry::new("root.txt", b"yy".to_vec()),
            ],
        )
        .unwrap();
        let tab = ArchiveTab::opened(1, "test.pak".to_string(), archive);
        let rows = browser_rows(&tab);
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0].selection, BrowserSelection::Directory(_)));
    }

    #[test]
    fn browser_sort_keeps_directories_first_and_uses_raw_sizes() {
        let archive = PakArchive::new(
            EncodeOptions::default(),
            vec![
                PakEntry::new("z-small.bin", vec![0]),
                PakEntry::new("a-large.bin", vec![0; 2_048]),
                PakEntry::new("folder/item.bin", vec![0; 16]),
            ],
        )
        .unwrap();
        let mut tab = ArchiveTab::opened(1, "test.pak".to_string(), archive);
        tab.sort = BrowserSort {
            key: BrowserSortKey::Size,
            direction: BrowserSortDirection::Descending,
        };

        let rows = browser_rows(&tab);
        assert!(matches!(rows[0].selection, BrowserSelection::Directory(_)));
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

    #[test]
    fn directory_selection_covers_descendants_only() {
        let archive = PakArchive::new(
            EncodeOptions::default(),
            vec![
                PakEntry::new("a/one", vec![]),
                PakEntry::new("ab/two", vec![]),
            ],
        )
        .unwrap();
        assert_eq!(indices_for_directory(&archive, &["a".to_string()]), vec![0]);
    }

    #[test]
    fn popcap_filetime_is_presented_as_utc_calendar_time() {
        assert_eq!(
            format_filetime(116_444_736_000_000_000).as_deref(),
            Some("1970-01-01 00:00:00")
        );
    }
}
