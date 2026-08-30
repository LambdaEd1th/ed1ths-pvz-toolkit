use crate::{NewtonOpenRequest, platform};
use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use newton_manifest::{
    CompositeGroup, Resource, ResourceGroup, ResourceManifest, ResourceType, SimpleGroup, Subgroup,
    ValidationIssue, ValidationProfile, ValidationReport, ValidationSeverity, from_bytes, to_bytes,
};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use toolkit_ui::{
    DropIndicator, InlineNotice, ToolPage, ToolPageToolbar, WorkspaceCard, push_application_log,
};

const NEWTON_PAGE_CSS: Asset = asset!("/assets/newton/page.css");
const MANIFEST_FILE_ACCEPT: &str = ".newton,.json,.yaml,.yml,.toml,application/json,application/yaml,text/yaml,text/plain,application/octet-stream";
const GROUP_PAGE_SIZE: usize = 120;
const RECORD_PAGE_SIZE: usize = 160;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ManifestFormat {
    #[default]
    Newton,
    Json,
    Yaml,
    Toml,
}

impl ManifestFormat {
    const ALL: [Self; 4] = [Self::Newton, Self::Json, Self::Yaml, Self::Toml];
    #[cfg(test)]
    const TEXT: [Self; 3] = [Self::Json, Self::Yaml, Self::Toml];

    const fn label(self) -> &'static str {
        match self {
            Self::Newton => "NEWTON",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Newton => "官方二进制清单",
            Self::Json => "格式化 JSON 文本",
            Self::Yaml => "易读 YAML 文本",
            Self::Toml => "TOML 配置文本",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Newton => "newton",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }

    const fn filter_name(self) -> &'static str {
        match self {
            Self::Newton => "NEWTON manifest",
            Self::Json => "JSON document",
            Self::Yaml => "YAML document",
            Self::Toml => "TOML document",
        }
    }

    const fn filter_extensions(self) -> &'static [&'static str] {
        match self {
            Self::Newton => &["newton"],
            Self::Json => &["json"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
        }
    }

    const fn export_extension(self) -> &'static str {
        match self {
            Self::Newton => "NEWTON",
            _ => self.extension(),
        }
    }

    fn from_file_name(name: &str) -> Self {
        match name.rsplit_once('.').map(|(_, extension)| extension) {
            Some(extension) if extension.eq_ignore_ascii_case("json") => Self::Json,
            Some(extension)
                if extension.eq_ignore_ascii_case("yaml")
                    || extension.eq_ignore_ascii_case("yml") =>
            {
                Self::Yaml
            }
            Some(extension) if extension.eq_ignore_ascii_case("toml") => Self::Toml,
            _ => Self::Newton,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ManifestSelection {
    #[default]
    Group,
    Record(usize),
}

#[derive(Clone)]
struct ManifestTab {
    id: u64,
    name: String,
    manifest: Arc<RwLock<ResourceManifest>>,
    revision: u64,
    dirty: bool,
    selected_group: usize,
    selection: ManifestSelection,
    group_query: String,
    group_page: usize,
    record_query: String,
    record_page: usize,
    validation: ValidationReport,
    validation_stale: bool,
}

impl PartialEq for ManifestTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.revision == other.revision
            && self.dirty == other.dirty
            && self.selected_group == other.selected_group
            && self.selection == other.selection
            && self.group_query == other.group_query
            && self.group_page == other.group_page
            && self.record_query == other.record_query
            && self.record_page == other.record_page
            && self.validation == other.validation
            && self.validation_stale == other.validation_stale
    }
}

impl ManifestTab {
    fn opened(id: u64, name: String, manifest: ResourceManifest) -> Self {
        let validation = manifest.validate(ValidationProfile::Canonical);
        Self {
            id,
            name,
            manifest: Arc::new(RwLock::new(manifest)),
            revision: 0,
            dirty: false,
            selected_group: 0,
            selection: ManifestSelection::Group,
            group_query: String::new(),
            group_page: 0,
            record_query: String::new(),
            record_page: 0,
            validation,
            validation_stale: false,
        }
    }

    fn blank(id: u64) -> Self {
        Self::opened(
            id,
            "RESOURCES.NEWTON".to_string(),
            ResourceManifest {
                slot_count: 0,
                groups: Vec::new(),
            },
        )
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

fn read_manifest(
    manifest: &Arc<RwLock<ResourceManifest>>,
) -> RwLockReadGuard<'_, ResourceManifest> {
    manifest
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_manifest(
    manifest: &Arc<RwLock<ResourceManifest>>,
) -> RwLockWriteGuard<'_, ResourceManifest> {
    manifest
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[component]
pub fn NewtonManifestPage(
    open_request: Option<NewtonOpenRequest>,
    #[props(default = true)] active: bool,
) -> Element {
    let tabs = use_signal(Vec::<ManifestTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);
    let mut export_menu_open = use_signal(|| false);
    let mut handled_open_request_id = use_signal(|| None::<u64>);
    let mut pending_close_tab_id = use_signal(|| None::<u64>);

    use_effect(use_reactive(&open_request, move |request| {
        let Some(request) = request else {
            return;
        };
        if handled_open_request_id() == Some(request.id) {
            return;
        }
        handled_open_request_id.set(Some(request.id));
        open_manifest_bytes(
            request.name,
            request.bytes,
            tabs,
            active_tab_id,
            next_tab_id,
            status,
        );
    }));

    if !active {
        return rsx! {};
    }

    let tabs_snapshot = tabs();
    let active_id_snapshot = active_tab_id();
    let active_tab_snapshot = active_id_snapshot
        .and_then(|active_id| tabs_snapshot.iter().find(|tab| tab.id == active_id))
        .cloned();
    let host_class = if dragging() {
        "newton-page-host is-dragging"
    } else {
        "newton-page-host"
    };
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: NEWTON_PAGE_CSS }
        div {
            class: "{host_class}",
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
                    load_manifest_files(files, tabs, active_tab_id, next_tab_id, status);
                }
            },
            ToolPage { namespace: "newton", class: "newton-page",
                ToolPageToolbar {
                    class: "newton-page-toolbar",
                    actions: rsx! {
                        div { class: "ui-tool-page-actions newton-toolbar-actions",
                            label {
                                class: "newton-icon-button primary",
                                title: "打开 NEWTON / JSON / YAML / TOML",
                                aria_label: "打开清单",
                                Glyph { name: "open" }
                                input {
                                    class: "newton-file-input",
                                    r#type: "file",
                                    accept: MANIFEST_FILE_ACCEPT,
                                    multiple: true,
                                    onchange: move |event| {
                                        let files = event.files();
                                        if !files.is_empty() {
                                            load_manifest_files(
                                                files,
                                                tabs,
                                                active_tab_id,
                                                next_tab_id,
                                                status,
                                            );
                                        }
                                    }
                                }
                            }
                            button {
                                r#type: "button",
                                class: "newton-icon-button",
                                title: "新建清单",
                                aria_label: "新建清单",
                                onclick: move |_| create_blank_tab(
                                    tabs,
                                    active_tab_id,
                                    next_tab_id,
                                    status,
                                ),
                                Glyph { name: "new" }
                            }
                            span { class: "newton-toolbar-divider" }
                            button {
                                r#type: "button",
                                class: "newton-icon-button",
                                disabled: active_tab_snapshot.is_none(),
                                title: "添加复合资源组",
                                aria_label: "添加复合资源组",
                                onclick: move |_| add_group(
                                    tabs,
                                    active_tab_id,
                                    true,
                                    status,
                                ),
                                Glyph { name: "composite" }
                            }
                            button {
                                r#type: "button",
                                class: "newton-icon-button",
                                disabled: active_tab_snapshot.is_none(),
                                title: "添加简单资源组",
                                aria_label: "添加简单资源组",
                                onclick: move |_| add_group(
                                    tabs,
                                    active_tab_id,
                                    false,
                                    status,
                                ),
                                Glyph { name: "group" }
                            }
                            span { class: "newton-toolbar-spacer" }
                            button {
                                r#type: "button",
                                class: "newton-icon-button",
                                disabled: active_tab_snapshot.is_none(),
                                title: "重新校验",
                                aria_label: "重新校验",
                                onclick: move |_| validate_active_tab(tabs, active_tab_id, status),
                                Glyph { name: "validate" }
                            }
                            div { class: "newton-export-control",
                                button {
                                    r#type: "button",
                                    class: if export_menu_open() {
                                        "newton-icon-button primary is-open"
                                    } else {
                                        "newton-icon-button primary"
                                    },
                                    disabled: active_tab_snapshot.is_none(),
                                    title: "导出清单",
                                    aria_label: "导出清单",
                                    aria_expanded: export_menu_open(),
                                    onclick: move |_| export_menu_open.toggle(),
                                    Glyph { name: "save" }
                                }
                                if export_menu_open() {
                                    button {
                                        r#type: "button",
                                        class: "newton-export-backdrop",
                                        aria_label: "关闭导出菜单",
                                        onclick: move |_| export_menu_open.set(false),
                                    }
                                    div {
                                        class: "newton-export-menu",
                                        role: "menu",
                                        aria_label: "导出格式",
                                        for format in ManifestFormat::ALL {
                                            button {
                                                r#type: "button",
                                                role: "menuitem",
                                                onclick: move |_| {
                                                    export_menu_open.set(false);
                                                    save_active_tab(
                                                        tabs,
                                                        active_tab_id,
                                                        status,
                                                        format,
                                                    );
                                                },
                                                span { class: "newton-export-format-mark",
                                                    if format == ManifestFormat::Newton {
                                                        Glyph { name: "manifest" }
                                                    } else {
                                                        Glyph { name: "code" }
                                                    }
                                                }
                                                span {
                                                    strong { "{format.label()}" }
                                                    small { "{format.description()}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                ManifestTabStrip {
                    tabs: tabs_snapshot.clone(),
                    active_tab_id: active_id_snapshot,
                    on_activate: move |tab_id| {
                        active_tab_id.set(Some(tab_id));
                        status.set(AppStatus::default());
                    },
                    on_close: move |tab_id| {
                        let dirty = tabs
                            .peek()
                            .iter()
                            .find(|tab| tab.id == tab_id)
                            .is_some_and(|tab| tab.dirty);
                        if dirty {
                            pending_close_tab_id.set(Some(tab_id));
                        } else {
                            close_tab(tabs, active_tab_id, tab_id);
                        }
                    },
                    on_files: move |files| {
                        load_manifest_files(files, tabs, active_tab_id, next_tab_id, status)
                    }
                }

                WorkspaceCard { class: "newton-workspace-card", aria_label: "NEWTON Manifest Editor",
                    if let Some(tab) = active_tab_snapshot {
                        ManifestWorkspace { tab, tabs, status }
                    } else {
                        EmptyWorkspace {
                            on_files: move |files| load_manifest_files(
                                files,
                                tabs,
                                active_tab_id,
                                next_tab_id,
                                status,
                            ),
                            on_new: move |_| create_blank_tab(
                                tabs,
                                active_tab_id,
                                next_tab_id,
                                status,
                            )
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "newton-status-notice",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "拖放 NEWTON / JSON / YAML / TOML 以打开".to_string() }
                    }
                }
            }
        }

        if let Some(tab_id) = pending_close_tab_id() {
            CloseDialog {
                on_cancel: move |_| pending_close_tab_id.set(None),
                on_discard: move |_| {
                    pending_close_tab_id.set(None);
                    close_tab(tabs, active_tab_id, tab_id);
                }
            }
        }
    }
}

#[component]
fn ManifestTabStrip(
    tabs: Vec<ManifestTab>,
    active_tab_id: Option<u64>,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "newton-tab-strip ui-document-tab-strip",
            div {
                class: "newton-tab-list ui-document-tab-list",
                role: "tablist",
                aria_label: "打开的 NEWTON 清单",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: manifest_tab_class(Some(tab.id) == active_tab_id, tab.dirty),
                        button {
                            r#type: "button",
                            class: "newton-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            tabindex: if Some(tab.id) == active_tab_id { "0" } else { "-1" },
                            title: "{tab.name}",
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "newton-tab-dot ui-document-tab-dot" }
                            span { class: "newton-tab-name ui-document-tab-name", "{tab.name}" }
                        }
                        button {
                            r#type: "button",
                            class: "newton-tab-close ui-document-tab-close",
                            title: "关闭 {tab.name}",
                            aria_label: "关闭 {tab.name}",
                            onclick: move |event| {
                                event.stop_propagation();
                                on_close.call(tab.id);
                            },
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: "newton-tab-add ui-document-new-tab",
                    title: "添加 NEWTON",
                    aria_label: "添加 NEWTON",
                    Glyph { name: "add" }
                    input {
                        class: "newton-file-input",
                        r#type: "file",
                        accept: MANIFEST_FILE_ACCEPT,
                        multiple: true,
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
fn EmptyWorkspace(
    on_files: EventHandler<Vec<FileData>>,
    on_new: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section { class: "newton-empty-workspace",
            div { class: "newton-empty-icon", Glyph { name: "manifest" } }
            span { class: "newton-eyebrow", "RESOURCE MANIFEST" }
            h2 { "打开 NEWTON 资源清单" }
            p { "支持 NEWTON、JSON、YAML 与 TOML；每个清单独立保存在一个标签页中。" }
            div { class: "newton-empty-actions",
                label { class: "newton-action-button primary",
                    Glyph { name: "open" }
                    "选择清单"
                    input {
                        class: "newton-file-input",
                        r#type: "file",
                        accept: MANIFEST_FILE_ACCEPT,
                        multiple: true,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() {
                                on_files.call(files);
                            }
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "newton-action-button",
                    onclick: move |event| on_new.call(event),
                    Glyph { name: "new" }
                    "新建清单"
                }
            }
            small { "也可以直接拖入文本清单，或从 RSB Archive 中打开 NEWTON" }
        }
    }
}

#[component]
fn ManifestWorkspace(
    tab: ManifestTab,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    let manifest = read_manifest(&tab.manifest);
    let slot_count = manifest.slot_count;
    let group_count = manifest.groups.len();
    let resource_count = manifest
        .groups
        .iter()
        .map(|group| match group {
            ResourceGroup::Simple(group) => group.resources.len(),
            ResourceGroup::Composite(_) => 0,
        })
        .sum::<usize>();
    let subgroup_count = manifest
        .groups
        .iter()
        .map(|group| match group {
            ResourceGroup::Composite(group) => group.subgroups.len(),
            ResourceGroup::Simple(_) => 0,
        })
        .sum::<usize>();
    drop(manifest);
    let errors = tab.validation.errors().count();
    let warnings = tab.validation.warnings().count();
    rsx! {
        div { class: "newton-document",
            section { class: "newton-summary-grid",
                SummaryCard { label: "槽位", value: slot_count.to_string(), glyph: "slot" }
                SummaryCard { label: "资源组", value: group_count.to_string(), glyph: "group" }
                SummaryCard { label: "资源 / 子组", value: format!("{resource_count} / {subgroup_count}"), glyph: "records" }
                SummaryCard {
                    label: if tab.validation_stale { "校验（待刷新）" } else { "校验" },
                    value: format!("{errors} / {warnings}"),
                    glyph: if errors > 0 { "warning" } else { "validate" }
                }
            }

            div { class: "newton-editor-grid",
                GroupNavigator { tab: tab.clone(), tabs, status }
                RecordPanel { tab: tab.clone(), tabs, status }
                InspectorPanel { tab, tabs, status }
            }
        }
    }
}

#[component]
fn SummaryCard(label: String, value: String, glyph: String) -> Element {
    rsx! {
        article { class: "newton-summary-card",
            span { class: "newton-summary-icon", Glyph { name: glyph } }
            div {
                small { "{label}" }
                strong { "{value}" }
            }
        }
    }
}

#[component]
fn GroupNavigator(
    tab: ManifestTab,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    let query = tab.group_query.trim().to_ascii_lowercase();
    let manifest = read_manifest(&tab.manifest);
    let group_count = manifest.groups.len();
    let visible_groups = manifest
        .groups
        .iter()
        .enumerate()
        .filter(|(_, group)| query.is_empty() || group.id().to_ascii_lowercase().contains(&query))
        .map(|(index, group)| {
            (
                index,
                group.id().to_string(),
                matches!(group, ResourceGroup::Composite(_)),
                match group {
                    ResourceGroup::Composite(group) => group.subgroups.len(),
                    ResourceGroup::Simple(group) => group.resources.len(),
                },
            )
        })
        .collect::<Vec<_>>();
    drop(manifest);
    let filtered_total = visible_groups.len();
    let page_count = filtered_total.div_ceil(GROUP_PAGE_SIZE).max(1);
    let current_page = tab.group_page.min(page_count - 1);
    let page_start = current_page * GROUP_PAGE_SIZE;
    let page_groups = visible_groups
        .into_iter()
        .skip(page_start)
        .take(GROUP_PAGE_SIZE)
        .collect::<Vec<_>>();

    rsx! {
        aside { class: "newton-panel newton-group-panel",
            PanelHeading {
                eyebrow: "GROUPS",
                title: "资源组",
                action: rsx! {
                    span { class: "newton-panel-count", "{group_count}" }
                }
            }
            label { class: "newton-search-field",
                Glyph { name: "search" }
                input {
                    r#type: "search",
                    value: "{tab.group_query}",
                    placeholder: "搜索资源组",
                    oninput: move |event| {
                        update_tab_state(tabs, tab.id, |tab| {
                            tab.group_query = event.value();
                            tab.group_page = 0;
                        });
                    }
                }
            }
            div { class: "newton-group-list",
                if page_groups.is_empty() {
                    div { class: "newton-list-empty", "没有匹配的资源组" }
                }
                for (index, id, composite, count) in page_groups {
                    button {
                        r#type: "button",
                        class: if index == tab.selected_group {
                            "newton-group-item is-active"
                        } else {
                            "newton-group-item"
                        },
                        onclick: move |_| {
                            update_tab_state(tabs, tab.id, |tab| {
                                tab.selected_group = index;
                                tab.selection = ManifestSelection::Group;
                                tab.record_query.clear();
                                tab.record_page = 0;
                            });
                        },
                        span { class: if composite { "newton-group-mark composite" } else { "newton-group-mark simple" },
                            if composite { "C" } else { "S" }
                        }
                        span { class: "newton-group-copy",
                            strong { "{id}" }
                            small {
                                if composite {
                                    "{count} 个子组"
                                } else {
                                    "{count} 个资源"
                                }
                            }
                        }
                        Glyph { name: "chevron" }
                    }
                }
            }
            footer { class: "newton-list-pagination",
                span { "{page_start.saturating_add(1).min(filtered_total)}–{(page_start + GROUP_PAGE_SIZE).min(filtered_total)} / {filtered_total}" }
                div {
                    button {
                        r#type: "button",
                        disabled: current_page == 0,
                        onclick: move |_| update_tab_state(tabs, tab.id, |tab| {
                            tab.group_page = tab.group_page.saturating_sub(1);
                        }),
                        Glyph { name: "left" }
                    }
                    span { "{current_page + 1} / {page_count}" }
                    button {
                        r#type: "button",
                        disabled: current_page + 1 >= page_count,
                        onclick: move |_| update_tab_state(tabs, tab.id, |tab| {
                            tab.group_page = tab.group_page.saturating_add(1);
                        }),
                        Glyph { name: "right" }
                    }
                }
            }
            div { class: "newton-panel-footer-actions",
                button {
                    r#type: "button",
                    title: "添加复合资源组",
                    onclick: move |_| add_group_by_tab(tabs, tab.id, true, status),
                    Glyph { name: "composite" }
                    "复合组"
                }
                button {
                    r#type: "button",
                    title: "添加简单资源组",
                    onclick: move |_| add_group_by_tab(tabs, tab.id, false, status),
                    Glyph { name: "group" }
                    "简单组"
                }
            }
        }
    }
}

#[component]
fn RecordPanel(
    tab: ManifestTab,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    let manifest = read_manifest(&tab.manifest);
    let Some(group) = manifest.groups.get(tab.selected_group) else {
        return rsx! {
            main { class: "newton-panel newton-record-panel newton-no-group",
                Glyph { name: "group" }
                h2 { "还没有资源组" }
                p { "创建复合组以引用子组，或创建简单组以保存资源记录。" }
            }
        };
    };
    let group_id = group.id().to_string();
    let is_composite = matches!(group, ResourceGroup::Composite(_));
    let query = tab.record_query.trim().to_ascii_lowercase();
    let (kind, total, visible_records) = match &group {
        ResourceGroup::Composite(group) => {
            let rows = group
                .subgroups
                .iter()
                .enumerate()
                .filter(|(_, subgroup)| {
                    query.is_empty() || subgroup.id.to_ascii_lowercase().contains(&query)
                })
                .map(|(index, subgroup)| RecordRow::Subgroup {
                    index,
                    id: subgroup.id.clone(),
                    resolution: subgroup.resolution,
                })
                .collect::<Vec<_>>();
            ("复合资源组", group.subgroups.len(), rows)
        }
        ResourceGroup::Simple(group) => {
            let rows = group
                .resources
                .iter()
                .enumerate()
                .filter(|(_, resource)| {
                    query.is_empty()
                        || resource.id.to_ascii_lowercase().contains(&query)
                        || resource.path.to_ascii_lowercase().contains(&query)
                        || resource_type_label(resource.resource_type)
                            .to_ascii_lowercase()
                            .contains(&query)
                })
                .map(|(index, resource)| RecordRow::Resource {
                    index,
                    id: resource.id.clone(),
                    path: resource.path.clone(),
                    slot: resource.slot,
                    resource_type: resource.resource_type,
                })
                .collect::<Vec<_>>();
            ("简单资源组", group.resources.len(), rows)
        }
    };
    let filtered_total = visible_records.len();
    let page_count = filtered_total.div_ceil(RECORD_PAGE_SIZE).max(1);
    let current_page = tab.record_page.min(page_count - 1);
    let page_start = current_page * RECORD_PAGE_SIZE;
    let page_rows = visible_records
        .into_iter()
        .skip(page_start)
        .take(RECORD_PAGE_SIZE)
        .collect::<Vec<_>>();
    drop(manifest);

    rsx! {
        main { class: "newton-panel newton-record-panel",
            PanelHeading {
                eyebrow: "MANIFEST",
                title: group_id,
                action: rsx! {
                    span { class: "newton-kind-badge", "{kind}" }
                }
            }
            div { class: "newton-record-toolbar",
                label { class: "newton-search-field",
                    Glyph { name: "search" }
                    input {
                        r#type: "search",
                        value: "{tab.record_query}",
                        placeholder: "搜索 ID、路径或类型",
                        oninput: move |event| {
                            update_tab_state(tabs, tab.id, |tab| {
                                tab.record_query = event.value();
                                tab.record_page = 0;
                            });
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "newton-compact-button",
                    onclick: move |_| add_record(tabs, tab.id, tab.selected_group, status),
                    Glyph { name: "add" }
                    if is_composite { "添加子组" } else { "添加资源" }
                }
            }
            div { class: "newton-record-list",
                if page_rows.is_empty() {
                    div { class: "newton-list-empty newton-record-empty",
                        Glyph { name: "records" }
                        strong { if total == 0 { "该资源组还是空的" } else { "没有匹配的记录" } }
                    }
                }
                for row in page_rows {
                    {render_record_row(row, &tab, tabs)}
                }
            }
            footer { class: "newton-record-footer",
                span { "显示 {page_start.saturating_add(1).min(filtered_total)}–{(page_start + RECORD_PAGE_SIZE).min(filtered_total)} / {filtered_total}" }
                div {
                    button {
                        r#type: "button",
                        disabled: current_page == 0,
                        onclick: move |_| update_tab_state(tabs, tab.id, |tab| {
                            tab.record_page = tab.record_page.saturating_sub(1);
                        }),
                        Glyph { name: "left" }
                    }
                    span { "{current_page + 1} / {page_count}" }
                    button {
                        r#type: "button",
                        disabled: current_page + 1 >= page_count,
                        onclick: move |_| update_tab_state(tabs, tab.id, |tab| {
                            tab.record_page = tab.record_page.saturating_add(1);
                        }),
                        Glyph { name: "right" }
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
enum RecordRow {
    Subgroup {
        index: usize,
        id: String,
        resolution: Option<u32>,
    },
    Resource {
        index: usize,
        id: String,
        path: String,
        slot: u32,
        resource_type: ResourceType,
    },
}

fn render_record_row(row: RecordRow, tab: &ManifestTab, tabs: Signal<Vec<ManifestTab>>) -> Element {
    let tab_id = tab.id;
    let selection = tab.selection;
    match row {
        RecordRow::Subgroup {
            index,
            id,
            resolution,
        } => rsx! {
            button {
                key: "subgroup-{index}",
                r#type: "button",
                class: if selection == ManifestSelection::Record(index) {
                    "newton-record-row is-active"
                } else {
                    "newton-record-row"
                },
                onclick: move |_| update_tab_state(tabs, tab_id, |tab| {
                    tab.selection = ManifestSelection::Record(index);
                }),
                span { class: "newton-record-type subgroup", "SG" }
                span { class: "newton-record-main",
                    strong { "{id}" }
                    small { "子组引用" }
                }
                span { class: "newton-record-meta",
                    if let Some(resolution) = resolution {
                        "{resolution}p"
                    } else {
                        "默认分辨率"
                    }
                }
                Glyph { name: "chevron" }
            }
        },
        RecordRow::Resource {
            index,
            id,
            path,
            slot,
            resource_type,
        } => rsx! {
            button {
                key: "resource-{index}",
                r#type: "button",
                class: if selection == ManifestSelection::Record(index) {
                    "newton-record-row is-active"
                } else {
                    "newton-record-row"
                },
                onclick: move |_| update_tab_state(tabs, tab_id, |tab| {
                    tab.selection = ManifestSelection::Record(index);
                }),
                span { class: "newton-record-type", "{resource_type_short(resource_type)}" }
                span { class: "newton-record-main",
                    strong { "{id}" }
                    small { title: "{path}", "{path}" }
                }
                span { class: "newton-record-meta", "Slot {slot}" }
                Glyph { name: "chevron" }
            }
        },
    }
}

#[component]
fn InspectorPanel(
    tab: ManifestTab,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    let manifest = read_manifest(&tab.manifest);
    let selected_fields =
        manifest
            .groups
            .get(tab.selected_group)
            .map(|group| match (group, tab.selection) {
                (ResourceGroup::Composite(group), ManifestSelection::Record(index)) => group
                    .subgroups
                    .get(index)
                    .cloned()
                    .map(|subgroup| {
                        rsx! {
                            SubgroupFields {
                                tab_id: tab.id,
                                group_index: tab.selected_group,
                                record_index: index,
                                subgroup,
                                tabs,
                                status,
                            }
                        }
                    })
                    .unwrap_or_else(|| rsx! { GroupFields { tab: tab.clone(), tabs, status } }),
                (ResourceGroup::Simple(group), ManifestSelection::Record(index)) => group
                    .resources
                    .get(index)
                    .cloned()
                    .map(|resource| {
                        rsx! {
                            ResourceFields {
                                tab_id: tab.id,
                                group_index: tab.selected_group,
                                record_index: index,
                                resource,
                                tabs,
                                status,
                            }
                        }
                    })
                    .unwrap_or_else(|| rsx! { GroupFields { tab: tab.clone(), tabs, status } }),
                _ => rsx! { GroupFields { tab: tab.clone(), tabs, status } },
            });
    drop(manifest);
    rsx! {
        aside { class: "newton-panel newton-inspector-panel",
            PanelHeading {
                eyebrow: "INSPECTOR",
                title: "属性",
                action: rsx! {
                    span { class: "newton-dirty-indicator",
                        if tab.dirty { "已修改" } else { "已保存" }
                    }
                }
            }
            div { class: "newton-inspector-scroll",
                ManifestFields { tab: tab.clone(), tabs }
                if let Some(selected_fields) = selected_fields {
                    {selected_fields}
                }
                ValidationPanel {
                    report: tab.validation.clone(),
                    stale: tab.validation_stale,
                }
            }
        }
    }
}

#[component]
fn ManifestFields(tab: ManifestTab, tabs: Signal<Vec<ManifestTab>>) -> Element {
    let slot_count = read_manifest(&tab.manifest).slot_count;
    rsx! {
        FieldSection { title: "清单",
            NumberField {
                label: "槽位总数",
                value: slot_count.to_string(),
                min: Some(0),
                oninput: move |value: String| {
                    if let Ok(slot_count) = value.parse::<u32>() {
                        edit_manifest(tabs, tab.id, |manifest| manifest.slot_count = slot_count);
                    }
                }
            }
        }
    }
}

#[component]
fn GroupFields(
    tab: ManifestTab,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    let Some(group) = read_manifest(&tab.manifest)
        .groups
        .get(tab.selected_group)
        .cloned()
    else {
        return rsx! {};
    };
    let id = group.id().to_string();
    let resolution = optional_u32_text(group.resolution());
    let parent = group.parent().unwrap_or_default().to_string();
    let kind = if matches!(group, ResourceGroup::Composite(_)) {
        "复合资源组"
    } else {
        "简单资源组"
    };
    rsx! {
        FieldSection { title: "资源组",
            div { class: "newton-inspector-kind", "{kind}" }
            TextField {
                label: "ID",
                value: id,
                placeholder: "资源组 ID",
                oninput: move |value: String| edit_group(
                    tabs,
                    tab.id,
                    tab.selected_group,
                    move |group| match group {
                        ResourceGroup::Composite(group) => group.id = value,
                        ResourceGroup::Simple(group) => group.id = value,
                    },
                )
            }
            NumberField {
                label: "分辨率",
                value: resolution,
                min: Some(0),
                placeholder: "默认",
                oninput: move |value: String| {
                    let resolution = parse_optional_u32(&value);
                    edit_group(tabs, tab.id, tab.selected_group, move |group| match group {
                        ResourceGroup::Composite(group) => group.resolution = resolution,
                        ResourceGroup::Simple(group) => group.resolution = resolution,
                    });
                }
            }
            TextField {
                label: "父组",
                value: parent,
                placeholder: "无",
                oninput: move |value: String| {
                    let parent = nonempty(value);
                    edit_group(tabs, tab.id, tab.selected_group, move |group| match group {
                        ResourceGroup::Composite(group) => group.parent = parent,
                        ResourceGroup::Simple(group) => group.parent = parent,
                    });
                }
            }
            button {
                r#type: "button",
                class: "newton-danger-button",
                onclick: move |_| delete_group(tabs, tab.id, tab.selected_group, status),
                Glyph { name: "delete" }
                "删除资源组"
            }
        }
    }
}

#[component]
fn SubgroupFields(
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    subgroup: Subgroup,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    rsx! {
        FieldSection { title: "子组引用",
            TextField {
                label: "ID",
                value: subgroup.id,
                placeholder: "子组 ID",
                oninput: move |value: String| edit_subgroup(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    move |subgroup| subgroup.id = value,
                )
            }
            NumberField {
                label: "分辨率",
                value: optional_u32_text(subgroup.resolution),
                min: Some(0),
                placeholder: "默认",
                oninput: move |value: String| {
                    let resolution = parse_optional_u32(&value);
                    edit_subgroup(
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        move |subgroup| subgroup.resolution = resolution,
                    );
                }
            }
            button {
                r#type: "button",
                class: "newton-danger-button",
                onclick: move |_| delete_record(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    status,
                ),
                Glyph { name: "delete" }
                "删除子组引用"
            }
        }
    }
}

#[component]
fn ResourceFields(
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    resource: Resource,
    tabs: Signal<Vec<ManifestTab>>,
    status: Signal<AppStatus>,
) -> Element {
    let resource_for_type = resource.clone();
    let resource_for_atlas = resource.clone();
    rsx! {
        FieldSection { title: "资源",
            SelectField {
                label: "类型",
                value: resource_type_key(resource.resource_type).to_string(),
                options: resource_type_options(),
                onchange: move |value: String| {
                    if let Some(resource_type) = resource_type_from_key(&value) {
                        edit_resource(
                            tabs,
                            tab_id,
                            group_index,
                            record_index,
                            move |resource| resource.resource_type = resource_type,
                        );
                    }
                }
            }
            NumberField {
                label: "Slot",
                value: resource.slot.to_string(),
                min: Some(0),
                oninput: move |value: String| {
                    if let Ok(slot) = value.parse::<u32>() {
                        edit_resource(
                            tabs,
                            tab_id,
                            group_index,
                            record_index,
                            move |resource| resource.slot = slot,
                        );
                    }
                }
            }
            TextField {
                label: "ID",
                value: resource.id,
                placeholder: "资源 ID",
                oninput: move |value: String| edit_resource(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    move |resource| resource.id = value,
                )
            }
            TextField {
                label: "路径",
                value: resource.path,
                placeholder: "资源路径",
                oninput: move |value: String| edit_resource(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    move |resource| resource.path = value,
                )
            }
            TextField {
                label: "父图集",
                value: resource.parent.unwrap_or_default(),
                placeholder: "无",
                oninput: move |value: String| {
                    let parent = nonempty(value);
                    edit_resource(
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        move |resource| resource.parent = parent,
                    );
                }
            }
            ToggleField {
                label: "图集资源",
                checked: resource_for_atlas.atlas,
                onchange: move |checked| edit_resource(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    move |resource| resource.atlas = checked,
                )
            }
        }
        if resource_for_type.resource_type == ResourceType::Image {
            FieldSection { title: "图像与图集几何",
                div { class: "newton-field-grid two",
                    OptionalU32Field {
                        label: "Width",
                        value: resource_for_type.width,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::Width,
                    }
                    OptionalU32Field {
                        label: "Height",
                        value: resource_for_type.height,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::Height,
                    }
                    OptionalI32Field {
                        label: "X",
                        value: resource_for_type.x,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceSignedField::X,
                    }
                    OptionalI32Field {
                        label: "Y",
                        value: resource_for_type.y,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceSignedField::Y,
                    }
                    OptionalU32Field {
                        label: "Atlas X",
                        value: resource_for_type.ax,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::AtlasX,
                    }
                    OptionalU32Field {
                        label: "Atlas Y",
                        value: resource_for_type.ay,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::AtlasY,
                    }
                    OptionalU32Field {
                        label: "Atlas W",
                        value: resource_for_type.aw,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::AtlasWidth,
                    }
                    OptionalU32Field {
                        label: "Atlas H",
                        value: resource_for_type.ah,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::AtlasHeight,
                    }
                    OptionalU32Field {
                        label: "Columns",
                        value: resource_for_type.cols,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::Columns,
                    }
                    OptionalU32Field {
                        label: "Rows",
                        value: resource_for_type.rows,
                        tabs,
                        tab_id,
                        group_index,
                        record_index,
                        field: ResourceNumberField::Rows,
                    }
                }
            }
        }
        FieldSection { title: "记录操作",
            button {
                r#type: "button",
                class: "newton-danger-button",
                onclick: move |_| delete_record(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    status,
                ),
                Glyph { name: "delete" }
                "删除资源"
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceNumberField {
    Width,
    Height,
    AtlasX,
    AtlasY,
    AtlasWidth,
    AtlasHeight,
    Columns,
    Rows,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceSignedField {
    X,
    Y,
}

#[component]
fn OptionalU32Field(
    label: &'static str,
    value: Option<u32>,
    tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    field: ResourceNumberField,
) -> Element {
    rsx! {
        NumberField {
            label,
            value: optional_u32_text(value),
            min: Some(0),
            placeholder: "—",
            oninput: move |value: String| {
                let value = parse_optional_u32(&value);
                edit_resource(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    move |resource| match field {
                        ResourceNumberField::Width => resource.width = value,
                        ResourceNumberField::Height => resource.height = value,
                        ResourceNumberField::AtlasX => resource.ax = value,
                        ResourceNumberField::AtlasY => resource.ay = value,
                        ResourceNumberField::AtlasWidth => resource.aw = value,
                        ResourceNumberField::AtlasHeight => resource.ah = value,
                        ResourceNumberField::Columns => resource.cols = value,
                        ResourceNumberField::Rows => resource.rows = value,
                    },
                );
            }
        }
    }
}

#[component]
fn OptionalI32Field(
    label: &'static str,
    value: Option<i32>,
    tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    field: ResourceSignedField,
) -> Element {
    rsx! {
        NumberField {
            label,
            value: value.map(|value| value.to_string()).unwrap_or_default(),
            placeholder: "—",
            oninput: move |value: String| {
                let value = parse_optional_i32(&value);
                edit_resource(
                    tabs,
                    tab_id,
                    group_index,
                    record_index,
                    move |resource| match field {
                        ResourceSignedField::X => resource.x = value,
                        ResourceSignedField::Y => resource.y = value,
                    },
                );
            }
        }
    }
}

#[component]
fn ValidationPanel(report: ValidationReport, stale: bool) -> Element {
    let error_count = report.errors().count();
    let warning_count = report.warnings().count();
    let visible_issues = report.issues.iter().take(8).cloned().collect::<Vec<_>>();
    let validation_dot_class = if error_count > 0 {
        "newton-validation-dot error"
    } else if warning_count > 0 || stale {
        "newton-validation-dot warning"
    } else {
        "newton-validation-dot ok"
    };
    rsx! {
        section { class: "newton-validation-panel",
            div { class: "newton-validation-heading",
                div {
                    span { class: "newton-field-section-label", "校验" }
                    strong {
                        if stale {
                            "内容已变化，需要重新校验"
                        } else if error_count == 0 && warning_count == 0 {
                            "清单符合规范"
                        } else {
                            "{error_count} 个错误 · {warning_count} 个警告"
                        }
                    }
                }
                span { class: validation_dot_class }
            }
            if !visible_issues.is_empty() {
                div { class: "newton-issue-list",
                    for issue in visible_issues {
                        ValidationIssueRow { issue }
                    }
                }
            }
        }
    }
}

#[component]
fn ValidationIssueRow(issue: ValidationIssue) -> Element {
    let tone = match issue.severity {
        ValidationSeverity::Error => "error",
        ValidationSeverity::Warning => "warning",
    };
    rsx! {
        div { class: "newton-issue {tone}",
            span {}
            div {
                strong { "{issue.path}" }
                p { "{issue.message}" }
            }
        }
    }
}

#[component]
fn FieldSection(title: &'static str, children: Element) -> Element {
    rsx! {
        section { class: "newton-field-section",
            span { class: "newton-field-section-label", "{title}" }
            {children}
        }
    }
}

#[component]
fn TextField(
    label: &'static str,
    value: String,
    #[props(default = "")] placeholder: &'static str,
    oninput: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "newton-form-field",
            span { "{label}" }
            input {
                r#type: "text",
                value,
                placeholder,
                oninput: move |event| oninput.call(event.value()),
            }
        }
    }
}

#[component]
fn NumberField(
    label: &'static str,
    value: String,
    #[props(default)] min: Option<i64>,
    #[props(default = "")] placeholder: &'static str,
    oninput: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "newton-form-field",
            span { "{label}" }
            input {
                r#type: "number",
                value,
                min,
                placeholder,
                oninput: move |event| oninput.call(event.value()),
            }
        }
    }
}

#[component]
fn SelectField(
    label: &'static str,
    value: String,
    options: Vec<(&'static str, &'static str)>,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "newton-form-field",
            span { "{label}" }
            select {
                value,
                onchange: move |event| onchange.call(event.value()),
                for (option_value, option_label) in options {
                    option { value: option_value, "{option_label}" }
                }
            }
        }
    }
}

#[component]
fn ToggleField(label: &'static str, checked: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "newton-toggle-field",
            span { "{label}" }
            input {
                r#type: "checkbox",
                checked,
                onchange: move |event| onchange.call(event.checked()),
            }
            i {}
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: String, action: Element) -> Element {
    rsx! {
        header { class: "newton-panel-heading",
            div {
                span { class: "newton-eyebrow", "{eyebrow}" }
                h2 { title: "{title}", "{title}" }
            }
            {action}
        }
    }
}

#[component]
fn CloseDialog(on_cancel: EventHandler<()>, on_discard: EventHandler<()>) -> Element {
    rsx! {
        div { class: "newton-dialog-layer",
            button {
                class: "newton-dialog-backdrop",
                aria_label: "取消关闭",
                onclick: move |_| on_cancel.call(()),
            }
            section { class: "newton-dialog", role: "dialog", aria_modal: "true",
                span { class: "newton-dialog-icon", Glyph { name: "warning" } }
                h2 { "关闭已修改的清单？" }
                p { "尚未导出的修改将会丢失。" }
                div {
                    button {
                        r#type: "button",
                        onclick: move |_| on_cancel.call(()),
                        "取消"
                    }
                    button {
                        r#type: "button",
                        class: "danger",
                        onclick: move |_| on_discard.call(()),
                        "放弃修改"
                    }
                }
            }
        }
    }
}

#[component]
fn Glyph(name: String) -> Element {
    let path = match name.as_str() {
        "open" => "M3 7h5l2 2h11v10H3V7Zm0 0V5h6l2 2",
        "new" => "M14 2H6a2 2 0 0 0-2 2v16h16V8l-6-6Zm0 0v6h6M12 12v6m-3-3h6",
        "save" => "M12 3v12m0 0 5-5m-5 5-5-5M4 20h16",
        "validate" => "m5 12 4 4L19 6",
        "warning" => "M12 3 2 21h20L12 3Zm0 6v5m0 3h.01",
        "composite" => "M4 5h7v6H4V5Zm9 8h7v6h-7v-6Zm-2-5h4v5m-6-2v5h4",
        "group" => "M4 5h16v14H4V5Zm0 4h16",
        "records" => "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
        "slot" => "M5 4h14v16H5V4Zm4 0v16m6-16v16",
        "manifest" => "M14 2H6a2 2 0 0 0-2 2v16h16V8l-6-6Zm0 0v6h6M8 13h8M8 17h6",
        "code" => "m8 9-3 3 3 3m8-6 3 3-3 3m-2-9-4 18",
        "search" => "m21 21-4.35-4.35M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z",
        "add" => "M12 5v14M5 12h14",
        "close" => "m6 6 12 12M18 6 6 18",
        "delete" => "M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6",
        "chevron" => "m9 18 6-6-6-6",
        "left" => "m15 18-6-6 6-6",
        "right" => "m9 18 6-6-6-6",
        _ => "",
    };
    rsx! {
        svg {
            class: "newton-glyph",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: path }
        }
    }
}

fn open_manifest_bytes(
    name: String,
    bytes: Arc<[u8]>,
    mut tabs: Signal<Vec<ManifestTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    mut next_tab_id: Signal<u64>,
    mut status: Signal<AppStatus>,
) {
    let format = ManifestFormat::from_file_name(&name);
    match decode_manifest(format, &bytes) {
        Ok(manifest) => {
            let id = next_tab_id();
            next_tab_id.set(id.wrapping_add(1).max(1));
            let group_count = manifest.groups.len();
            tabs.write()
                .push(ManifestTab::opened(id, name.clone(), manifest));
            active_tab_id.set(Some(id));
            push_application_log(
                "NEWTON",
                "INFO",
                "OPEN",
                format!(
                    "Opened {name} as {} ({group_count} groups, {} bytes)",
                    format.label(),
                    bytes.len()
                ),
            );
            status.set(AppStatus::new(
                format!("已打开 {name}，包含 {group_count} 个资源组。"),
                StatusTone::Success,
            ));
        }
        Err(error) => {
            push_application_log(
                "NEWTON",
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

fn load_manifest_files(
    files: Vec<FileData>,
    tabs: Signal<Vec<ManifestTab>>,
    active_tab_id: Signal<Option<u64>>,
    next_tab_id: Signal<u64>,
    status: Signal<AppStatus>,
) {
    spawn(async move {
        for file in files {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => open_manifest_bytes(
                    name,
                    Arc::from(bytes.to_vec()),
                    tabs,
                    active_tab_id,
                    next_tab_id,
                    status,
                ),
                Err(error) => {
                    let mut status = status;
                    status.set(AppStatus::new(
                        format!("无法读取文件：{error}"),
                        StatusTone::Error,
                    ));
                }
            }
        }
    });
}

fn create_blank_tab(
    mut tabs: Signal<Vec<ManifestTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    mut next_tab_id: Signal<u64>,
    mut status: Signal<AppStatus>,
) {
    let id = next_tab_id();
    next_tab_id.set(id.wrapping_add(1).max(1));
    let mut tab = ManifestTab::blank(id);
    tab.dirty = true;
    tabs.write().push(tab);
    active_tab_id.set(Some(id));
    status.set(AppStatus::new(
        "已创建空白 NEWTON 清单。",
        StatusTone::Neutral,
    ));
}

fn close_tab(
    mut tabs: Signal<Vec<ManifestTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    tab_id: u64,
) {
    let mut tabs = tabs.write();
    let Some(index) = tabs.iter().position(|tab| tab.id == tab_id) else {
        return;
    };
    let was_active = active_tab_id() == Some(tab_id);
    tabs.remove(index);
    if was_active {
        let next = tabs
            .get(index)
            .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
            .map(|tab| tab.id);
        active_tab_id.set(next);
    }
}

fn update_tab_state(
    mut tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    update: impl FnOnce(&mut ManifestTab),
) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        update(tab);
    }
}

fn edit_manifest(
    mut tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    update: impl FnOnce(&mut ResourceManifest),
) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        {
            let mut manifest = write_manifest(&tab.manifest);
            update(&mut manifest);
        }
        tab.revision = tab.revision.wrapping_add(1);
        tab.dirty = true;
        tab.validation_stale = true;
    }
}

fn edit_group(
    tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    update: impl FnOnce(&mut ResourceGroup),
) {
    edit_manifest(tabs, tab_id, |manifest| {
        if let Some(group) = manifest.groups.get_mut(group_index) {
            update(group);
        }
    });
}

fn edit_subgroup(
    tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    update: impl FnOnce(&mut Subgroup),
) {
    edit_group(tabs, tab_id, group_index, |group| {
        if let ResourceGroup::Composite(group) = group
            && let Some(subgroup) = group.subgroups.get_mut(record_index)
        {
            update(subgroup);
        }
    });
}

fn edit_resource(
    tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    update: impl FnOnce(&mut Resource),
) {
    edit_group(tabs, tab_id, group_index, |group| {
        if let ResourceGroup::Simple(group) = group
            && let Some(resource) = group.resources.get_mut(record_index)
        {
            update(resource);
        }
    });
}

fn add_group(
    tabs: Signal<Vec<ManifestTab>>,
    active_tab_id: Signal<Option<u64>>,
    composite: bool,
    status: Signal<AppStatus>,
) {
    if let Some(tab_id) = active_tab_id() {
        add_group_by_tab(tabs, tab_id, composite, status);
    }
}

fn add_group_by_tab(
    mut tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    composite: bool,
    mut status: Signal<AppStatus>,
) {
    let mut tabs_write = tabs.write();
    let Some(tab) = tabs_write.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    let mut manifest = write_manifest(&tab.manifest);
    let index = manifest.groups.len();
    let id = unique_group_id(
        &manifest,
        if composite {
            "NEW_COMPOSITE"
        } else {
            "NEW_GROUP"
        },
    );
    let group = if composite {
        ResourceGroup::Composite(CompositeGroup {
            id,
            resolution: None,
            parent: None,
            subgroups: Vec::new(),
        })
    } else {
        ResourceGroup::Simple(SimpleGroup {
            id,
            resolution: None,
            parent: None,
            resources: Vec::new(),
        })
    };
    manifest.groups.push(group);
    drop(manifest);
    tab.selected_group = index;
    tab.selection = ManifestSelection::Group;
    tab.group_page = index / GROUP_PAGE_SIZE;
    tab.record_query.clear();
    tab.record_page = 0;
    tab.revision = tab.revision.wrapping_add(1);
    tab.dirty = true;
    tab.validation_stale = true;
    drop(tabs_write);
    status.set(AppStatus::new("已添加资源组。", StatusTone::Success));
}

fn delete_group(
    mut tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    mut status: Signal<AppStatus>,
) {
    let mut tabs_write = tabs.write();
    let Some(tab) = tabs_write.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    let mut manifest = write_manifest(&tab.manifest);
    if group_index >= manifest.groups.len() {
        return;
    }
    manifest.groups.remove(group_index);
    tab.selected_group = group_index.min(manifest.groups.len().saturating_sub(1));
    drop(manifest);
    tab.group_page = tab.selected_group / GROUP_PAGE_SIZE;
    tab.selection = ManifestSelection::Group;
    tab.record_page = 0;
    tab.revision = tab.revision.wrapping_add(1);
    tab.dirty = true;
    tab.validation_stale = true;
    drop(tabs_write);
    status.set(AppStatus::new("资源组已删除。", StatusTone::Warning));
}

fn add_record(
    mut tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    mut status: Signal<AppStatus>,
) {
    let mut tabs_write = tabs.write();
    let Some(tab) = tabs_write.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    let mut manifest = write_manifest(&tab.manifest);
    let next_slot = manifest.slot_count;
    let Some(group) = manifest.groups.get_mut(group_index) else {
        return;
    };
    let (index, message) = match group {
        ResourceGroup::Composite(group) => {
            let index = group.subgroups.len();
            group.subgroups.push(Subgroup {
                id: format!("NEW_SUBGROUP_{}", index + 1),
                resolution: None,
            });
            (index, "已添加子组引用。")
        }
        ResourceGroup::Simple(group) => {
            let index = group.resources.len();
            group.resources.push(Resource {
                resource_type: ResourceType::File,
                slot: next_slot,
                id: format!("NEW_RESOURCE_{}", index + 1),
                path: String::new(),
                width: None,
                height: None,
                x: None,
                y: None,
                ax: None,
                ay: None,
                aw: None,
                ah: None,
                cols: None,
                rows: None,
                atlas: false,
                parent: None,
            });
            manifest.slot_count = next_slot.saturating_add(1);
            (index, "已添加资源记录。")
        }
    };
    drop(manifest);
    tab.selection = ManifestSelection::Record(index);
    tab.revision = tab.revision.wrapping_add(1);
    tab.dirty = true;
    tab.validation_stale = true;
    drop(tabs_write);
    status.set(AppStatus::new(message, StatusTone::Success));
}

fn delete_record(
    mut tabs: Signal<Vec<ManifestTab>>,
    tab_id: u64,
    group_index: usize,
    record_index: usize,
    mut status: Signal<AppStatus>,
) {
    let mut tabs_write = tabs.write();
    let Some(tab) = tabs_write.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    let mut manifest = write_manifest(&tab.manifest);
    let Some(group) = manifest.groups.get_mut(group_index) else {
        return;
    };
    let deleted = match group {
        ResourceGroup::Composite(group) if record_index < group.subgroups.len() => {
            group.subgroups.remove(record_index);
            true
        }
        ResourceGroup::Simple(group) if record_index < group.resources.len() => {
            group.resources.remove(record_index);
            true
        }
        _ => false,
    };
    drop(manifest);
    if deleted {
        tab.selection = ManifestSelection::Group;
        tab.revision = tab.revision.wrapping_add(1);
        tab.dirty = true;
        tab.validation_stale = true;
    }
    drop(tabs_write);
    if deleted {
        status.set(AppStatus::new("记录已删除。", StatusTone::Warning));
    }
}

fn validate_active_tab(
    mut tabs: Signal<Vec<ManifestTab>>,
    active_tab_id: Signal<Option<u64>>,
    mut status: Signal<AppStatus>,
) {
    let Some(tab_id) = active_tab_id() else {
        return;
    };
    let mut tabs_write = tabs.write();
    let Some(tab) = tabs_write.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };
    tab.validation = read_manifest(&tab.manifest).validate(ValidationProfile::Canonical);
    tab.validation_stale = false;
    let errors = tab.validation.errors().count();
    let warnings = tab.validation.warnings().count();
    drop(tabs_write);
    status.set(AppStatus::new(
        if errors == 0 && warnings == 0 {
            "清单校验通过。".to_string()
        } else {
            format!("校验完成：{errors} 个错误，{warnings} 个警告。")
        },
        if errors > 0 {
            StatusTone::Error
        } else if warnings > 0 {
            StatusTone::Warning
        } else {
            StatusTone::Success
        },
    ));
}

fn save_active_tab(
    tabs: Signal<Vec<ManifestTab>>,
    active_tab_id: Signal<Option<u64>>,
    status: Signal<AppStatus>,
    format: ManifestFormat,
) {
    let Some(tab_id) = active_tab_id() else {
        return;
    };
    let Some(tab) = tabs.peek().iter().find(|tab| tab.id == tab_id).cloned() else {
        return;
    };
    let bytes = match encode_manifest(format, &read_manifest(&tab.manifest)) {
        Ok(bytes) => bytes,
        Err(error) => {
            let mut status = status;
            status.set(AppStatus::new(
                format!("无法编码 {}：{error}", format.label()),
                StatusTone::Error,
            ));
            return;
        }
    };
    let file_name = manifest_export_name(&tab.name, format);
    let mut status = status;
    status.set(AppStatus::new(
        format!("正在导出 {}…", format.label()),
        StatusTone::Neutral,
    ));
    spawn(async move {
        match platform::save_bytes(
            &file_name,
            format.filter_name(),
            format.filter_extensions(),
            &bytes,
        )
        .await
        {
            Ok(true) => {
                if format == ManifestFormat::Newton {
                    update_tab_state(tabs, tab_id, |tab| {
                        tab.dirty = false;
                        tab.validation =
                            read_manifest(&tab.manifest).validate(ValidationProfile::Canonical);
                        tab.validation_stale = false;
                    });
                }
                push_application_log(
                    "NEWTON",
                    "INFO",
                    "EXPORT",
                    format!(
                        "Exported {file_name} as {} ({} bytes)",
                        format.label(),
                        bytes.len()
                    ),
                );
                status.set(AppStatus::new(
                    format!("{} 已导出。", format.label()),
                    StatusTone::Success,
                ));
            }
            Ok(false) => {
                status.set(AppStatus::default());
            }
            Err(error) => {
                push_application_log(
                    "NEWTON",
                    "ERROR",
                    "EXPORT",
                    format!("Export {file_name} failed: {error}"),
                );
                status.set(AppStatus::new(error, StatusTone::Error));
            }
        }
    });
}

fn decode_manifest(format: ManifestFormat, bytes: &[u8]) -> Result<ResourceManifest, String> {
    match format {
        ManifestFormat::Newton => from_bytes(bytes).map_err(|error| error.to_string()),
        ManifestFormat::Json => {
            serde_json::from_slice(bytes).map_err(|error| format!("JSON 解析失败：{error}"))
        }
        ManifestFormat::Yaml => {
            serde_yaml::from_slice(bytes).map_err(|error| format!("YAML 解析失败：{error}"))
        }
        ManifestFormat::Toml => {
            let text =
                std::str::from_utf8(bytes).map_err(|error| format!("TOML 不是 UTF-8：{error}"))?;
            toml::from_str(text).map_err(|error| format!("TOML 解析失败：{error}"))
        }
    }
}

fn encode_manifest(format: ManifestFormat, manifest: &ResourceManifest) -> Result<Vec<u8>, String> {
    match format {
        ManifestFormat::Newton => to_bytes(manifest).map_err(|error| error.to_string()),
        ManifestFormat::Json => {
            let mut bytes = serde_json::to_vec_pretty(manifest)
                .map_err(|error| format!("JSON 序列化失败：{error}"))?;
            bytes.push(b'\n');
            Ok(bytes)
        }
        ManifestFormat::Yaml => serde_yaml::to_string(manifest)
            .map(String::into_bytes)
            .map_err(|error| format!("YAML 序列化失败：{error}")),
        ManifestFormat::Toml => toml::to_string_pretty(manifest)
            .map(String::into_bytes)
            .map_err(|error| format!("TOML 序列化失败：{error}")),
    }
}

fn manifest_export_name(source_name: &str, format: ManifestFormat) -> String {
    let leaf = source_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source_name);
    let stem = leaf
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("RESOURCES");
    format!("{stem}.{}", format.export_extension())
}

fn unique_group_id(manifest: &ResourceManifest, base: &str) -> String {
    if !manifest.groups.iter().any(|group| group.id() == base) {
        return base.to_string();
    }
    (2..)
        .map(|index| format!("{base}_{index}"))
        .find(|candidate| !manifest.groups.iter().any(|group| group.id() == candidate))
        .unwrap_or_else(|| format!("{base}_NEW"))
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn optional_u32_text(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn parse_optional_u32(value: &str) -> Option<u32> {
    (!value.trim().is_empty())
        .then(|| value.parse::<u32>().ok())
        .flatten()
}

fn parse_optional_i32(value: &str) -> Option<i32> {
    (!value.trim().is_empty())
        .then(|| value.parse::<i32>().ok())
        .flatten()
}

fn resource_type_label(resource_type: ResourceType) -> &'static str {
    match resource_type {
        ResourceType::Image => "Image",
        ResourceType::PopAnim => "PopAnim",
        ResourceType::SoundBank => "SoundBank",
        ResourceType::File => "File",
        ResourceType::PrimeFont => "PrimeFont",
        ResourceType::RenderEffect => "RenderEffect",
        ResourceType::DecodedSoundBank => "DecodedSoundBank",
    }
}

fn resource_type_short(resource_type: ResourceType) -> &'static str {
    match resource_type {
        ResourceType::Image => "IMG",
        ResourceType::PopAnim => "PAM",
        ResourceType::SoundBank => "BNK",
        ResourceType::File => "FILE",
        ResourceType::PrimeFont => "FONT",
        ResourceType::RenderEffect => "FX",
        ResourceType::DecodedSoundBank => "WEM",
    }
}

fn resource_type_key(resource_type: ResourceType) -> &'static str {
    resource_type_label(resource_type)
}

fn resource_type_from_key(value: &str) -> Option<ResourceType> {
    match value {
        "Image" => Some(ResourceType::Image),
        "PopAnim" => Some(ResourceType::PopAnim),
        "SoundBank" => Some(ResourceType::SoundBank),
        "File" => Some(ResourceType::File),
        "PrimeFont" => Some(ResourceType::PrimeFont),
        "RenderEffect" => Some(ResourceType::RenderEffect),
        "DecodedSoundBank" => Some(ResourceType::DecodedSoundBank),
        _ => None,
    }
}

fn resource_type_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Image", "Image"),
        ("PopAnim", "PopAnim"),
        ("SoundBank", "SoundBank"),
        ("File", "File"),
        ("PrimeFont", "PrimeFont"),
        ("RenderEffect", "RenderEffect"),
        ("DecodedSoundBank", "DecodedSoundBank"),
    ]
}

fn manifest_tab_class(active: bool, dirty: bool) -> &'static str {
    match (active, dirty) {
        (true, true) => "newton-tab ui-document-tab is-active is-dirty",
        (true, false) => "newton-tab ui-document-tab is-active",
        (false, true) => "newton-tab ui-document-tab is-dirty",
        (false, false) => "newton-tab ui-document-tab",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ResourceManifest {
        ResourceManifest {
            slot_count: 2,
            groups: vec![
                ResourceGroup::Composite(CompositeGroup {
                    id: "ManifestGroup".into(),
                    resolution: Some(1536),
                    parent: None,
                    subgroups: vec![Subgroup {
                        id: "ManifestGroup_1536".into(),
                        resolution: Some(1536),
                    }],
                }),
                ResourceGroup::Simple(SimpleGroup {
                    id: "ManifestGroup_1536".into(),
                    resolution: Some(1536),
                    parent: Some("ManifestGroup".into()),
                    resources: vec![Resource {
                        resource_type: ResourceType::Image,
                        slot: 1,
                        id: "IMAGE_TEST".into(),
                        path: "images/test.ptx".into(),
                        width: Some(64),
                        height: Some(32),
                        x: Some(-2),
                        y: Some(3),
                        ax: Some(4),
                        ay: Some(5),
                        aw: Some(60),
                        ah: Some(28),
                        cols: Some(1),
                        rows: Some(1),
                        atlas: true,
                        parent: Some("IMAGE_ATLAS".into()),
                    }],
                }),
            ],
        }
    }

    #[test]
    fn every_export_format_round_trips_the_semantic_manifest() {
        let expected = sample_manifest();
        for format in ManifestFormat::TEXT {
            let encoded = encode_manifest(format, &expected)
                .unwrap_or_else(|error| panic!("{} encode failed: {error}", format.label()));
            let decoded = decode_manifest(format, &encoded)
                .unwrap_or_else(|error| panic!("{} decode failed: {error}", format.label()));
            assert_eq!(decoded, expected, "{} round trip", format.label());
        }
    }

    #[test]
    fn format_detection_and_export_names_follow_extensions() {
        assert_eq!(
            ManifestFormat::from_file_name("RESOURCES.NEWTON"),
            ManifestFormat::Newton
        );
        assert_eq!(
            ManifestFormat::from_file_name("resources.JSON"),
            ManifestFormat::Json
        );
        assert_eq!(
            ManifestFormat::from_file_name("resources.yml"),
            ManifestFormat::Yaml
        );
        assert_eq!(
            ManifestFormat::from_file_name("resources.toml"),
            ManifestFormat::Toml
        );
        assert_eq!(
            manifest_export_name("folder/RESOURCES.NEWTON", ManifestFormat::Yaml),
            "RESOURCES.yaml"
        );
    }
}
