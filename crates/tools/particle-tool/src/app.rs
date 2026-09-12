use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use particle_codec::{
    Particles, ParticlesEmitter, ParticlesTrackNode, ParticlesVersion, decode_pc, decode_phone32,
    decode_phone64, encode, trail, trail::Trail, xml,
};
use toolkit_ui::{DropIndicator, InlineNotice, ToolPage, WorkspaceCard, push_application_log};

use crate::platform;

const PARTICLE_PAGE_CSS: Asset = asset!("/assets/particle/page.css");

#[derive(Clone)]
enum ParticleDocument {
    Particle {
        particles: Particles,
        version: ParticlesVersion,
    },
    Trail(Trail),
}

impl ParticleDocument {
    const fn kind_label(&self) -> &'static str {
        match self {
            Self::Particle { .. } => "PARTICLE",
            Self::Trail(_) => "TRAIL",
        }
    }

    fn normalized_xml(&self) -> Result<String, String> {
        match self {
            Self::Particle { particles, .. } => {
                xml::format_particles_xml(particles).map_err(|error| error.to_string())
            }
            Self::Trail(trail) => xml::format_trail_xml(trail).map_err(|error| error.to_string()),
        }
    }

    fn encode(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Particle { particles, version } => {
                encode(particles, *version).map_err(|error| error.to_string())
            }
            Self::Trail(trail) => trail::encode_trail(trail).map_err(|error| error.to_string()),
        }
    }
}

#[derive(Clone)]
struct ParticleTab {
    id: u64,
    name: String,
    source_size: usize,
    document: ParticleDocument,
    applied_xml: String,
    draft_xml: String,
    dirty: bool,
    selected_emitter: usize,
}

impl PartialEq for ParticleTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.source_size == other.source_size
            && self.applied_xml == other.applied_xml
            && self.draft_xml == other.draft_xml
            && self.dirty == other.dirty
            && self.selected_emitter == other.selected_emitter
    }
}

impl ParticleTab {
    fn new(
        id: u64,
        name: String,
        source_size: usize,
        document: ParticleDocument,
        dirty: bool,
    ) -> Result<Self, String> {
        let xml = document.normalized_xml()?;
        Ok(Self {
            id,
            name,
            source_size,
            document,
            applied_xml: xml.clone(),
            draft_xml: xml,
            dirty,
            selected_emitter: 0,
        })
    }

    fn draft_changed(&self) -> bool {
        self.draft_xml != self.applied_xml
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
    tabs: Signal<Vec<ParticleTab>>,
    active_tab_id: Signal<Option<u64>>,
    next_tab_id: Signal<u64>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
}

#[component]
pub fn ParticlePage() -> Element {
    let tabs = use_signal(Vec::<ParticleTab>::new);
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
    toolkit_ui::use_tool_open(toolkit_ui::ToolKind::Particle, busy, move |files| {
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
        document::Stylesheet { href: PARTICLE_PAGE_CSS }
        div {
            class: if dragging() { "particle-page-host is-dragging" } else { "particle-page-host" },
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
                namespace: "particle",
                class: if tabs_snapshot.is_empty() { "particle-page is-empty" } else { "particle-page" },
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
                WorkspaceCard { class: "particle-workspace-card", aria_label: "Particle Editor",
                    if let Some(tab) = active_tab {
                        DocumentWorkspace { tab, signals }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| open_files(files, signals),
                            on_new_particle: move |_| create_particle(signals),
                            on_new_trail: move |_| create_trail(signals),
                        }
                    }
                    if !status_snapshot.message.is_empty() {
                        InlineNotice {
                            class: "particle-status",
                            tone: status_snapshot.tone.class().to_string(),
                            "{status_snapshot.message}"
                        }
                    }
                    if dragging() {
                        DropIndicator { title: "松开以打开 Particle、Trail 或 XML" }
                    }
                    if busy() {
                        div { class: "particle-busy", role: "status",
                            span {}
                            "正在处理 Particle…"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TabStrip(
    tabs: Vec<ParticleTab>,
    active_tab_id: Option<u64>,
    busy: bool,
    on_activate: EventHandler<u64>,
    on_close: EventHandler<u64>,
    on_files: EventHandler<Vec<FileData>>,
) -> Element {
    rsx! {
        nav { class: "particle-tab-strip ui-document-tab-strip",
            div { class: "particle-tab-list ui-document-tab-list", role: "tablist", aria_label: "打开的 Particle 文件",
                for tab in tabs {
                    div {
                        key: "{tab.id}",
                        class: if Some(tab.id) == active_tab_id { "particle-tab ui-document-tab is-active" } else { "particle-tab ui-document-tab" },
                        button {
                            class: "particle-tab-select ui-document-tab-label",
                            role: "tab",
                            aria_selected: Some(tab.id) == active_tab_id,
                            title: "{tab.name}",
                            disabled: busy,
                            onclick: move |_| on_activate.call(tab.id),
                            span { class: "particle-tab-dot ui-document-tab-dot" }
                            span { class: "particle-tab-name ui-document-tab-name", "{tab.name}" }
                            if tab.unsaved() { span { class: "particle-tab-unsaved", title: "存在未导出的更改" } }
                        }
                        button {
                            class: "particle-tab-close ui-document-tab-close",
                            title: "关闭 {tab.name}",
                            aria_label: "关闭 {tab.name}",
                            disabled: busy,
                            onclick: move |_| on_close.call(tab.id),
                            Glyph { name: "close" }
                        }
                    }
                }
                label {
                    class: if busy { "particle-tab-add ui-document-new-tab is-disabled" } else { "particle-tab-add ui-document-new-tab" },
                    title: "添加文件",
                    aria_label: "添加文件",
                    Glyph { name: "add" }
                    input {
                        class: "particle-file-input",
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
    on_new_particle: EventHandler<MouseEvent>,
    on_new_trail: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section { class: "particle-empty",
            div { class: "particle-empty-visual", aria_hidden: "true",
                span { class: "particle-empty-orbit orbit-a" }
                span { class: "particle-empty-orbit orbit-b" }
                span { class: "particle-empty-core" }
                span { class: "particle-empty-dot dot-a" }
                span { class: "particle-empty-dot dot-b" }
                span { class: "particle-empty-dot dot-c" }
            }
            span { class: "particle-eyebrow", "PARTICLE WORKSPACE" }
            h2 { "编辑 PopCap Particle 与 Trail" }
            p { "打开 compiled 或 XML，检查发射器和轨道，在源视图中编辑全部属性，再导出目标平台格式。" }
            div { class: "particle-empty-actions",
                label { class: if busy { "particle-primary is-disabled" } else { "particle-primary" },
                    Glyph { name: "open" }
                    "选择文件"
                    input {
                        class: "particle-file-input",
                        r#type: "file",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() { on_files.call(files); }
                        }
                    }
                }
                button { class: "particle-secondary", disabled: busy, onclick: move |event| on_new_particle.call(event), "新建 Particle" }
                button { class: "particle-secondary", disabled: busy, onclick: move |event| on_new_trail.call(event), "新建 Trail" }
            }
            small { "支持 PC、32/64 位移动端 compiled，以及 Particle/Trail XML" }
        }
    }
}

#[component]
fn DocumentWorkspace(tab: ParticleTab, signals: WorkspaceSignals) -> Element {
    let draft_changed = tab.draft_changed();
    let tab_for_xml = tab.clone();
    let tab_for_binary = tab.clone();
    let source_label = if tab.source_size == 0 {
        "新建文档".to_string()
    } else {
        format!("源文件 {}", format_bytes(tab.source_size))
    };
    let summary = document_summary(&tab.document);

    rsx! {
        div { class: "particle-document",
            header { class: "particle-document-header",
                div { class: "particle-document-title",
                    span { class: "particle-kind-badge", "{tab.document.kind_label()}" }
                    div {
                        h2 { "{tab.name}" }
                        p { "{source_label} · {summary.subtitle}" }
                    }
                }
                div { class: "particle-header-actions",
                    button {
                        class: "particle-action",
                        disabled: (signals.busy)() || draft_changed,
                        title: if draft_changed { "请先应用 XML 更改" } else { "导出 XML" },
                        onclick: move |_| export_xml(tab_for_xml.clone(), signals.tabs, signals.status),
                        Glyph { name: "code" }
                        "XML"
                    }
                    button {
                        class: "particle-action primary",
                        disabled: (signals.busy)() || draft_changed,
                        title: if draft_changed { "请先应用 XML 更改" } else { "导出 compiled" },
                        onclick: move |_| export_binary(tab_for_binary.clone(), signals.tabs, signals.status),
                        Glyph { name: "download" }
                        "Compiled"
                    }
                }
            }

            div { class: "particle-summary-grid",
                SummaryCard { glyph: "emitters", label: summary.primary_label, value: summary.primary_value }
                SummaryCard { glyph: "track", label: "轨道节点", value: summary.track_nodes.to_string() }
                SummaryCard { glyph: "image", label: "图像引用", value: summary.images.to_string() }
                SummaryCard { glyph: "format", label: "输出布局", value: summary.format_label }
            }

            match tab.document.clone() {
                ParticleDocument::Particle { particles, version } => rsx! {
                    ParticleWorkspace {
                        tab_id: tab.id,
                        particles,
                        version,
                        selected_emitter: tab.selected_emitter,
                        signals,
                    }
                },
                ParticleDocument::Trail(_) => rsx! {
                    TrailWorkspace { tab: tab.clone() }
                },
            }

            XmlEditor {
                tab_id: tab.id,
                draft: tab.draft_xml,
                changed: draft_changed,
                busy: (signals.busy)(),
                signals,
            }
        }
    }
}

#[derive(Clone)]
struct DocumentSummary {
    subtitle: String,
    primary_label: &'static str,
    primary_value: String,
    track_nodes: usize,
    images: usize,
    format_label: String,
}

fn document_summary(document: &ParticleDocument) -> DocumentSummary {
    match document {
        ParticleDocument::Particle { particles, version } => DocumentSummary {
            subtitle: format!("{} 个发射器", particles.emitters.len()),
            primary_label: "发射器",
            primary_value: particles.emitters.len().to_string(),
            track_nodes: particles
                .emitters
                .iter()
                .map(emitter_track_node_count)
                .sum(),
            images: particles
                .emitters
                .iter()
                .filter(|emitter| emitter.image.is_some() || emitter.image_path.is_some())
                .count(),
            format_label: version_label(*version).to_string(),
        },
        ParticleDocument::Trail(trail) => DocumentSummary {
            subtitle: format!("最多 {} 个轨迹点", trail.max_points),
            primary_label: "轨迹点",
            primary_value: trail.max_points.max(0).to_string(),
            track_nodes: trail_track_node_count(trail),
            images: usize::from(trail.image.is_some()),
            format_label: "Trail".to_string(),
        },
    }
}

#[component]
fn SummaryCard(glyph: &'static str, label: &'static str, value: String) -> Element {
    rsx! {
        article { class: "particle-summary-card",
            span { Glyph { name: glyph } }
            div { small { "{label}" } strong { "{value}" } }
        }
    }
}

#[component]
fn ParticleWorkspace(
    tab_id: u64,
    particles: Particles,
    version: ParticlesVersion,
    selected_emitter: usize,
    signals: WorkspaceSignals,
) -> Element {
    let selected = particles.emitters.get(selected_emitter).cloned();
    rsx! {
        div { class: "particle-main-grid",
            section { class: "particle-card particle-emitter-card",
                PanelHeading { eyebrow: "STRUCTURE", title: "发射器", glyph: "emitters" }
                div { class: "particle-emitter-tools",
                    span { "{particles.emitters.len()} items" }
                    div {
                        button { title: "添加空发射器", onclick: move |_| add_emitter(tab_id, signals), Glyph { name: "add" } }
                        button { title: "删除当前发射器", disabled: particles.emitters.is_empty(), onclick: move |_| remove_emitter(tab_id, signals), Glyph { name: "trash" } }
                    }
                }
                div { class: "particle-emitter-list",
                    for (index, emitter) in particles.emitters.iter().enumerate() {
                        button {
                            key: "{index}",
                            class: if index == selected_emitter { "particle-emitter-row is-active" } else { "particle-emitter-row" },
                            onclick: move |_| select_emitter(tab_id, index, signals.tabs),
                            span { class: "particle-emitter-index", "{index + 1}" }
                            div {
                                strong { "{emitter_name(emitter, index)}" }
                                small { "{emitter_image_label(emitter)}" }
                            }
                            span { "{emitter_track_count(emitter)}" }
                        }
                    }
                    if particles.emitters.is_empty() {
                        div { class: "particle-list-empty", "当前文档没有发射器" }
                    }
                }
                label { class: "particle-version-field",
                    span { "Compiled 输出布局" }
                    select {
                        value: version_code(version),
                        onchange: move |event| set_version(tab_id, &event.value(), signals),
                        option { value: "pc", "PC · 32-bit" }
                        option { value: "phone32", "Mobile · 32-bit" }
                        option { value: "phone64", "Mobile · 64-bit" }
                    }
                }
            }

            section { class: "particle-card particle-inspector-card",
                if let Some(emitter) = selected {
                    PanelHeading { eyebrow: "EMITTER", title: emitter_name(&emitter, selected_emitter), glyph: "spark" }
                    dl { class: "particle-property-grid",
                        Property { label: "Image", value: emitter.image.clone().or(emitter.image_path.clone()).unwrap_or_else(|| "—".to_string()) }
                        Property { label: "Frames", value: emitter.image_frames.unwrap_or(1).to_string() }
                        Property { label: "Emitter type", value: emitter.emitter_type.unwrap_or(1).to_string() }
                        Property { label: "Flags", value: format!("0x{:08X}", emitter.particle_flags) }
                        Property { label: "Fields", value: emitter.field.as_ref().map_or(0, Vec::len).to_string() }
                        Property { label: "System fields", value: emitter.system_field.as_ref().map_or(0, Vec::len).to_string() }
                    }
                    div { class: "particle-track-heading",
                        span { "ACTIVE TRACKS" }
                        b { "{emitter_track_count(&emitter)}" }
                    }
                    div { class: "particle-track-list",
                        for track in emitter_tracks(&emitter) {
                            div { class: "particle-track-row",
                                div { strong { "{track.name}" } small { "{track.preview}" } }
                                span { "{track.nodes}" }
                            }
                        }
                        if emitter_track_count(&emitter) == 0 {
                            div { class: "particle-list-empty", "该发射器没有轨道节点" }
                        }
                    }
                } else {
                    div { class: "particle-inspector-empty",
                        Glyph { name: "emitters" }
                        strong { "选择或添加发射器" }
                        p { "完整属性可在下方 XML 源视图中编辑。" }
                    }
                }
            }
        }
    }
}

#[component]
fn TrailWorkspace(tab: ParticleTab) -> Element {
    let ParticleDocument::Trail(trail) = tab.document else {
        return rsx! {};
    };
    rsx! {
        div { class: "particle-main-grid is-trail",
            section { class: "particle-card particle-trail-overview",
                PanelHeading { eyebrow: "TRAIL", title: "轨迹属性", glyph: "trail" }
                dl { class: "particle-property-grid trail",
                    Property { label: "Image", value: trail.image.clone().unwrap_or_else(|| "—".to_string()) }
                    Property { label: "Max points", value: trail.max_points.to_string() }
                    Property { label: "Min distance", value: format_float(trail.min_point_distance) }
                    Property { label: "Flags", value: format!("0x{:08X}", trail.trail_flags) }
                }
                p { class: "particle-card-note", "Trail compiled 使用固定布局；在 XML 中修改属性并应用后即可重新编码。" }
            }
            section { class: "particle-card particle-inspector-card",
                PanelHeading { eyebrow: "CURVES", title: "轨迹曲线", glyph: "track" }
                div { class: "particle-track-list trail",
                    for track in trail_tracks(&trail) {
                        div { class: "particle-track-row",
                            div { strong { "{track.name}" } small { "{track.preview}" } }
                            span { "{track.nodes}" }
                        }
                    }
                    if trail_track_node_count(&trail) == 0 {
                        div { class: "particle-list-empty", "当前 Trail 没有曲线节点" }
                    }
                }
            }
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: String, glyph: &'static str) -> Element {
    rsx! {
        div { class: "particle-panel-heading",
            div { span { class: "particle-eyebrow", "{eyebrow}" } h3 { "{title}" } }
            span { Glyph { name: glyph } }
        }
    }
}

#[component]
fn Property(label: &'static str, value: String) -> Element {
    rsx! { div { dt { "{label}" } dd { "{value}" } } }
}

#[component]
fn XmlEditor(
    tab_id: u64,
    draft: String,
    changed: bool,
    busy: bool,
    signals: WorkspaceSignals,
) -> Element {
    rsx! {
        section { class: "particle-card particle-xml-card",
            div { class: "particle-xml-heading",
                div {
                    span { class: "particle-eyebrow", "SOURCE" }
                    h3 { "XML 源视图" }
                    p { "可编辑全部发射器、字段、轨道和曲线属性" }
                }
                div {
                    if changed { span { class: "particle-source-dirty", "未应用" } }
                    button {
                        class: if changed { "particle-apply is-dirty" } else { "particle-apply" },
                        disabled: busy || !changed,
                        onclick: move |_| apply_xml(tab_id, signals),
                        Glyph { name: "check" }
                        "应用并校验"
                    }
                }
            }
            textarea {
                class: "particle-xml-editor",
                "data-text-selectable": "true",
                aria_label: "Particle XML 编辑器",
                spellcheck: false,
                disabled: busy,
                value: "{draft}",
                oninput: move |event| update_draft(tab_id, event.value(), signals.tabs),
            }
        }
    }
}

#[derive(Clone)]
struct TrackSummary {
    name: &'static str,
    nodes: usize,
    preview: String,
}

fn track_summary(
    output: &mut Vec<TrackSummary>,
    name: &'static str,
    nodes: &Option<Vec<ParticlesTrackNode>>,
) {
    let Some(nodes) = nodes.as_ref().filter(|nodes| !nodes.is_empty()) else {
        return;
    };
    output.push(TrackSummary {
        name,
        nodes: nodes.len(),
        preview: xml::format_track_nodes(nodes),
    });
}

fn emitter_tracks(emitter: &ParticlesEmitter) -> Vec<TrackSummary> {
    let mut tracks = Vec::new();
    track_summary(&mut tracks, "SystemDuration", &emitter.system_duration);
    track_summary(
        &mut tracks,
        "CrossFadeDuration",
        &emitter.cross_fade_duration,
    );
    track_summary(&mut tracks, "SpawnRate", &emitter.spawn_rate);
    track_summary(&mut tracks, "SpawnMinActive", &emitter.spawn_min_active);
    track_summary(&mut tracks, "SpawnMaxActive", &emitter.spawn_max_active);
    track_summary(&mut tracks, "SpawnMaxLaunched", &emitter.spawn_max_launched);
    track_summary(&mut tracks, "EmitterRadius", &emitter.emitter_radius);
    track_summary(&mut tracks, "EmitterOffsetX", &emitter.emitter_offset_x);
    track_summary(&mut tracks, "EmitterOffsetY", &emitter.emitter_offset_y);
    track_summary(&mut tracks, "EmitterBoxX", &emitter.emitter_box_x);
    track_summary(&mut tracks, "EmitterBoxY", &emitter.emitter_box_y);
    track_summary(&mut tracks, "EmitterPath", &emitter.emitter_path);
    track_summary(&mut tracks, "EmitterSkewX", &emitter.emitter_skew_x);
    track_summary(&mut tracks, "EmitterSkewY", &emitter.emitter_skew_y);
    track_summary(&mut tracks, "ParticleDuration", &emitter.particle_duration);
    track_summary(&mut tracks, "SystemRed", &emitter.system_red);
    track_summary(&mut tracks, "SystemGreen", &emitter.system_green);
    track_summary(&mut tracks, "SystemBlue", &emitter.system_blue);
    track_summary(&mut tracks, "SystemAlpha", &emitter.system_alpha);
    track_summary(&mut tracks, "SystemBrightness", &emitter.system_brightness);
    track_summary(&mut tracks, "LaunchSpeed", &emitter.launch_speed);
    track_summary(&mut tracks, "LaunchAngle", &emitter.launch_angle);
    track_summary(&mut tracks, "ParticleRed", &emitter.particle_red);
    track_summary(&mut tracks, "ParticleGreen", &emitter.particle_green);
    track_summary(&mut tracks, "ParticleBlue", &emitter.particle_blue);
    track_summary(&mut tracks, "ParticleAlpha", &emitter.particle_alpha);
    track_summary(
        &mut tracks,
        "ParticleBrightness",
        &emitter.particle_brightness,
    );
    track_summary(
        &mut tracks,
        "ParticleSpinAngle",
        &emitter.particle_spin_angle,
    );
    track_summary(
        &mut tracks,
        "ParticleSpinSpeed",
        &emitter.particle_spin_speed,
    );
    track_summary(&mut tracks, "ParticleScale", &emitter.particle_scale);
    track_summary(&mut tracks, "ParticleStretch", &emitter.particle_stretch);
    track_summary(&mut tracks, "CollisionReflect", &emitter.collision_reflect);
    track_summary(&mut tracks, "CollisionSpin", &emitter.collision_spin);
    track_summary(&mut tracks, "ClipTop", &emitter.clip_top);
    track_summary(&mut tracks, "ClipBottom", &emitter.clip_bottom);
    track_summary(&mut tracks, "ClipLeft", &emitter.clip_left);
    track_summary(&mut tracks, "ClipRight", &emitter.clip_right);
    track_summary(&mut tracks, "AnimationRate", &emitter.animation_rate);
    tracks
}

fn trail_tracks(trail: &Trail) -> Vec<TrackSummary> {
    let mut tracks = Vec::new();
    track_summary(&mut tracks, "WidthOverLength", &trail.width_over_length);
    track_summary(&mut tracks, "WidthOverLife", &trail.width_over_life);
    track_summary(&mut tracks, "AlphaOverLength", &trail.alpha_over_length);
    track_summary(&mut tracks, "AlphaOverLife", &trail.alpha_over_life);
    tracks
}

fn emitter_track_count(emitter: &ParticlesEmitter) -> usize {
    emitter_tracks(emitter).len()
}

fn emitter_track_node_count(emitter: &ParticlesEmitter) -> usize {
    emitter_tracks(emitter)
        .into_iter()
        .map(|track| track.nodes)
        .sum::<usize>()
        + emitter
            .field
            .iter()
            .chain(emitter.system_field.iter())
            .flatten()
            .flat_map(|field| [&field.x, &field.y])
            .filter_map(Option::as_ref)
            .map(Vec::len)
            .sum::<usize>()
}

fn trail_track_node_count(trail: &Trail) -> usize {
    trail_tracks(trail)
        .into_iter()
        .map(|track| track.nodes)
        .sum()
}

fn emitter_name(emitter: &ParticlesEmitter, index: usize) -> String {
    emitter
        .name
        .clone()
        .unwrap_or_else(|| format!("Emitter {}", index + 1))
}

fn emitter_image_label(emitter: &ParticlesEmitter) -> &str {
    emitter
        .image
        .as_deref()
        .or(emitter.image_path.as_deref())
        .unwrap_or("No image")
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
                    Ok(document) => {
                        match ParticleTab::new(id, name.clone(), bytes.len(), document, false) {
                            Ok(tab) => {
                                push_application_log(
                                    "PARTICLE",
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
                            "PARTICLE",
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

fn decode_document(name: &str, bytes: &[u8]) -> Result<ParticleDocument, String> {
    let lower = name.to_ascii_lowercase();
    if let Ok(text) = std::str::from_utf8(bytes)
        && looks_like_xml(text)
    {
        if text.contains("<Emitter") {
            return xml::parse_particles_xml(text)
                .map(|particles| ParticleDocument::Particle {
                    particles,
                    version: ParticlesVersion::PC,
                })
                .map_err(|error| error.to_string());
        }
        if lower.contains("trail") || looks_like_trail_xml(text) {
            return xml::parse_trail_xml(text)
                .map(ParticleDocument::Trail)
                .map_err(|error| error.to_string());
        }
        return Err("未识别的 XML：缺少 Emitter 或 Trail 属性".to_string());
    }

    if lower.contains("trail") {
        return trail::decode_trail(bytes)
            .map(ParticleDocument::Trail)
            .map_err(|error| error.to_string());
    }

    decode_particle_binary(bytes).or_else(|particle_error| {
        trail::decode_trail(bytes)
            .map(ParticleDocument::Trail)
            .map_err(|trail_error| format!("Particle: {particle_error}; Trail: {trail_error}"))
    })
}

fn decode_particle_binary(bytes: &[u8]) -> Result<ParticleDocument, String> {
    if let Ok(particles) = decode_pc(bytes) {
        return Ok(ParticleDocument::Particle {
            particles,
            version: ParticlesVersion::PC,
        });
    }
    if let Ok(particles) = decode_phone32(bytes) {
        return Ok(ParticleDocument::Particle {
            particles,
            version: ParticlesVersion::Phone32,
        });
    }
    decode_phone64(bytes)
        .map(|particles| ParticleDocument::Particle {
            particles,
            version: ParticlesVersion::Phone64,
        })
        .map_err(|error| error.to_string())
}

fn looks_like_xml(text: &str) -> bool {
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with('<')
}

fn looks_like_trail_xml(text: &str) -> bool {
    [
        "<MaxPoints",
        "<MinPointDistance",
        "<WidthOverLength",
        "<WidthOverLife",
        "<AlphaOverLength",
        "<AlphaOverLife",
    ]
    .iter()
    .any(|tag| text.contains(tag))
}

fn create_particle(mut signals: WorkspaceSignals) {
    let id = (signals.next_tab_id)();
    signals.next_tab_id.set(id.wrapping_add(1).max(1));
    let document = ParticleDocument::Particle {
        particles: Particles {
            emitters: vec![ParticlesEmitter {
                name: Some("Emitter 1".to_string()),
                ..Default::default()
            }],
        },
        version: ParticlesVersion::PC,
    };
    match ParticleTab::new(id, "untitled.particle.xml".to_string(), 0, document, true) {
        Ok(tab) => {
            signals.tabs.write().push(tab);
            signals.active_tab_id.set(Some(id));
            signals
                .status
                .set(AppStatus::new("已新建 Particle。", StatusTone::Success));
        }
        Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
    }
}

fn create_trail(mut signals: WorkspaceSignals) {
    let id = (signals.next_tab_id)();
    signals.next_tab_id.set(id.wrapping_add(1).max(1));
    let document = ParticleDocument::Trail(Trail {
        max_points: 20,
        min_point_distance: 1.0,
        trail_flags: 0,
        image: None,
        width_over_length: None,
        width_over_life: None,
        alpha_over_length: None,
        alpha_over_life: None,
    });
    match ParticleTab::new(id, "untitled.trail.xml".to_string(), 0, document, true) {
        Ok(tab) => {
            signals.tabs.write().push(tab);
            signals.active_tab_id.set(Some(id));
            signals
                .status
                .set(AppStatus::new("已新建 Trail。", StatusTone::Success));
        }
        Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
    }
}

fn close_tab(
    mut tabs: Signal<Vec<ParticleTab>>,
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

fn select_emitter(tab_id: u64, index: usize, mut tabs: Signal<Vec<ParticleTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.selected_emitter = index;
    }
}

fn update_draft(tab_id: u64, value: String, mut tabs: Signal<Vec<ParticleTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == tab_id) {
        tab.draft_xml = value;
    }
}

fn apply_xml(tab_id: u64, mut signals: WorkspaceSignals) {
    let Some(tab) = (signals.tabs)()
        .iter()
        .find(|tab| tab.id == tab_id)
        .cloned()
    else {
        return;
    };
    let parsed: Result<ParticleDocument, String> = match tab.document {
        ParticleDocument::Particle { version, .. } => {
            if !tab.draft_xml.trim().is_empty() && !tab.draft_xml.contains("<Emitter") {
                Err("Particle XML 缺少 Emitter 元素".to_string())
            } else {
                xml::parse_particles_xml(&tab.draft_xml)
                    .map(|particles| ParticleDocument::Particle { particles, version })
                    .map_err(|error| error.to_string())
            }
        }
        ParticleDocument::Trail(_) => {
            if !looks_like_trail_xml(&tab.draft_xml) {
                Err("Trail XML 缺少 MaxPoints、曲线或距离属性".to_string())
            } else {
                xml::parse_trail_xml(&tab.draft_xml)
                    .map(ParticleDocument::Trail)
                    .map_err(|error| error.to_string())
            }
        }
    };
    match parsed {
        Ok(document) => match document.normalized_xml() {
            Ok(normalized) => {
                if let Some(current) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
                {
                    current.document = document;
                    current.applied_xml = normalized.clone();
                    current.draft_xml = normalized;
                    current.dirty = true;
                    if let ParticleDocument::Particle { particles, .. } = &current.document {
                        current.selected_emitter = current
                            .selected_emitter
                            .min(particles.emitters.len().saturating_sub(1));
                    }
                }
                push_application_log(
                    "PARTICLE",
                    "INFO",
                    "APPLY",
                    format!("Applied XML for {}", tab.name),
                );
                signals.status.set(AppStatus::new(
                    "XML 已通过校验并应用。",
                    StatusTone::Success,
                ));
            }
            Err(error) => signals.status.set(AppStatus::new(error, StatusTone::Error)),
        },
        Err(error) => {
            let message = format!("XML 无法解析：{error}");
            push_application_log("PARTICLE", "ERROR", "APPLY", message.clone());
            signals
                .status
                .set(AppStatus::new(message, StatusTone::Error));
        }
    }
}

fn add_emitter(tab_id: u64, mut signals: WorkspaceSignals) {
    let mut applied = None;
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
        && let ParticleDocument::Particle { particles, .. } = &mut tab.document
    {
        let index = particles.emitters.len();
        particles.emitters.push(ParticlesEmitter {
            name: Some(format!("Emitter {}", index + 1)),
            ..Default::default()
        });
        tab.selected_emitter = index;
        tab.dirty = true;
        applied = tab.document.normalized_xml().ok();
    }
    if let Some(xml) = applied
        && let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
    {
        tab.applied_xml = xml.clone();
        tab.draft_xml = xml;
        signals
            .status
            .set(AppStatus::new("已添加空发射器。", StatusTone::Success));
    }
}

fn remove_emitter(tab_id: u64, mut signals: WorkspaceSignals) {
    let mut applied = None;
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
        && let ParticleDocument::Particle { particles, .. } = &mut tab.document
        && !particles.emitters.is_empty()
    {
        let index = tab.selected_emitter.min(particles.emitters.len() - 1);
        particles.emitters.remove(index);
        tab.selected_emitter = index.min(particles.emitters.len().saturating_sub(1));
        tab.dirty = true;
        applied = tab.document.normalized_xml().ok();
    }
    if let Some(xml) = applied
        && let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
    {
        tab.applied_xml = xml.clone();
        tab.draft_xml = xml;
        signals
            .status
            .set(AppStatus::new("已删除发射器。", StatusTone::Warning));
    }
}

fn set_version(tab_id: u64, value: &str, mut signals: WorkspaceSignals) {
    let version = match value {
        "phone32" => ParticlesVersion::Phone32,
        "phone64" => ParticlesVersion::Phone64,
        _ => ParticlesVersion::PC,
    };
    if let Some(tab) = signals.tabs.write().iter_mut().find(|tab| tab.id == tab_id)
        && let ParticleDocument::Particle {
            version: current, ..
        } = &mut tab.document
    {
        *current = version;
        tab.dirty = true;
        signals.status.set(AppStatus::new(
            format!("输出布局已改为 {}。", version_label(version)),
            StatusTone::Success,
        ));
    }
}

fn export_xml(tab: ParticleTab, mut tabs: Signal<Vec<ParticleTab>>, mut status: Signal<AppStatus>) {
    spawn(async move {
        let name = xml_output_name(&tab);
        if export_bytes(
            &name,
            "Particle XML",
            &["xml"],
            tab.applied_xml.as_bytes(),
            "XML",
            &mut status,
        )
        .await
        {
            mark_exported(tab.id, &mut tabs);
        }
    });
}

fn export_binary(
    tab: ParticleTab,
    mut tabs: Signal<Vec<ParticleTab>>,
    mut status: Signal<AppStatus>,
) {
    spawn(async move {
        match tab.document.encode() {
            Ok(bytes) => {
                let name = binary_output_name(&tab);
                if export_bytes(
                    &name,
                    "Particle compiled",
                    &["compiled"],
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
                "PARTICLE",
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
                "PARTICLE",
                "ERROR",
                "EXPORT",
                format!("Export {name} failed: {error}"),
            );
            status.set(AppStatus::new(error, StatusTone::Error));
            false
        }
    }
}

fn mark_exported(id: u64, tabs: &mut Signal<Vec<ParticleTab>>) {
    if let Some(tab) = tabs.write().iter_mut().find(|tab| tab.id == id) {
        tab.dirty = false;
    }
}

fn xml_output_name(tab: &ParticleTab) -> String {
    let stem = document_stem(&tab.name);
    match tab.document {
        ParticleDocument::Particle { .. } => format!("{stem}.particle.xml"),
        ParticleDocument::Trail(_) => format!("{stem}.trail.xml"),
    }
}

fn binary_output_name(tab: &ParticleTab) -> String {
    let stem = document_stem(&tab.name);
    match tab.document {
        ParticleDocument::Particle { .. } => format!("{stem}.particle.compiled"),
        ParticleDocument::Trail(_) => format!("{stem}.trail.compiled"),
    }
}

fn document_stem(name: &str) -> String {
    let mut stem = name.to_string();
    for suffix in [
        ".particle.compiled",
        ".trail.compiled",
        ".particle.xml",
        ".trail.xml",
        ".compiled",
        ".xml",
    ] {
        if stem.to_ascii_lowercase().ends_with(suffix) {
            stem.truncate(stem.len() - suffix.len());
            break;
        }
    }
    if stem.is_empty() {
        "untitled".to_string()
    } else {
        stem
    }
}

const fn version_code(version: ParticlesVersion) -> &'static str {
    match version {
        ParticlesVersion::PC => "pc",
        ParticlesVersion::Phone32 => "phone32",
        ParticlesVersion::Phone64 => "phone64",
    }
}

const fn version_label(version: ParticlesVersion) -> &'static str {
    match version {
        ParticlesVersion::PC => "PC · 32-bit",
        ParticlesVersion::Phone32 => "Mobile · 32-bit",
        ParticlesVersion::Phone64 => "Mobile · 64-bit",
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
        "add" => "M12 5v14M5 12h14",
        "close" => "M7 7l10 10M17 7L7 17",
        "download" => "M12 3v12M7 10l5 5 5-5M5 20h14",
        "code" => "M8 9l-4 3 4 3M16 9l4 3-4 3M14 5l-4 14",
        "emitters" => {
            "M12 4v4M12 16v4M4 12h4M16 12h4M6.3 6.3l2.8 2.8M14.9 14.9l2.8 2.8M17.7 6.3l-2.8 2.8M9.1 14.9l-2.8 2.8M12 9a3 3 0 1 1 0 6 3 3 0 0 1 0-6"
        }
        "track" => "M4 17c3-8 6-8 8-2s5 4 8-8M4 17h4M16 7h4",
        "image" => "M4 5h16v14H4zM8 10a2 2 0 1 0 0-4M4 16l5-5 4 4 2-2 5 5",
        "format" => "M5 4h14v16H5zM8 8h8M8 12h8M8 16h5",
        "spark" => "M12 3l1.5 5.5L19 10l-5.5 1.5L12 17l-1.5-5.5L5 10l5.5-1.5z",
        "trail" => "M4 17c4-10 8-10 10-4s4 4 6-6M4 17h5",
        "trash" => "M5 7h14M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5",
        "check" => "M5 12l4 4L19 6",
        _ => "M4 12h16",
    };
    rsx! {
        svg { class: "particle-glyph", view_box: "0 0 24 24",
            path { d: path, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_particle_and_trail_xml() {
        let particle = decode_document("test.xml", b"<Emitter><Name>A</Name></Emitter>").unwrap();
        assert!(matches!(particle, ParticleDocument::Particle { .. }));

        let trail = decode_document("test.trail.xml", b"<MaxPoints>20</MaxPoints>").unwrap();
        assert!(matches!(trail, ParticleDocument::Trail(_)));

        assert!(decode_document("unknown.xml", b"<Unknown />").is_err());
    }

    #[test]
    fn output_names_do_not_stack_extensions() {
        assert_eq!(document_stem("fire.particle.compiled"), "fire");
        assert_eq!(document_stem("ice.trail.xml"), "ice");
    }

    #[test]
    fn summary_counts_track_nodes() {
        let emitter = ParticlesEmitter {
            spawn_rate: Some(vec![ParticlesTrackNode::default(); 3]),
            particle_scale: Some(vec![ParticlesTrackNode::default(); 2]),
            ..Default::default()
        };
        assert_eq!(emitter_track_count(&emitter), 2);
        assert_eq!(emitter_track_node_count(&emitter), 5);
    }

    #[test]
    fn compiled_layout_detection_roundtrips_all_versions() {
        let particles = Particles {
            emitters: vec![ParticlesEmitter {
                name: Some("Test".to_string()),
                image: Some("12".to_string()),
                ..Default::default()
            }],
        };
        for expected in [
            ParticlesVersion::PC,
            ParticlesVersion::Phone32,
            ParticlesVersion::Phone64,
        ] {
            let bytes = encode(&particles, expected).unwrap();
            let decoded = decode_document("test.compiled", &bytes).unwrap();
            let ParticleDocument::Particle { version, .. } = decoded else {
                panic!("expected Particle document");
            };
            assert_eq!(version, expected);
        }
    }
}
