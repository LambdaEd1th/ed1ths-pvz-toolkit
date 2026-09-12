use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use rsb_patch_worker::{
    ApplyRequest, CreateRequest, InspectRequest, PatchMode, PatchRecordSummary, PatchSummary,
};
use toolkit_ui::{
    DropIndicator, InlineNotice, ToolPage, ToolPageToolbar, WorkspaceCard, push_application_log,
};

use crate::{platform, processing};

const RSB_PATCH_PAGE_CSS: Asset = asset!("/assets/rsb-patch/page.css");
const PATCH_ACCEPT: &str = ".rsbpatch,application/octet-stream";
const ARCHIVE_ACCEPT: &str = ".rsb,.obb,application/octet-stream";
const RECORD_PAGE_SIZE: usize = 500;

#[derive(Clone)]
struct NamedBytes {
    name: String,
    data: Arc<[u8]>,
}

impl PartialEq for NamedBytes {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Arc::ptr_eq(&self.data, &other.data)
    }
}

#[derive(Clone)]
struct PatchTab {
    id: u64,
    name: String,
    data: Arc<[u8]>,
    summary: Arc<PatchSummary>,
    query: String,
    selected_record: Option<usize>,
    row_limit: usize,
}

impl PartialEq for PatchTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && Arc::ptr_eq(&self.data, &other.data)
            && Arc::ptr_eq(&self.summary, &other.summary)
            && self.query == other.query
            && self.selected_record == other.selected_record
            && self.row_limit == other.row_limit
    }
}

impl PatchTab {
    fn new(id: u64, name: String, data: Vec<u8>, summary: PatchSummary) -> Self {
        Self {
            id,
            name,
            data: Arc::from(data),
            summary: Arc::new(summary),
            query: String::new(),
            selected_record: None,
            row_limit: RECORD_PAGE_SIZE,
        }
    }
}

#[derive(Clone, Default, PartialEq)]
struct CreateDraft {
    before: Option<NamedBytes>,
    after: Option<NamedBytes>,
    mode: PatchMode,
}

#[derive(Clone, Default, PartialEq)]
struct ApplyDraft {
    before: Option<NamedBytes>,
    patch: Option<NamedBytes>,
    mode: PatchMode,
}

#[derive(Clone, PartialEq)]
enum WorkflowDialog {
    Create(CreateDraft),
    Apply(ApplyDraft),
}

#[derive(Clone, Copy)]
enum DraftTarget {
    CreateBefore,
    CreateAfter,
    ApplyBefore,
    ApplyPatch,
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

#[derive(Clone, Copy, PartialEq)]
struct WorkspaceSignals {
    tabs: Signal<Vec<PatchTab>>,
    active_tab_id: Signal<Option<u64>>,
    next_tab_id: Signal<u64>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
}

#[component]
pub fn RsbPatchPage() -> Element {
    let tabs = use_signal(Vec::<PatchTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(|| {
        AppStatus::new(
            "打开 RSBP，或创建、应用一个 RSB 差分补丁。",
            StatusTone::Neutral,
        )
    });
    let mut dialog = use_signal(|| None::<WorkflowDialog>);
    let signals = WorkspaceSignals {
        tabs,
        active_tab_id,
        next_tab_id,
        busy,
        status,
    };
    toolkit_ui::use_tool_open(toolkit_ui::ToolKind::RsbPatch, busy, move |files| {
        let files = files
            .into_iter()
            .map(toolkit_ui::ToolFile::into_file_data)
            .collect();
        open_patch_files(files, signals);
    });
    let tabs_snapshot = tabs();
    let active_id = active_tab_id();
    let active_tab = active_id
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let active_for_apply = active_tab.clone();
    let active_for_export = active_tab.clone();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: RSB_PATCH_PAGE_CSS }
        div {
            class: if dragging() { "rsp-page-host is-dragging" } else { "rsp-page-host" },
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
                if !files.is_empty() { open_patch_files(files, signals); }
            },
            ToolPage {
                namespace: "rsb-patch",
                class: if tabs_snapshot.is_empty() { "rsp-page is-empty" } else { "rsp-page" },
                ToolPageToolbar { class: "rsp-toolbar",
                    actions: rsx! {
                        Toolbar {
                            active: active_tab.clone(),
                            busy: busy(),
                            on_open: move |files| open_patch_files(files, signals),
                            on_create: move |_| dialog.set(Some(WorkflowDialog::Create(CreateDraft::default()))),
                            on_apply: move |_| {
                                let patch = active_for_apply.as_ref().map(|tab| NamedBytes {
                                    name: tab.name.clone(),
                                    data: Arc::clone(&tab.data),
                                });
                                dialog.set(Some(WorkflowDialog::Apply(ApplyDraft {
                                    patch,
                                    ..ApplyDraft::default()
                                })));
                            },
                            on_export: move |_| {
                                if let Some(tab) = active_for_export.clone() {
                                    export_patch(tab, status);
                                }
                            },
                        }
                    }
                }
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
                        on_files: move |files| open_patch_files(files, signals),
                    }
                }
                WorkspaceCard { class: "rsp-workspace-card", aria_label: "RSB Patch",
                    if let Some(tab) = active_tab {
                        PatchWorkspace { tab, tabs, active_tab_id }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_open: move |files| open_patch_files(files, signals),
                            on_create: move |_| dialog.set(Some(WorkflowDialog::Create(CreateDraft::default()))),
                            on_apply: move |_| dialog.set(Some(WorkflowDialog::Apply(ApplyDraft::default()))),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "rsp-status",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "松开以打开 RSBP 补丁" }
                    }
                    if busy() {
                        div { class: "rsp-busy", role: "status",
                            span {}
                            "正在后台处理 RSBP…"
                        }
                    }
                }
            }
            if let Some(workflow) = dialog() {
                WorkflowSheet {
                    workflow,
                    busy: busy(),
                    on_close: move |_| if !busy() { dialog.set(None); },
                    on_file: move |(target, file)| load_draft_file(file, target, dialog, busy, status),
                    on_mode: move |mode| set_dialog_mode(dialog, mode),
                    on_submit: move |_| submit_workflow(dialog, signals),
                }
            }
        }
    }
}

#[component]
fn Toolbar(
    active: Option<PatchTab>,
    busy: bool,
    on_open: EventHandler<Vec<FileData>>,
    on_create: EventHandler<MouseEvent>,
    on_apply: EventHandler<MouseEvent>,
    on_export: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "ui-island ui-tool-page-actions rsp-toolbar-actions",
            label {
                class: if busy { "rsp-icon-button is-disabled" } else { "rsp-icon-button" },
                title: "打开 RSBP",
                aria_label: "打开 RSBP",
                Glyph { name: "open" }
                input {
                    class: "rsp-file-input",
                    r#type: "file",
                    accept: PATCH_ACCEPT,
                    multiple: true,
                    disabled: busy,
                    onchange: move |event| {
                        let files = event.files();
                        if !files.is_empty() { on_open.call(files); }
                    }
                }
            }
            button {
                class: "rsp-icon-button",
                title: "创建补丁",
                aria_label: "创建补丁",
                disabled: busy,
                onclick: move |event| on_create.call(event),
                Glyph { name: "create" }
            }
            button {
                class: "rsp-icon-button",
                title: "应用补丁",
                aria_label: "应用补丁",
                disabled: busy,
                onclick: move |event| on_apply.call(event),
                Glyph { name: "apply" }
            }
            div { class: "rsp-toolbar-spacer" }
            if let Some(tab) = active {
                span { class: "rsp-format-pill", "RSBP · {tab.summary.packet_count} RSG" }
                button {
                    class: "rsp-icon-button primary",
                    title: "导出补丁",
                    aria_label: "导出补丁",
                    disabled: busy,
                    onclick: move |event| on_export.call(event),
                    Glyph { name: "download" }
                }
            }
        }
    }
}

#[component]
fn TabStrip(
    tabs: Vec<PatchTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "rsp-tab-strip ui-document-tab-strip",
            div { class: "rsp-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 RSBP 文件",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: if Some(tab.id) == active_tab_id { "rsp-tab ui-document-tab is-active" } else { "rsp-tab ui-document-tab" },
                        button {
                            class: "rsp-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            title: "{tab.name}",
                            disabled: busy,
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "rsp-tab-dot ui-document-tab-dot" }
                            span { class: "rsp-tab-name ui-document-tab-name", "{tab.name}" }
                        }
                        button {
                            class: "rsp-tab-close ui-document-tab-close",
                            title: "关闭 {tab.name}",
                            aria_label: "关闭 {tab.name}",
                            disabled: busy,
                            onclick: move |_| on_close.call(tab.id),
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: if busy { "rsp-tab-add ui-document-new-tab is-disabled" } else { "rsp-tab-add ui-document-new-tab" },
                    title: "打开 RSBP",
                    aria_label: "打开 RSBP",
                    Glyph { name: "add" }
                    input {
                        class: "rsp-file-input",
                        r#type: "file",
                        accept: PATCH_ACCEPT,
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
    on_open: EventHandler<Vec<FileData>>,
    on_create: EventHandler<MouseEvent>,
    on_apply: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section { class: "rsp-empty",
            div { class: "rsp-empty-mark", aria_hidden: "true",
                span { "RSB" }
                i { Glyph { name: "difference" } }
                span { class: "is-patch", "RSBP" }
            }
            span { class: "rsp-eyebrow", "RESOURCE STREAM BUNDLE PATCH" }
            h2 { "创建、检查或应用 RSB 补丁" }
            p { "补丁使用规范化 RSBP 容器与 VCDIFF，可选择直接处理已压缩 RSG，或使用 raw packet 模式。" }
            div { class: "rsp-empty-actions",
                label { class: if busy { "rsp-primary is-disabled" } else { "rsp-primary" },
                    Glyph { name: "open" }
                    "打开补丁"
                    input {
                        class: "rsp-file-input",
                        r#type: "file",
                        accept: PATCH_ACCEPT,
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() { on_open.call(files); }
                        }
                    }
                }
                button { class: "rsp-secondary", disabled: busy, onclick: move |event| on_create.call(event),
                    Glyph { name: "create" }
                    "创建补丁"
                }
                button { class: "rsp-secondary", disabled: busy, onclick: move |event| on_apply.call(event),
                    Glyph { name: "apply" }
                    "应用补丁"
                }
            }
        }
    }
}

#[component]
fn PatchWorkspace(
    tab: PatchTab,
    mut tabs: Signal<Vec<PatchTab>>,
    active_tab_id: Signal<Option<u64>>,
) -> Element {
    let summary = &tab.summary;
    let query = tab.query.trim().to_ascii_lowercase();
    let matching = summary
        .records
        .iter()
        .enumerate()
        .filter(|(_, record)| query.is_empty() || record.name.to_ascii_lowercase().contains(&query))
        .collect::<Vec<_>>();
    let matching_count = matching.len();
    let visible = matching.into_iter().take(tab.row_limit).collect::<Vec<_>>();
    let selected = tab
        .selected_record
        .and_then(|index| summary.records.get(index));
    rsx! {
        div { class: "rsp-document",
            section { class: "rsp-summary-grid",
                SummaryCard { label: "补丁大小", value: format_bytes(summary.container_size), detail: format!("VCDIFF {}", format_bytes(summary.total_delta_size)), icon: "archive" }
                SummaryCard { label: "目标归档", value: format_bytes(summary.target_archive_size), detail: "重建后的有效大小".to_string(), icon: "target" }
                SummaryCard { label: "RSG 记录", value: summary.packet_count.to_string(), detail: format!("{} 个发生变化", summary.changed_packet_count), icon: "packets" }
                SummaryCard { label: "信息区", value: if summary.information_changed { "已变化".to_string() } else { "未变化".to_string() }, detail: format_bytes(summary.information_delta_size), icon: "info" }
            }
            div { class: "rsp-content-grid",
                section { class: "rsp-record-card",
                    header { class: "rsp-record-header",
                        div {
                            strong { "Packet records" }
                            span { "{matching_count} / {summary.packet_count}" }
                        }
                        label { class: "rsp-search",
                            Glyph { name: "search" }
                            input {
                                value: "{tab.query}",
                                placeholder: "搜索 RSG…",
                                oninput: move |event| update_active_tab(tabs, active_tab_id, |tab| {
                                    tab.query = event.value();
                                    tab.row_limit = RECORD_PAGE_SIZE;
                                    tab.selected_record = None;
                                }),
                            }
                        }
                    }
                    div { class: "rsp-record-table",
                        div { class: "rsp-record-row rsp-record-row--head",
                            span { "RSG" }
                            span { "状态" }
                            span { "Delta" }
                        }
                        for (index, record) in visible {
                            RecordRow {
                                key: "{index}",
                                record: record.clone(),
                                selected: tab.selected_record == Some(index),
                                on_select: move |_| update_active_tab(tabs, active_tab_id, |tab| tab.selected_record = Some(index)),
                            }
                        }
                    }
                    if matching_count > tab.row_limit {
                        button {
                            class: "rsp-load-more",
                            onclick: move |_| update_active_tab(tabs, active_tab_id, |tab| tab.row_limit += RECORD_PAGE_SIZE),
                            "再显示 {RECORD_PAGE_SIZE} 项"
                        }
                    }
                }
                section { class: "rsp-detail-card",
                    if let Some(record) = selected {
                        span { class: "rsp-detail-icon", Glyph { name: "packet" } }
                        span { class: "rsp-detail-eyebrow", "RSG RECORD" }
                        h3 { title: "{record.name}", "{record.name}" }
                        div { class: if record.changed { "rsp-detail-state is-changed" } else { "rsp-detail-state" },
                            if record.changed { "包含 VCDIFF" } else { "复用原始包" }
                        }
                        dl {
                            div { dt { "Delta size" } dd { "{format_bytes(record.delta_size)}" } }
                            div { dt { "Before MD5" } dd { class: "rsp-hash", "{record.before_md5}" } }
                        }
                    } else {
                        div { class: "rsp-detail-empty",
                            Glyph { name: "packet" }
                            strong { "选择一个 RSG 记录" }
                            p { "这里会显示该包是否包含差分、VCDIFF 大小与原始 MD5。" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SummaryCard(label: &'static str, value: String, detail: String, icon: &'static str) -> Element {
    rsx! {
        article { class: "rsp-summary-card",
            span { class: "rsp-summary-icon", Glyph { name: icon } }
            div {
                span { "{label}" }
                strong { "{value}" }
                small { "{detail}" }
            }
        }
    }
}

#[component]
fn RecordRow(
    record: PatchRecordSummary,
    selected: bool,
    on_select: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if selected { "rsp-record-row is-selected" } else { "rsp-record-row" },
            title: "{record.name}",
            onclick: move |event| on_select.call(event),
            span { class: "rsp-record-name", Glyph { name: "packet" } "{record.name}" }
            span { class: if record.changed { "rsp-change is-changed" } else { "rsp-change" },
                if record.changed { "Changed" } else { "Reused" }
            }
            span { class: "rsp-record-size", "{format_bytes(record.delta_size)}" }
        }
    }
}

#[component]
fn WorkflowSheet(
    workflow: WorkflowDialog,
    busy: bool,
    on_close: EventHandler<()>,
    on_file: EventHandler<(DraftTarget, FileData)>,
    on_mode: EventHandler<PatchMode>,
    on_submit: EventHandler<()>,
) -> Element {
    let (title, description, mode, ready) = match &workflow {
        WorkflowDialog::Create(draft) => (
            "创建 RSBP",
            "比较两个 RSB v4 归档，并生成可复用的差分补丁。",
            draft.mode,
            draft.before.is_some() && draft.after.is_some(),
        ),
        WorkflowDialog::Apply(draft) => (
            "应用 RSBP",
            "验证原始归档的 MD5，并重建补丁对应的目标 RSB。",
            draft.mode,
            draft.before.is_some() && draft.patch.is_some(),
        ),
    };
    let creating = matches!(workflow, WorkflowDialog::Create(_));
    let submit_glyph = if creating { "create" } else { "apply" };
    let submit_label = if creating {
        "创建补丁"
    } else {
        "应用并导出"
    };
    rsx! {
        div { class: "rsp-dialog-layer",
            button { class: "rsp-dialog-backdrop", aria_label: "关闭", onclick: move |_| on_close.call(()) }
            section { class: "rsp-dialog", role: "dialog", aria_modal: "true", aria_label: "{title}",
                header {
                    div {
                        span { "RSBP WORKFLOW" }
                        h2 { "{title}" }
                        p { "{description}" }
                    }
                    button { class: "rsp-dialog-close", aria_label: "关闭", disabled: busy, onclick: move |_| on_close.call(()), Glyph { name: "close" } }
                }
                div { class: "rsp-file-slots",
                    match workflow.clone() {
                        WorkflowDialog::Create(draft) => rsx! {
                            FileSlot { label: "Before archive", hint: "原始 RSB / OBB", file: draft.before, accept: ARCHIVE_ACCEPT, busy, on_file: move |file| on_file.call((DraftTarget::CreateBefore, file)) }
                            FileSlot { label: "After archive", hint: "目标 RSB / OBB", file: draft.after, accept: ARCHIVE_ACCEPT, busy, on_file: move |file| on_file.call((DraftTarget::CreateAfter, file)) }
                        },
                        WorkflowDialog::Apply(draft) => rsx! {
                            FileSlot { label: "Before archive", hint: "补丁对应的原始 RSB / OBB", file: draft.before, accept: ARCHIVE_ACCEPT, busy, on_file: move |file| on_file.call((DraftTarget::ApplyBefore, file)) }
                            FileSlot { label: "Patch file", hint: "RSBP / .rsbpatch", file: draft.patch, accept: PATCH_ACCEPT, busy, on_file: move |file| on_file.call((DraftTarget::ApplyPatch, file)) }
                        },
                    }
                }
                div { class: "rsp-mode-section",
                    div {
                        strong { "Packet mode" }
                        span { "创建和应用时必须使用同一种模式" }
                    }
                    div { class: "rsp-mode-control", role: "group", aria_label: "Packet mode",
                        button { class: if mode == PatchMode::Stored { "is-active" } else { "" }, disabled: busy, onclick: move |_| on_mode.call(PatchMode::Stored),
                            strong { "Stored" }
                            span { "直接比较已压缩 RSG" }
                        }
                        button { class: if mode == PatchMode::Raw { "is-active" } else { "" }, disabled: busy, onclick: move |_| on_mode.call(PatchMode::Raw),
                            strong { "Raw" }
                            span { "解压后比较并重压缩" }
                        }
                    }
                }
                footer {
                    button { class: "rsp-dialog-cancel", disabled: busy, onclick: move |_| on_close.call(()), "取消" }
                    button { class: "rsp-dialog-submit", disabled: busy || !ready, onclick: move |_| on_submit.call(()),
                        Glyph { name: submit_glyph }
                        "{submit_label}"
                    }
                }
            }
        }
    }
}

#[component]
fn FileSlot(
    label: &'static str,
    hint: &'static str,
    file: Option<NamedBytes>,
    accept: &'static str,
    busy: bool,
    on_file: EventHandler<FileData>,
) -> Element {
    let has_file = file.is_some();
    let slot_glyph = if has_file { "check" } else { "open" };
    rsx! {
        label { class: if has_file { "rsp-file-slot has-file" } else { "rsp-file-slot" },
            span { class: "rsp-file-slot-icon", Glyph { name: slot_glyph } }
            div {
                strong { "{label}" }
                if let Some(file) = file {
                    span { title: "{file.name}", "{file.name}" }
                    small { "{format_bytes(file.data.len() as u64)}" }
                } else {
                    span { "{hint}" }
                    small { "点击选择文件" }
                }
            }
            input {
                class: "rsp-file-input",
                r#type: "file",
                accept,
                disabled: busy,
                onchange: move |event| {
                    if let Some(file) = event.files().into_iter().next() { on_file.call(file); }
                }
            }
        }
    }
}

fn open_patch_files(files: Vec<FileData>, mut signals: WorkspaceSignals) {
    if (signals.busy)() {
        return;
    }
    let count = files.len();
    signals.busy.set(true);
    signals.status.set(AppStatus::new(
        format!("正在读取并检查 {count} 个 RSBP…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let mut opened = Vec::new();
        let mut errors = Vec::new();
        for file in files {
            let name = file.name();
            let result = match file.read_bytes().await {
                Ok(bytes) => {
                    processing::inspect_patch(InspectRequest {
                        name: name.clone(),
                        data: bytes.to_vec(),
                    })
                    .await
                }
                Err(error) => Err(format!("无法读取文件：{error}")),
            };
            match result {
                Ok(inspected) => {
                    let id = (signals.next_tab_id)();
                    signals.next_tab_id.set(id.wrapping_add(1).max(1));
                    opened.push(PatchTab::new(
                        id,
                        inspected.summary.name.clone(),
                        inspected.data,
                        inspected.summary,
                    ));
                    push_application_log("RSBP", "INFO", "OPEN", format!("Opened {name}"));
                }
                Err(error) => {
                    errors.push(format!("{name}：{error}"));
                    push_application_log(
                        "RSBP",
                        "ERROR",
                        "OPEN",
                        format!("Open {name} failed: {error}"),
                    );
                }
            }
        }
        signals.busy.set(false);
        if let Some(last) = opened.last() {
            signals.active_tab_id.set(Some(last.id));
        }
        let opened_count = opened.len();
        signals.tabs.write().extend(opened);
        signals.status.set(if errors.is_empty() {
            AppStatus::new(
                format!("已打开 {opened_count} 个补丁。"),
                StatusTone::Success,
            )
        } else if opened_count == 0 {
            AppStatus::new(errors.join("；"), StatusTone::Error)
        } else {
            AppStatus::new(
                format!("已打开 {opened_count} 个补丁；{} 个失败。", errors.len()),
                StatusTone::Warning,
            )
        });
    });
}

fn load_draft_file(
    file: FileData,
    target: DraftTarget,
    mut dialog: Signal<Option<WorkflowDialog>>,
    mut busy: Signal<bool>,
    mut status: Signal<AppStatus>,
) {
    if busy() {
        return;
    }
    let name = file.name();
    busy.set(true);
    status.set(AppStatus::new(
        format!("正在读取 {name}…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        match file.read_bytes().await {
            Ok(bytes) => {
                let input = NamedBytes {
                    name: name.clone(),
                    data: Arc::from(bytes.to_vec()),
                };
                if let Some(workflow) = dialog.write().as_mut() {
                    match (workflow, target) {
                        (WorkflowDialog::Create(draft), DraftTarget::CreateBefore) => {
                            draft.before = Some(input)
                        }
                        (WorkflowDialog::Create(draft), DraftTarget::CreateAfter) => {
                            draft.after = Some(input)
                        }
                        (WorkflowDialog::Apply(draft), DraftTarget::ApplyBefore) => {
                            draft.before = Some(input)
                        }
                        (WorkflowDialog::Apply(draft), DraftTarget::ApplyPatch) => {
                            draft.patch = Some(input)
                        }
                        _ => {}
                    }
                }
                status.set(AppStatus::new(
                    format!("已读取 {name}。"),
                    StatusTone::Success,
                ));
            }
            Err(error) => status.set(AppStatus::new(
                format!("无法读取文件：{error}"),
                StatusTone::Error,
            )),
        }
        busy.set(false);
    });
}

fn set_dialog_mode(mut dialog: Signal<Option<WorkflowDialog>>, mode: PatchMode) {
    if let Some(workflow) = dialog.write().as_mut() {
        match workflow {
            WorkflowDialog::Create(draft) => draft.mode = mode,
            WorkflowDialog::Apply(draft) => draft.mode = mode,
        }
    }
}

fn submit_workflow(mut dialog: Signal<Option<WorkflowDialog>>, mut signals: WorkspaceSignals) {
    if (signals.busy)() {
        return;
    }
    let Some(workflow) = dialog() else {
        return;
    };
    match workflow {
        WorkflowDialog::Create(draft) => {
            let (Some(before), Some(after)) = (draft.before, draft.after) else {
                return;
            };
            signals.busy.set(true);
            signals.status.set(AppStatus::new(
                "正在后台比较 RSB 并创建补丁…",
                StatusTone::Neutral,
            ));
            spawn(async move {
                let result = processing::create_patch(CreateRequest {
                    before_name: before.name,
                    before: before.data.as_ref().to_vec(),
                    after_name: after.name,
                    after: after.data.as_ref().to_vec(),
                    mode: draft.mode,
                })
                .await;
                signals.busy.set(false);
                match result {
                    Ok(created) => {
                        let id = (signals.next_tab_id)();
                        signals.next_tab_id.set(id.wrapping_add(1).max(1));
                        let name = created.name.clone();
                        signals.tabs.write().push(PatchTab::new(
                            id,
                            created.name,
                            created.data,
                            created.summary,
                        ));
                        signals.active_tab_id.set(Some(id));
                        dialog.set(None);
                        signals.status.set(AppStatus::new(
                            format!("已创建 {name}，可以检查并导出。"),
                            StatusTone::Success,
                        ));
                        push_application_log("RSBP", "INFO", "CREATE", format!("Created {name}"));
                    }
                    Err(error) => {
                        signals
                            .status
                            .set(AppStatus::new(error.clone(), StatusTone::Error));
                        push_application_log("RSBP", "ERROR", "CREATE", error);
                    }
                }
            });
        }
        WorkflowDialog::Apply(draft) => {
            let (Some(before), Some(patch)) = (draft.before, draft.patch) else {
                return;
            };
            signals.busy.set(true);
            signals.status.set(AppStatus::new(
                "正在后台验证并重建 RSB…",
                StatusTone::Neutral,
            ));
            spawn(async move {
                let result = processing::apply_patch(ApplyRequest {
                    before_name: before.name,
                    before: before.data.as_ref().to_vec(),
                    patch_name: patch.name,
                    patch: patch.data.as_ref().to_vec(),
                    mode: draft.mode,
                })
                .await;
                match result {
                    Ok(applied) => {
                        let name = applied.name.clone();
                        match platform::save_bytes(&name, "RSB Archive", &["rsb"], &applied.data)
                            .await
                        {
                            Ok(true) => {
                                dialog.set(None);
                                signals.status.set(AppStatus::new(
                                    format!("已重建并导出 {name}。"),
                                    StatusTone::Success,
                                ));
                                push_application_log(
                                    "RSBP",
                                    "INFO",
                                    "APPLY",
                                    format!("Exported {name}"),
                                );
                            }
                            Ok(false) => signals
                                .status
                                .set(AppStatus::new("已取消导出。", StatusTone::Neutral)),
                            Err(error) => {
                                signals.status.set(AppStatus::new(error, StatusTone::Error))
                            }
                        }
                    }
                    Err(error) => {
                        signals
                            .status
                            .set(AppStatus::new(error.clone(), StatusTone::Error));
                        push_application_log("RSBP", "ERROR", "APPLY", error);
                    }
                }
                signals.busy.set(false);
            });
        }
    }
}

fn export_patch(tab: PatchTab, mut status: Signal<AppStatus>) {
    spawn(async move {
        match platform::save_bytes(&tab.name, "RSBP Patch", &["rsbpatch"], tab.data.as_ref()).await
        {
            Ok(true) => {
                status.set(AppStatus::new(
                    format!("已导出 {}。", tab.name),
                    StatusTone::Success,
                ));
                push_application_log("RSBP", "INFO", "EXPORT", format!("Exported {}", tab.name));
            }
            Ok(false) => {}
            Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
        }
    });
}

fn close_tab(
    mut tabs: Signal<Vec<PatchTab>>,
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

fn update_active_tab(
    mut tabs: Signal<Vec<PatchTab>>,
    active_tab_id: Signal<Option<u64>>,
    update: impl FnOnce(&mut PatchTab),
) {
    let Some(id) = active_tab_id() else {
        return;
    };
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == id) {
        update(tab);
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

#[component]
fn Glyph(name: &'static str) -> Element {
    let path = match name {
        "open" => "M3 7h6l2 2h10v10H3zM3 7V5h7l2 2",
        "add" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "create" => "M4 5h7l2 2h7v12H4zM12 10v6M9 13h6",
        "apply" => "M4 5h7l2 2h7v12H4zM8 13h8M13 10l3 3-3 3",
        "download" => "M12 3v12M7 10l5 5 5-5M5 20h14",
        "difference" => "M8 4v16M16 4v16M4 8h8M12 16h8",
        "archive" => "M4 5h16v4H4zM5 9h14v11H5zM10 13h4",
        "target" => "M12 3a9 9 0 1 0 9 9M12 7a5 5 0 1 0 5 5M12 11a1 1 0 1 0 1 1",
        "packets" => "M4 7l8-4 8 4-8 4zM4 12l8 4 8-4M4 17l8 4 8-4",
        "info" => "M12 8h.01M11 12h1v5h1M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z",
        "search" => "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM16 16l4 4",
        "packet" => "M4 7l8-4 8 4v10l-8 4-8-4zM4 7l8 4 8-4M12 11v10",
        "check" => "M5 12l4 4L19 6",
        _ => "M4 12h16",
    };
    rsx! {
        svg { class: "rsp-glyph", view_box: "0 0 24 24",
            path { d: path, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_format_is_stable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
    }
}
