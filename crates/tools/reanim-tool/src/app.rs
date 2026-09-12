use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use reanim_codec::{
    Reanim, ReanimTrack, ReanimTransform, ReanimVersion, decode_with_version, encode,
};
use toolkit_ui::{DropIndicator, InlineNotice, ToolPage, WorkspaceCard, push_application_log};

use crate::platform;

const REANIM_PAGE_CSS: Asset = asset!("/assets/reanim/page.css");
const FRAMES_PER_PAGE: usize = 36;

#[derive(Clone, PartialEq)]
struct ReanimTab {
    id: u64,
    name: String,
    source_size: usize,
    reanim: Reanim,
    version: ReanimVersion,
    applied_json: String,
    draft_json: String,
    dirty: bool,
    selected_track: usize,
    selected_frame: usize,
}

impl ReanimTab {
    fn new(
        id: u64,
        name: String,
        source_size: usize,
        reanim: Reanim,
        version: ReanimVersion,
        dirty: bool,
    ) -> Result<Self, String> {
        let json = format_json(&reanim)?;
        Ok(Self {
            id,
            name,
            source_size,
            reanim,
            version,
            applied_json: json.clone(),
            draft_json: json,
            dirty,
            selected_track: 0,
            selected_frame: 0,
        })
    }

    fn draft_changed(&self) -> bool {
        self.draft_json != self.applied_json
    }

    fn unsaved(&self) -> bool {
        self.dirty || self.draft_changed()
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

#[derive(Clone, Copy, PartialEq)]
struct WorkspaceSignals {
    tabs: Signal<Vec<ReanimTab>>,
    active_tab_id: Signal<Option<u64>>,
    next_tab_id: Signal<u64>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
}

#[component]
pub fn ReanimPage() -> Element {
    let tabs = use_signal(Vec::<ReanimTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);
    let signals = WorkspaceSignals {
        tabs,
        active_tab_id,
        next_tab_id,
        busy,
        status,
    };
    toolkit_ui::use_tool_open(toolkit_ui::ToolKind::Reanim, busy, move |files| {
        let files = files
            .into_iter()
            .map(toolkit_ui::ToolFile::into_file_data)
            .collect();
        open_files(files, signals);
    });
    let tabs_snapshot = tabs();
    let active_id = active_tab_id();
    let active_tab = active_id
        .and_then(|id| tabs_snapshot.iter().find(|tab| tab.id == id))
        .cloned();
    let status_snapshot = status();

    rsx! {
        document::Stylesheet { href: REANIM_PAGE_CSS }
        div {
            class: if dragging() { "reanim-page-host is-dragging" } else { "reanim-page-host" },
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
                if !files.is_empty() { open_files(files, signals); }
            },
            ToolPage {
                namespace: "reanim",
                class: if tabs_snapshot.is_empty() { "reanim-page is-empty" } else { "reanim-page" },
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
                        on_files: move |files| open_files(files, signals),
                    }
                }
                WorkspaceCard { class: "reanim-workspace-card", aria_label: "REANIM Editor",
                    if let Some(tab) = active_tab {
                        DocumentWorkspace { tab, signals }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| open_files(files, signals),
                            on_new: move |_| create_document(signals),
                            on_xfl: move |_| import_xfl(signals),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "reanim-status",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "松开以打开 REANIM 或结构化文本" }
                    }
                    if busy() {
                        div { class: "reanim-busy", role: "status",
                            span {}
                            "正在处理 REANIM…"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TabStrip(
    tabs: Vec<ReanimTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "reanim-tab-strip ui-document-tab-strip",
            div { class: "reanim-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 REANIM 文件",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: if Some(tab.id) == active_tab_id { "reanim-tab ui-document-tab is-active" } else { "reanim-tab ui-document-tab" },
                        button {
                            class: "reanim-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            title: "{tab.name}",
                            disabled: busy,
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "reanim-tab-dot ui-document-tab-dot" }
                            span { class: "reanim-tab-name ui-document-tab-name", "{tab.name}" }
                            if tab.unsaved() { span { class: "reanim-tab-unsaved", title: "存在未导出的更改" } }
                        }
                        button {
                            class: "reanim-tab-close ui-document-tab-close",
                            title: "关闭 {tab.name}",
                            aria_label: "关闭 {tab.name}",
                            disabled: busy,
                            onclick: move |_| on_close.call(tab.id),
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: if busy { "reanim-tab-add ui-document-new-tab is-disabled" } else { "reanim-tab-add ui-document-new-tab" },
                    title: "添加文件",
                    aria_label: "添加文件",
                    Glyph { name: "add" }
                    input {
                        class: "reanim-file-input",
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
    on_xfl: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section { class: "reanim-empty",
            div { class: "reanim-empty-visual", aria_hidden: "true",
                span { class: "reanim-empty-track track-a" }
                span { class: "reanim-empty-track track-b" }
                span { class: "reanim-empty-track track-c" }
                span { class: "reanim-empty-play" }
            }
            span { class: "reanim-eyebrow", "REANIM WORKSPACE" }
            h2 { "编辑 PopCap REANIM 时间轴" }
            p { "打开 compiled 或结构化文本，检查轨道和逐帧变换，调整属性并导出目标平台格式。" }
            div { class: "reanim-empty-actions",
                label { class: if busy { "reanim-primary is-disabled" } else { "reanim-primary" },
                    Glyph { name: "open" }
                    "选择文件"
                    input {
                        class: "reanim-file-input",
                        r#type: "file",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() { on_files.call(files); }
                        }
                    }
                }
                button { class: "reanim-secondary", disabled: busy || !platform::XFL_AVAILABLE, onclick: move |event| on_xfl.call(event),
                    Glyph { name: "folder" }
                    "导入 XFL"
                }
                button { class: "reanim-secondary", disabled: busy, onclick: move |event| on_new.call(event), "新建 REANIM" }
            }
            small { "支持 PC、32/64 位移动端 compiled、JSON/YAML/TOML；XFL 目录由桌面版处理" }
        }
    }
}

#[component]
fn DocumentWorkspace(tab: ReanimTab, signals: WorkspaceSignals) -> Element {
    let draft_changed = tab.draft_changed();
    let source_label = if tab.source_size == 0 {
        "新建或导入文档".to_string()
    } else {
        format!("源文件 {}", format_bytes(tab.source_size))
    };
    let track_count = tab.reanim.tracks.len();
    let frame_count = tab
        .reanim
        .tracks
        .iter()
        .map(|track| track.transforms.len())
        .sum::<usize>();
    let image_count = tab
        .reanim
        .tracks
        .iter()
        .flat_map(|track| &track.transforms)
        .filter_map(|transform| transform.i.as_deref())
        .collect::<HashSet<_>>()
        .len();
    let tab_for_json = tab.clone();
    let tab_for_binary = tab.clone();
    let tab_for_xfl = tab.clone();

    rsx! {
        div { class: "reanim-document",
            header { class: "reanim-document-header",
                div { class: "reanim-document-title",
                    span { class: "reanim-kind-badge", "REANIM" }
                    div {
                        h2 { "{tab.name}" }
                        p { "{source_label} · {track_count} 条轨道 · {frame_count} 个变换" }
                    }
                }
                div { class: "reanim-header-actions",
                    button {
                        class: "reanim-action",
                        disabled: (signals.busy)() || draft_changed || !platform::XFL_AVAILABLE,
                        title: if platform::XFL_AVAILABLE { "导入 XFL 目录" } else { "XFL 目录导入仅支持桌面版" },
                        onclick: move |_| import_xfl(signals),
                        Glyph { name: "folder" }
                        "XFL+"
                    }
                    button {
                        class: "reanim-action",
                        disabled: (signals.busy)() || draft_changed || !platform::XFL_AVAILABLE,
                        title: if platform::XFL_AVAILABLE { "导出 XFL 目录" } else { "XFL 目录导出仅支持桌面版" },
                        onclick: move |_| export_xfl(tab_for_xfl.clone(), signals.tabs, signals.status),
                        Glyph { name: "layers" }
                        "XFL"
                    }
                    button {
                        class: "reanim-action",
                        disabled: (signals.busy)() || draft_changed,
                        title: if draft_changed { "请先应用 JSON 更改" } else { "导出 JSON" },
                        onclick: move |_| export_json(tab_for_json.clone(), signals.tabs, signals.status),
                        Glyph { name: "code" }
                        "JSON"
                    }
                    button {
                        class: "reanim-action primary",
                        disabled: (signals.busy)() || draft_changed,
                        title: if draft_changed { "请先应用 JSON 更改" } else { "导出 compiled" },
                        onclick: move |_| export_binary(tab_for_binary.clone(), signals.tabs, signals.status),
                        Glyph { name: "download" }
                        "Compiled"
                    }
                }
            }

            div { class: "reanim-summary-grid",
                SummaryCard { glyph: "tracks", label: "轨道", value: track_count.to_string() }
                SummaryCard { glyph: "frames", label: "变换帧", value: frame_count.to_string() }
                SummaryCard { glyph: "image", label: "图像引用", value: image_count.to_string() }
                SummaryCard { glyph: "format", label: "输出布局", value: version_label(tab.version).to_string() }
            }

            DocumentSettings { tab: tab.clone(), disabled: draft_changed, signals }

            div { class: "reanim-main-grid",
                TrackPanel { tab: tab.clone(), disabled: draft_changed, signals }
                FramePanel { tab: tab.clone(), disabled: draft_changed, signals }
                TransformInspector { tab: tab.clone(), disabled: draft_changed, signals }
            }

            JsonEditor {
                tab_id: tab.id,
                draft: tab.draft_json,
                changed: draft_changed,
                busy: (signals.busy)(),
                signals,
            }
        }
    }
}

#[component]
fn SummaryCard(glyph: &'static str, label: &'static str, value: String) -> Element {
    rsx! {
        article { class: "reanim-summary-card",
            span { class: "reanim-summary-icon", Glyph { name: glyph } }
            div { small { "{label}" } strong { "{value}" } }
        }
    }
}

#[component]
fn DocumentSettings(tab: ReanimTab, disabled: bool, signals: WorkspaceSignals) -> Element {
    rsx! {
        section { class: "reanim-settings-card",
            div { class: "reanim-setting",
                label { r#for: "reanim-fps-{tab.id}", "帧率" }
                input {
                    id: "reanim-fps-{tab.id}",
                    r#type: "number",
                    min: "0.01",
                    step: "0.01",
                    value: format_float(tab.reanim.fps),
                    disabled,
                    oninput: move |event| set_fps(tab.id, &event.value(), signals),
                }
            }
            div { class: "reanim-setting",
                label { r#for: "reanim-scale-{tab.id}", "DoScale" }
                select {
                    id: "reanim-scale-{tab.id}",
                    value: do_scale_code(tab.reanim.do_scale),
                    disabled,
                    onchange: move |event| set_do_scale(tab.id, &event.value(), signals),
                    option { value: "none", "未设置" }
                    option { value: "0", "关闭" }
                    option { value: "1", "开启" }
                }
            }
            div { class: "reanim-setting wide",
                label { r#for: "reanim-version-{tab.id}", "Compiled 输出布局" }
                select {
                    id: "reanim-version-{tab.id}",
                    value: version_code(tab.version),
                    disabled,
                    onchange: move |event| set_version(tab.id, &event.value(), signals),
                    option { value: "pc", "PC · 32-bit" }
                    option { value: "phone32", "Mobile · 32-bit" }
                    option { value: "phone64", "Mobile · 64-bit" }
                }
            }
        }
    }
}

#[component]
fn TrackPanel(tab: ReanimTab, disabled: bool, signals: WorkspaceSignals) -> Element {
    let selected = tab.reanim.tracks.get(tab.selected_track).cloned();
    rsx! {
        section { class: "reanim-panel reanim-track-panel",
            header { class: "reanim-panel-header",
                div { span { "TRACKS" } strong { "轨道" } }
                div { class: "reanim-panel-actions",
                    button { title: "添加轨道", disabled, onclick: move |_| add_track(tab.id, signals), Glyph { name: "add" } }
                    button { title: "删除所选轨道", disabled: disabled || selected.is_none(), onclick: move |_| remove_track(tab.id, signals), Glyph { name: "trash" } }
                }
            }
            div { class: "reanim-track-list",
                if tab.reanim.tracks.is_empty() {
                    div { class: "reanim-panel-empty", "没有轨道" }
                }
                for (index, track) in tab.reanim.tracks.iter().enumerate() {
                    button {
                        key: "track-{tab.id}-{index}",
                        class: if index == tab.selected_track { "reanim-track-item is-active" } else { "reanim-track-item" },
                        onclick: move |_| select_track(tab.id, index, signals.tabs),
                        span { class: "reanim-track-index", "{index + 1}" }
                        span { class: "reanim-track-copy",
                            strong { "{track_name(track, index)}" }
                            small { {format!("{} frames", track.transforms.len())} }
                        }
                    }
                }
            }
            if let Some(ref track) = selected {
                div { class: "reanim-track-name-editor",
                    label { "轨道名称" }
                    input {
                        value: track.name.clone(),
                        disabled,
                        oninput: move |event| set_track_name(tab.id, event.value(), signals.tabs),
                    }
                }
            }
        }
    }
}

#[component]
fn FramePanel(tab: ReanimTab, disabled: bool, signals: WorkspaceSignals) -> Element {
    let track = tab.reanim.tracks.get(tab.selected_track).cloned();
    let frame_count = track.as_ref().map_or(0, |track| track.transforms.len());
    let max_page = frame_count.saturating_sub(1) / FRAMES_PER_PAGE;
    let page = (tab.selected_frame / FRAMES_PER_PAGE).min(max_page);
    let start = page * FRAMES_PER_PAGE;
    let end = (start + FRAMES_PER_PAGE).min(frame_count);
    rsx! {
        section { class: "reanim-panel reanim-frame-panel",
            header { class: "reanim-panel-header",
                div { span { "TIMELINE" } strong { "变换帧" } }
                div { class: "reanim-panel-actions",
                    button { title: "添加帧", disabled: disabled || track.is_none(), onclick: move |_| add_frame(tab.id, signals), Glyph { name: "add" } }
                    button { title: "删除所选帧", disabled: disabled || frame_count == 0, onclick: move |_| remove_frame(tab.id, signals), Glyph { name: "trash" } }
                }
            }
            if let Some(ref track) = track {
                div { class: "reanim-frame-list",
                    if track.transforms.is_empty() {
                        div { class: "reanim-panel-empty", "该轨道没有变换帧" }
                    }
                    for index in start..end {
                        {
                            let transform = &track.transforms[index];
                            rsx! {
                                button {
                                    key: "frame-{tab.id}-{tab.selected_track}-{index}",
                                    class: if index == tab.selected_frame { "reanim-frame-item is-active" } else { "reanim-frame-item" },
                                    onclick: move |_| select_frame(tab.id, index, signals.tabs),
                                    span { class: "reanim-frame-number", "{index}" }
                                    span { class: "reanim-frame-copy",
                                        strong { "{frame_title(transform)}" }
                                        small { "{frame_detail(transform)}" }
                                    }
                                    span { class: if transform.f == Some(-1.0) { "reanim-frame-state is-hidden" } else { "reanim-frame-state" } }
                                }
                            }
                        }
                    }
                }
                div { class: "reanim-pagination",
                    button { title: "上一页", disabled: page == 0, onclick: move |_| select_frame(tab.id, start.saturating_sub(FRAMES_PER_PAGE), signals.tabs), Glyph { name: "left" } }
                    span { "{page + 1} / {max_page + 1}" }
                    button { title: "下一页", disabled: page >= max_page, onclick: move |_| select_frame(tab.id, end, signals.tabs), Glyph { name: "right" } }
                }
            } else {
                div { class: "reanim-panel-empty fill", "选择或新建轨道后编辑时间轴" }
            }
        }
    }
}

#[component]
fn TransformInspector(tab: ReanimTab, disabled: bool, signals: WorkspaceSignals) -> Element {
    let transform = tab
        .reanim
        .tracks
        .get(tab.selected_track)
        .and_then(|track| track.transforms.get(tab.selected_frame))
        .cloned();
    let selection_key = format!("{}-{}-{}", tab.id, tab.selected_track, tab.selected_frame);
    rsx! {
        section { class: "reanim-panel reanim-inspector-panel",
            header { class: "reanim-panel-header",
                div { span { "INSPECTOR" } strong { "帧属性" } }
                if transform.is_some() { small { "Frame {tab.selected_frame}" } }
            }
            if let Some(transform) = transform {
                div {
                    key: "inspector-{tab.id}-{tab.selected_track}-{tab.selected_frame}",
                    class: "reanim-inspector-scroll",
                    div { class: "reanim-field-section",
                        h3 { "Transform" }
                        div { class: "reanim-field-grid",
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "x", label: "X", value: transform.x, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "y", label: "Y", value: transform.y, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "kx", label: "Skew X", value: transform.kx, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "ky", label: "Skew Y", value: transform.ky, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "sx", label: "Scale X", value: transform.sx, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "sy", label: "Scale Y", value: transform.sy, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "f", label: "Frame", value: transform.f, disabled, signals }
                            OptionalNumberField { identity: selection_key.clone(), tab_id: tab.id, field: "a", label: "Alpha", value: transform.a, disabled, signals }
                        }
                    }
                    div { class: "reanim-field-section",
                        h3 { "Content" }
                        OptionalTextField { identity: selection_key.clone(), tab_id: tab.id, field: "i", label: "Image / Symbol", value: transform.i, disabled, signals }
                        OptionalTextField { identity: selection_key.clone(), tab_id: tab.id, field: "resource", label: "Resource", value: transform.resource, disabled, signals }
                        OptionalTextField { identity: selection_key.clone(), tab_id: tab.id, field: "i2", label: "Image 2", value: transform.i2, disabled, signals }
                        OptionalTextField { identity: selection_key.clone(), tab_id: tab.id, field: "resource2", label: "Resource 2", value: transform.resource2, disabled, signals }
                        OptionalTextField { identity: selection_key.clone(), tab_id: tab.id, field: "font", label: "Font", value: transform.font, disabled, signals }
                        OptionalTextField { identity: selection_key.clone(), tab_id: tab.id, field: "text", label: "Text", value: transform.text, disabled, signals }
                    }
                }
            } else {
                div { class: "reanim-panel-empty fill", "选择一个变换帧以检查属性" }
            }
        }
    }
}

#[component]
fn OptionalNumberField(
    identity: String,
    tab_id: u64,
    field: &'static str,
    label: &'static str,
    value: Option<f32>,
    disabled: bool,
    signals: WorkspaceSignals,
) -> Element {
    rsx! {
        label { class: "reanim-field",
            span { "{label}" }
            input {
                key: "{identity}-{field}",
                r#type: "number",
                step: "any",
                placeholder: "unset",
                value: value.map(format_float).unwrap_or_default(),
                disabled,
                oninput: move |event| set_number_field(tab_id, field, &event.value(), signals),
            }
        }
    }
}

#[component]
fn OptionalTextField(
    identity: String,
    tab_id: u64,
    field: &'static str,
    label: &'static str,
    value: Option<String>,
    disabled: bool,
    signals: WorkspaceSignals,
) -> Element {
    rsx! {
        label { class: "reanim-field wide",
            span { "{label}" }
            input {
                key: "{identity}-{field}",
                placeholder: "unset",
                value: value.unwrap_or_default(),
                disabled,
                oninput: move |event| set_text_field(tab_id, field, event.value(), signals.tabs),
            }
        }
    }
}

#[component]
fn JsonEditor(
    tab_id: u64,
    draft: String,
    changed: bool,
    busy: bool,
    signals: WorkspaceSignals,
) -> Element {
    rsx! {
        section { class: "reanim-json-card",
            header {
                div { span { "SOURCE" } h3 { "JSON 源数据" } p { "可以编辑全部 REANIM 字段；应用后会同步结构化面板。" } }
                div { class: "reanim-json-actions",
                    button { disabled: busy || !changed, onclick: move |_| revert_json(tab_id, signals.tabs), "还原" }
                    button { class: "primary", disabled: busy || !changed, onclick: move |_| apply_json(tab_id, signals), Glyph { name: "check" } "应用" }
                }
            }
            textarea {
                value: draft,
                spellcheck: false,
                disabled: busy,
                oninput: move |event| update_draft(tab_id, event.value(), signals.tabs),
            }
        }
    }
}

fn open_files(files: Vec<FileData>, signals: WorkspaceSignals) {
    if files.is_empty() || (signals.busy)() {
        return;
    }
    let pending = files
        .into_iter()
        .map(|file| {
            let id = (signals.next_tab_id)();
            let mut next = signals.next_tab_id;
            next.set(id.wrapping_add(1).max(1));
            (id, file)
        })
        .collect::<Vec<_>>();
    let count = pending.len();
    let mut busy = signals.busy;
    let mut status = signals.status;
    let mut tabs = signals.tabs;
    let mut active_tab_id = signals.active_tab_id;
    busy.set(true);
    status.set(AppStatus::new(
        format!("正在读取并解析 {count} 个文件…"),
        StatusTone::Neutral,
    ));
    spawn(async move {
        let mut opened = Vec::new();
        let mut errors = Vec::new();
        for (id, file) in pending {
            let name = file.name();
            match file.read_bytes().await {
                Ok(bytes) => match decode_document(&name, &bytes) {
                    Ok((reanim, version)) => {
                        match ReanimTab::new(id, name.clone(), bytes.len(), reanim, version, false)
                        {
                            Ok(tab) => {
                                push_application_log(
                                    "REANIM",
                                    "INFO",
                                    "OPEN",
                                    format!("Opened {name}"),
                                );
                                opened.push(tab);
                            }
                            Err(error) => errors.push(format!("{name}：{error}")),
                        }
                    }
                    Err(error) => {
                        push_application_log(
                            "REANIM",
                            "ERROR",
                            "OPEN",
                            format!("Open {name} failed: {error}"),
                        );
                        errors.push(format!("{name}：{error}"));
                    }
                },
                Err(error) => errors.push(format!("{name}：无法读取文件：{error}")),
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

fn decode_document(name: &str, bytes: &[u8]) -> Result<(Reanim, ReanimVersion), String> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".json") {
        return serde_json::from_slice(bytes)
            .map(|reanim| (reanim, ReanimVersion::PC))
            .map_err(|error| format!("JSON 无法解析：{error}"));
    }
    if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        return serde_yaml::from_slice(bytes)
            .map(|reanim| (reanim, ReanimVersion::PC))
            .map_err(|error| format!("YAML 无法解析：{error}"));
    }
    if lower.ends_with(".toml") {
        let text =
            std::str::from_utf8(bytes).map_err(|error| format!("TOML 不是 UTF-8：{error}"))?;
        return toml::from_str(text)
            .map(|reanim| (reanim, ReanimVersion::PC))
            .map_err(|error| format!("TOML 无法解析：{error}"));
    }
    if let Ok(text) = std::str::from_utf8(bytes)
        && text
            .trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with('{')
    {
        return serde_json::from_str(text)
            .map(|reanim| (reanim, ReanimVersion::PC))
            .map_err(|error| format!("JSON 无法解析：{error}"));
    }
    decode_with_version(bytes).map_err(|error| error.to_string())
}

fn create_document(mut signals: WorkspaceSignals) {
    let id = (signals.next_tab_id)();
    signals.next_tab_id.set(id.wrapping_add(1).max(1));
    let reanim = Reanim {
        do_scale: None,
        fps: 30.0,
        tracks: vec![ReanimTrack {
            name: "Track 1".to_string(),
            transforms: vec![ReanimTransform::default()],
        }],
    };
    match ReanimTab::new(
        id,
        "untitled.reanim.json".to_string(),
        0,
        reanim,
        ReanimVersion::PC,
        true,
    ) {
        Ok(tab) => {
            signals.tabs.write().push(tab);
            signals.active_tab_id.set(Some(id));
            signals
                .status
                .set(AppStatus::new("已新建 REANIM。", StatusTone::Success));
        }
        Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
    }
}

fn import_xfl(mut signals: WorkspaceSignals) {
    if (signals.busy)() {
        return;
    }
    let id = (signals.next_tab_id)();
    signals.next_tab_id.set(id.wrapping_add(1).max(1));
    signals.busy.set(true);
    signals
        .status
        .set(AppStatus::new("正在导入 XFL…", StatusTone::Neutral));
    spawn(async move {
        match platform::open_xfl().await {
            Ok(Some((name, reanim))) => {
                match ReanimTab::new(id, name.clone(), 0, reanim, ReanimVersion::PC, true) {
                    Ok(tab) => {
                        signals.tabs.write().push(tab);
                        signals.active_tab_id.set(Some(id));
                        signals
                            .status
                            .set(AppStatus::new("XFL 已导入。", StatusTone::Success));
                        push_application_log(
                            "REANIM",
                            "INFO",
                            "IMPORT",
                            format!("Imported {name}"),
                        );
                    }
                    Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
                }
            }
            Ok(None) => signals.status.set(AppStatus::default()),
            Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
        }
        signals.busy.set(false);
    });
}

fn close_tab(
    mut tabs: Signal<Vec<ReanimTab>>,
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

fn select_track(tab_id: u64, index: usize, mut tabs: Signal<Vec<ReanimTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.selected_track = index;
        tab.selected_frame = 0;
    }
}

fn select_frame(tab_id: u64, index: usize, mut tabs: Signal<Vec<ReanimTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        let count = tab
            .reanim
            .tracks
            .get(tab.selected_track)
            .map_or(0, |track| track.transforms.len());
        tab.selected_frame = index.min(count.saturating_sub(1));
    }
}

fn sync_json(tab: &mut ReanimTab) -> Result<(), String> {
    let json = format_json(&tab.reanim)?;
    tab.applied_json = json.clone();
    tab.draft_json = json;
    tab.dirty = true;
    tab.selected_track = tab
        .selected_track
        .min(tab.reanim.tracks.len().saturating_sub(1));
    tab.selected_frame = tab
        .reanim
        .tracks
        .get(tab.selected_track)
        .map_or(0, |track| {
            tab.selected_frame
                .min(track.transforms.len().saturating_sub(1))
        });
    Ok(())
}

fn set_fps(tab_id: u64, value: &str, mut signals: WorkspaceSignals) {
    let Ok(value) = value.parse::<f32>() else {
        signals
            .status
            .set(AppStatus::new("帧率必须是有效数字。", StatusTone::Error));
        return;
    };
    if !value.is_finite() || value <= 0.0 {
        signals
            .status
            .set(AppStatus::new("帧率必须大于 0。", StatusTone::Error));
        return;
    }
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.reanim.fps = value;
        let _ = sync_json(tab);
    }
    signals.status.set(AppStatus::default());
}

fn set_do_scale(tab_id: u64, value: &str, mut signals: WorkspaceSignals) {
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.reanim.do_scale = match value {
            "0" => Some(0),
            "1" => Some(1),
            _ => None,
        };
        let _ = sync_json(tab);
    }
}

fn set_version(tab_id: u64, value: &str, mut signals: WorkspaceSignals) {
    let version = match value {
        "phone32" => ReanimVersion::Phone32,
        "phone64" => ReanimVersion::Phone64,
        _ => ReanimVersion::PC,
    };
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.version = version;
        tab.dirty = true;
    }
    signals.status.set(AppStatus::new(
        format!("输出布局已改为 {}。", version_label(version)),
        StatusTone::Success,
    ));
}

fn add_track(tab_id: u64, mut signals: WorkspaceSignals) {
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        let index = tab.reanim.tracks.len();
        tab.reanim.tracks.push(ReanimTrack {
            name: format!("Track {}", index + 1),
            transforms: vec![ReanimTransform::default()],
        });
        tab.selected_track = index;
        tab.selected_frame = 0;
        let _ = sync_json(tab);
    }
    signals
        .status
        .set(AppStatus::new("已添加轨道。", StatusTone::Success));
}

fn remove_track(tab_id: u64, mut signals: WorkspaceSignals) {
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
        && !tab.reanim.tracks.is_empty()
    {
        let index = tab.selected_track.min(tab.reanim.tracks.len() - 1);
        tab.reanim.tracks.remove(index);
        let _ = sync_json(tab);
    }
    signals
        .status
        .set(AppStatus::new("已删除轨道。", StatusTone::Warning));
}

fn set_track_name(tab_id: u64, value: String, mut tabs: Signal<Vec<ReanimTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if let Some(track) = tab.reanim.tracks.get_mut(tab.selected_track) {
            track.name = value;
        }
        let _ = sync_json(tab);
    }
}

fn add_frame(tab_id: u64, mut signals: WorkspaceSignals) {
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if let Some(track) = tab.reanim.tracks.get_mut(tab.selected_track) {
            track.transforms.push(ReanimTransform::default());
            tab.selected_frame = track.transforms.len() - 1;
        }
        let _ = sync_json(tab);
    }
    signals
        .status
        .set(AppStatus::new("已添加变换帧。", StatusTone::Success));
}

fn remove_frame(tab_id: u64, mut signals: WorkspaceSignals) {
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if let Some(track) = tab.reanim.tracks.get_mut(tab.selected_track)
            && !track.transforms.is_empty()
        {
            let index = tab.selected_frame.min(track.transforms.len() - 1);
            track.transforms.remove(index);
        }
        let _ = sync_json(tab);
    }
    signals
        .status
        .set(AppStatus::new("已删除变换帧。", StatusTone::Warning));
}

fn set_number_field(tab_id: u64, field: &'static str, value: &str, mut signals: WorkspaceSignals) {
    let parsed = if value.trim().is_empty() {
        None
    } else {
        let Ok(parsed) = value.parse::<f32>() else {
            signals.status.set(AppStatus::new(
                format!("{field} 必须是有效数字。"),
                StatusTone::Error,
            ));
            return;
        };
        if !parsed.is_finite() {
            signals.status.set(AppStatus::new(
                format!("{field} 必须是有限数字。"),
                StatusTone::Error,
            ));
            return;
        }
        Some(parsed)
    };
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if let Some(transform) = selected_transform_mut(tab) {
            match field {
                "x" => transform.x = parsed,
                "y" => transform.y = parsed,
                "kx" => transform.kx = parsed,
                "ky" => transform.ky = parsed,
                "sx" => transform.sx = parsed,
                "sy" => transform.sy = parsed,
                "f" => transform.f = parsed,
                "a" => transform.a = parsed,
                _ => return,
            }
        }
        let _ = sync_json(tab);
    }
    signals.status.set(AppStatus::default());
}

fn set_text_field(
    tab_id: u64,
    field: &'static str,
    value: String,
    mut tabs: Signal<Vec<ReanimTab>>,
) {
    let value = (!value.is_empty()).then_some(value);
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        if let Some(transform) = selected_transform_mut(tab) {
            match field {
                "i" => transform.i = value,
                "resource" => transform.resource = value,
                "i2" => transform.i2 = value,
                "resource2" => transform.resource2 = value,
                "font" => transform.font = value,
                "text" => transform.text = value,
                _ => return,
            }
        }
        let _ = sync_json(tab);
    }
}

fn selected_transform_mut(tab: &mut ReanimTab) -> Option<&mut ReanimTransform> {
    tab.reanim
        .tracks
        .get_mut(tab.selected_track)
        .and_then(|track| track.transforms.get_mut(tab.selected_frame))
}

fn update_draft(tab_id: u64, value: String, mut tabs: Signal<Vec<ReanimTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.draft_json = value;
    }
}

fn revert_json(tab_id: u64, mut tabs: Signal<Vec<ReanimTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.draft_json = tab.applied_json.clone();
    }
}

fn apply_json(tab_id: u64, mut signals: WorkspaceSignals) {
    let Some(tab) = (signals.tabs)()
        .iter()
        .find(|tab| tab.id == tab_id)
        .cloned()
    else {
        return;
    };
    match serde_json::from_str::<Reanim>(&tab.draft_json) {
        Ok(reanim) => match format_json(&reanim) {
            Ok(normalized) => {
                if let Some(current) = signals
                    .tabs
                    .write()
                    .iter_mut()
                    .find(|item| item.id == tab_id)
                {
                    current.reanim = reanim;
                    current.applied_json = normalized.clone();
                    current.draft_json = normalized;
                    current.dirty = true;
                    current.selected_track = current
                        .selected_track
                        .min(current.reanim.tracks.len().saturating_sub(1));
                    current.selected_frame = current
                        .reanim
                        .tracks
                        .get(current.selected_track)
                        .map_or(0, |track| {
                            current
                                .selected_frame
                                .min(track.transforms.len().saturating_sub(1))
                        });
                }
                signals.status.set(AppStatus::new(
                    "JSON 已通过校验并应用。",
                    StatusTone::Success,
                ));
                push_application_log(
                    "REANIM",
                    "INFO",
                    "APPLY",
                    format!("Applied JSON for {}", tab.name),
                );
            }
            Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
        },
        Err(error) => {
            let message = format!("JSON 无法解析：{error}");
            signals
                .status
                .set(AppStatus::new(message.clone(), StatusTone::Error));
            push_application_log("REANIM", "ERROR", "APPLY", message);
        }
    }
}

fn export_json(tab: ReanimTab, mut tabs: Signal<Vec<ReanimTab>>, mut status: Signal<AppStatus>) {
    spawn(async move {
        let name = format!("{}.reanim.json", document_stem(&tab.name));
        if export_bytes(
            &name,
            "REANIM JSON",
            &["json"],
            tab.applied_json.as_bytes(),
            "JSON",
            &mut status,
        )
        .await
        {
            mark_exported(tab.id, &mut tabs);
        }
    });
}

fn export_binary(tab: ReanimTab, mut tabs: Signal<Vec<ReanimTab>>, mut status: Signal<AppStatus>) {
    spawn(async move {
        match encode(&tab.reanim, tab.version) {
            Ok(bytes) => {
                let name = format!("{}.reanim.compiled", document_stem(&tab.name));
                if export_bytes(
                    &name,
                    "REANIM compiled",
                    &["compiled", "reanim"],
                    &bytes,
                    "COMPILED",
                    &mut status,
                )
                .await
                {
                    mark_exported(tab.id, &mut tabs);
                }
            }
            Err(error) => status.set(AppStatus::new(
                format!("无法编码 compiled：{error}"),
                StatusTone::Error,
            )),
        }
    });
}

fn export_xfl(tab: ReanimTab, mut tabs: Signal<Vec<ReanimTab>>, mut status: Signal<AppStatus>) {
    spawn(async move {
        let name = format!("{}-xfl", document_stem(&tab.name));
        match platform::save_xfl(&name, &tab.reanim).await {
            Ok(true) => {
                mark_exported(tab.id, &mut tabs);
                status.set(AppStatus::new(
                    format!("已导出 XFL 目录 {name}。"),
                    StatusTone::Success,
                ));
                push_application_log("REANIM", "INFO", "EXPORT", format!("Exported {name} (XFL)"));
            }
            Ok(false) => {}
            Err(error) => status.set(AppStatus::new(error, StatusTone::Error)),
        }
    });
}

async fn export_bytes(
    name: &str,
    filter: &str,
    extensions: &[&str],
    bytes: &[u8],
    kind: &str,
    status: &mut Signal<AppStatus>,
) -> bool {
    match platform::save_bytes(name, filter, extensions, bytes).await {
        Ok(true) => {
            push_application_log(
                "REANIM",
                "INFO",
                "EXPORT",
                format!("Exported {name} ({kind})"),
            );
            status.set(AppStatus::new(
                format!("已导出 {name}。"),
                StatusTone::Success,
            ));
            true
        }
        Ok(false) => false,
        Err(error) => {
            push_application_log(
                "REANIM",
                "ERROR",
                "EXPORT",
                format!("Export {name} failed: {error}"),
            );
            status.set(AppStatus::new(error, StatusTone::Error));
            false
        }
    }
}

fn mark_exported(id: u64, tabs: &mut Signal<Vec<ReanimTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == id) {
        tab.dirty = false;
    }
}

fn format_json(reanim: &Reanim) -> Result<String, String> {
    serde_json::to_string_pretty(reanim).map_err(|error| format!("无法生成 JSON：{error}"))
}

fn document_stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for suffix in [
        ".reanim.compiled",
        ".reanim.json",
        ".reanim.yaml",
        ".reanim.yml",
        ".reanim.toml",
        ".compiled",
        ".reanim",
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".xfl",
    ] {
        if lower.ends_with(suffix) {
            let stem = &name[..name.len() - suffix.len()];
            return if stem.is_empty() {
                "untitled".to_string()
            } else {
                stem.to_string()
            };
        }
    }
    if name.is_empty() {
        "untitled".to_string()
    } else {
        name.to_string()
    }
}

fn track_name(track: &ReanimTrack, index: usize) -> String {
    if track.name.trim().is_empty() {
        format!("Track {}", index + 1)
    } else {
        track.name.clone()
    }
}

fn frame_title(transform: &ReanimTransform) -> String {
    transform
        .i
        .clone()
        .or_else(|| transform.text.clone())
        .unwrap_or_else(|| "Transform".to_string())
}

fn frame_detail(transform: &ReanimTransform) -> String {
    let mut parts = Vec::new();
    if let Some(x) = transform.x {
        parts.push(format!("x {}", format_float(x)));
    }
    if let Some(y) = transform.y {
        parts.push(format!("y {}", format_float(y)));
    }
    if let Some(a) = transform.a {
        parts.push(format!("α {}", format_float(a)));
    }
    if transform.f == Some(-1.0) {
        parts.push("hidden".to_string());
    }
    if parts.is_empty() {
        "继承上一帧".to_string()
    } else {
        parts.join(" · ")
    }
}

const fn version_code(version: ReanimVersion) -> &'static str {
    match version {
        ReanimVersion::PC => "pc",
        ReanimVersion::Phone32 => "phone32",
        ReanimVersion::Phone64 => "phone64",
    }
}

const fn version_label(version: ReanimVersion) -> &'static str {
    match version {
        ReanimVersion::PC => "PC · 32-bit",
        ReanimVersion::Phone32 => "Mobile · 32-bit",
        ReanimVersion::Phone64 => "Mobile · 64-bit",
    }
}

const fn do_scale_code(value: Option<i8>) -> &'static str {
    match value {
        Some(0) => "0",
        Some(_) => "1",
        None => "none",
    }
}

fn format_float(value: f32) -> String {
    let formatted = format!("{value:.4}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
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
fn Glyph(name: &'static str) -> Element {
    let path = match name {
        "open" => "M3 7h6l2 2h10v10H3zM3 7V5h7l2 2",
        "folder" => "M3 6h6l2 2h10v11H3z",
        "add" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "download" => "M12 3v12M7 10l5 5 5-5M5 20h14",
        "code" => "M8 9l-4 3 4 3M16 9l4 3-4 3M14 5l-4 14",
        "layers" => "M12 3l9 5-9 5-9-5zM3 12l9 5 9-5M3 16l9 5 9-5",
        "tracks" => "M4 6h16M4 12h16M4 18h16M8 4v4M15 10v4M11 16v4",
        "frames" => "M4 5h16v14H4zM8 5v14M16 5v14M4 10h4M16 14h4",
        "image" => "M4 5h16v14H4zM8 10a2 2 0 1 0 0-4M4 16l5-5 4 4 2-2 5 5",
        "format" => "M5 4h14v16H5zM8 8h8M8 12h8M8 16h5",
        "trash" => "M5 7h14M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5",
        "check" => "M5 12l4 4L19 6",
        "left" => "M15 18l-6-6 6-6",
        "right" => "M9 18l6-6-6-6",
        _ => "M4 12h16",
    };
    rsx! {
        svg { class: "reanim-glyph", view_box: "0 0 24 24",
            path { d: path, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_structured_formats_and_binary_layouts() {
        let source = Reanim {
            fps: 30.0,
            ..Default::default()
        };
        let json = serde_json::to_vec(&source).unwrap();
        assert_eq!(decode_document("test.json", &json).unwrap().0, source);

        for version in [
            ReanimVersion::PC,
            ReanimVersion::Phone32,
            ReanimVersion::Phone64,
        ] {
            let bytes = encode(&source, version).unwrap();
            assert_eq!(decode_document("test.reanim", &bytes).unwrap().1, version);
        }
    }

    #[test]
    fn output_names_do_not_stack_extensions() {
        assert_eq!(document_stem("idle.reanim.compiled"), "idle");
        assert_eq!(document_stem("idle.reanim.json"), "idle");
        assert_eq!(document_stem("idle.xfl"), "idle");
    }

    #[test]
    fn frame_details_are_compact() {
        let transform = ReanimTransform {
            x: Some(1.0),
            a: Some(0.5),
            ..Default::default()
        };
        assert_eq!(frame_detail(&transform), "x 1 · α 0.5");
    }
}
