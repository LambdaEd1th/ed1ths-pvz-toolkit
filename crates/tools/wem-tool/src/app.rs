use crate::{platform, processing};
use dioxus::prelude::*;
use dioxus_html::{FileData, HasFileData};
use std::sync::Arc;
use toolkit_ui::{InlineNotice, ToolPage, WorkspaceCard, push_application_log};
use wem_audio_worker::{
    AudioInfo, ConvertRequest, ConvertedAudio, ExportTarget, PrepareRequest, PreparedAudio,
    SourceFormat,
};

const WEM_PAGE_CSS: Asset = asset!("/assets/wem/page.css");
const WEM_PLAYER_JS: Asset = asset!("/assets/wem/player.js");

#[derive(Clone)]
struct AudioDocument {
    name: String,
    bytes: Arc<[u8]>,
    info: AudioInfo,
    playback_url: String,
    warning: Option<String>,
}

#[derive(Clone)]
struct OutputDocument {
    file: Arc<ConvertedAudio>,
}

#[derive(Clone)]
struct AudioTab {
    id: u64,
    audio: AudioDocument,
    output: Option<OutputDocument>,
    target: ExportTarget,
}

impl AudioTab {
    fn new(id: u64, audio: AudioDocument) -> Self {
        Self {
            id,
            audio,
            output: None,
            target: ExportTarget::Automatic,
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

#[derive(Clone, Copy)]
struct TargetOption {
    target: ExportTarget,
    label: &'static str,
    detail: &'static str,
}

#[component]
pub fn WemAudioPage() -> Element {
    let tabs = use_signal(Vec::<AudioTab>::new);
    let mut active_tab_id = use_signal(|| None::<u64>);
    let next_tab_id = use_signal(|| 1_u64);
    let busy = use_signal(|| false);
    let mut dragging = use_signal(|| false);
    let mut status = use_signal(AppStatus::default);
    let mut generation = use_signal(|| 0_u64);
    let mut target_menu_open = use_signal(|| false);

    let tabs_snapshot = tabs();
    let active_tab_id_snapshot = active_tab_id();
    let active_tab_snapshot = active_tab_id_snapshot
        .and_then(|active_id| tabs_snapshot.iter().find(|tab| tab.id == active_id))
        .cloned();
    let document_snapshot = active_tab_snapshot.as_ref().map(|tab| &tab.audio);
    let output_snapshot = active_tab_snapshot
        .as_ref()
        .and_then(|tab| tab.output.as_ref());
    let target_snapshot = active_tab_snapshot
        .as_ref()
        .map(|tab| tab.target)
        .unwrap_or(ExportTarget::Automatic);
    let status_snapshot = status();
    let playback_key = document_snapshot
        .as_ref()
        .map(|audio| audio.playback_url.clone())
        .unwrap_or_default();
    use_effect(use_reactive(&playback_key, move |playback_key| {
        if playback_key.is_empty() {
            document::eval("window.wemPlayer?.unbind?.();");
        } else {
            document::eval(
                "requestAnimationFrame(() => { \
                    document.querySelector('[data-tool=\"wem\"] .wem-page')?.scrollTo({ top: 0, left: 0 }); \
                    window.wemPlayer?.bind?.(); \
                });",
            );
        }
    }));
    let active_tab_scroll_key = active_tab_id_snapshot.unwrap_or_default();
    use_effect(use_reactive(
        &active_tab_scroll_key,
        move |active_tab_scroll_key| {
            if active_tab_scroll_key != 0 {
                document::eval(&format!(
                    "requestAnimationFrame(() => document.querySelector('[data-tool=\"wem\"] [data-wem-tab-id=\"{active_tab_scroll_key}\"]')?.scrollIntoView({{ block: 'nearest', inline: 'nearest' }}));"
                ));
            }
        },
    ));
    let target_options = document_snapshot.map(available_targets).unwrap_or_default();
    let selected_target = if target_options
        .iter()
        .any(|option| option.target == target_snapshot)
    {
        target_snapshot
    } else {
        ExportTarget::Automatic
    };
    let selected_detail = target_options
        .iter()
        .find(|option| option.target == selected_target)
        .map(|option| option.detail)
        .unwrap_or("打开音频后选择输出格式");
    let selected_label = target_options
        .iter()
        .find(|option| option.target == selected_target)
        .map(|option| option.label)
        .unwrap_or("选择输出格式");
    let host_class = if dragging() {
        "wem-page-host is-dragging"
    } else {
        "wem-page-host"
    };

    rsx! {
        document::Script { src: WEM_PLAYER_JS }
        document::Stylesheet { href: WEM_PAGE_CSS }
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
                if files.is_empty() {
                    return;
                }
                load_files(
                    files,
                    tabs,
                    active_tab_id,
                    next_tab_id,
                    busy,
                    status,
                    generation,
                );
            },
            ToolPage {
                namespace: "wem",
                class: if tabs_snapshot.is_empty() {
                    "wem-page is-empty".to_string()
                } else {
                    "wem-page".to_string()
                },
                if !tabs_snapshot.is_empty() {
                    div { class: "wem-tab-strip",
                        div {
                            class: "wem-tab-list",
                            role: "tablist",
                            aria_label: "打开的音频",
                            for tab in &tabs_snapshot {
                                div {
                                    key: "{tab.id}",
                                    class: if Some(tab.id) == active_tab_id_snapshot {
                                        "wem-tab is-active"
                                    } else {
                                        "wem-tab"
                                    },
                                    "data-wem-tab-id": "{tab.id}",
                                    button {
                                        r#type: "button",
                                        class: "wem-tab-select",
                                        role: "tab",
                                        aria_selected: Some(tab.id) == active_tab_id_snapshot,
                                        tabindex: if Some(tab.id) == active_tab_id_snapshot { "0" } else { "-1" },
                                        title: "{tab.audio.name}",
                                        disabled: busy(),
                                        onclick: {
                                            let tab_id = tab.id;
                                            move |_| {
                                                if active_tab_id() != Some(tab_id) {
                                                    pause_player();
                                                    active_tab_id.set(Some(tab_id));
                                                    status.set(AppStatus::default());
                                                    target_menu_open.set(false);
                                                }
                                            }
                                        },
                                        span { class: "wem-tab-dot" }
                                        span { class: "wem-tab-name", "{tab.audio.name}" }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "wem-tab-close",
                                        title: "关闭 {tab.audio.name}",
                                        aria_label: "关闭 {tab.audio.name}",
                                        disabled: busy(),
                                        onclick: {
                                            let tab_id = tab.id;
                                            move |_| {
                                                let was_active = active_tab_id() == Some(tab_id);
                                                if was_active {
                                                    pause_player();
                                                }
                                                if close_tab(tabs, active_tab_id, tab_id) {
                                                    generation.set(generation().wrapping_add(1).max(1));
                                                    status.set(AppStatus::default());
                                                    target_menu_open.set(false);
                                                }
                                            }
                                        },
                                        Glyph { name: "close" }
                                    }
                                }
                            }
                        }
                        label {
                            class: if busy() {
                                "wem-tab-add is-disabled"
                            } else {
                                "wem-tab-add"
                            },
                            title: "添加音频",
                            aria_label: "添加音频",
                            Glyph { name: "open" }
                            input {
                                class: "wem-file-input",
                                r#type: "file",
                                accept: ".wem,.wav,.ogg,.oga,.m4a,.mp4,.aac,audio/*",
                                multiple: true,
                                disabled: busy(),
                                onchange: move |event| {
                                    let files = event.files();
                                    if !files.is_empty() {
                                        load_files(
                                            files,
                                            tabs,
                                            active_tab_id,
                                            next_tab_id,
                                            busy,
                                            status,
                                            generation,
                                        );
                                    }
                                },
                            }
                        }
                    }
                }

                main { class: "wem-workspace",
                    if let Some(audio) = document_snapshot.as_ref() {
                        WorkspaceCard { class: "wem-source-panel".to_string(), aria_label: "源音频".to_string(),
                            PanelHeading {
                                eyebrow: "SOURCE",
                                title: "源音频",
                                glyph: "audio",
                            }
                            div { class: "wem-source-identity",
                                div { class: "wem-source-icon", {source_glyph(audio.info.format)} }
                                div {
                                    strong { title: "{audio.name}", "{audio.name}" }
                                    span { "{format_label(audio.info.format)} · {audio.info.codec}" }
                                }
                            }
                            dl { class: "wem-metadata",
                                MetadataRow { label: "编码", value: audio.info.codec.clone() }
                                MetadataRow {
                                    label: "声道",
                                    value: audio.info.channels.map(channel_label).unwrap_or_else(|| "未知".to_string()),
                                }
                                MetadataRow {
                                    label: "采样率",
                                    value: audio.info.sample_rate.map(sample_rate_label).unwrap_or_else(|| "未知".to_string()),
                                }
                                MetadataRow { label: "文件大小", value: format_bytes(audio.info.byte_size) }
                            }
                            div { class: "wem-route-summary",
                                span { "{format_short(audio.info.format)}" }
                                i { aria_hidden: "true", "→" }
                                span { "{target_short(selected_target, audio)}" }
                            }
                        }

                        WorkspaceCard { class: "wem-player-panel".to_string(), aria_label: "音频播放器".to_string(),
                            div { class: "wem-player-heading",
                                div {
                                    span { class: "wem-panel-eyebrow", "PLAYER" }
                                    h2 { "预听" }
                                }
                                span { class: "wem-player-state",
                                    i {}
                                    span { class: "wem-player-state-label", "就绪" }
                                }
                            }
                            div { class: "wem-waveform", aria_hidden: "true",
                                div { class: "wem-waveform-glow" }
                                canvas { class: "wem-waveform-canvas" }
                            }
                            div { class: "wem-now-playing",
                                span { class: "wem-now-playing-mark", {source_glyph(audio.info.format)} }
                                div {
                                    strong { "{audio.name}" }
                                    span { "{audio.info.codec} · {audio.info.channels.map(channel_label).unwrap_or_else(|| \"未知声道\".to_string())}" }
                                }
                            }
                            audio {
                                class: "wem-audio-player",
                                preload: "metadata",
                                src: "{audio.playback_url}",
                                onmounted: move |_| {
                                    let _ = document::eval(
                                        "requestAnimationFrame(() => window.wemPlayer?.bind?.());",
                                    );
                                },
                            }
                            div {
                                class: "wem-player-controls",
                                role: "group",
                                aria_label: "音频播放控制",
                                tabindex: "0",
                                "data-playing": "false",
                                "data-muted": "false",
                                div { class: "wem-transport-controls",
                                    button {
                                        r#type: "button",
                                        class: "wem-player-button wem-skip-button",
                                        title: "后退 15 秒",
                                        aria_label: "后退 15 秒",
                                        "data-wem-action": "skip-back",
                                        SkipGlyph { forward: false }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "wem-player-button wem-play-button",
                                        title: "播放",
                                        aria_label: "播放",
                                        "data-wem-action": "play",
                                        span { class: "wem-play-glyph", Glyph { name: "play" } }
                                        span { class: "wem-pause-glyph", Glyph { name: "pause" } }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "wem-player-button wem-skip-button",
                                        title: "前进 15 秒",
                                        aria_label: "前进 15 秒",
                                        "data-wem-action": "skip-forward",
                                        SkipGlyph { forward: true }
                                    }
                                }
                                div { class: "wem-player-timeline",
                                    output {
                                        class: "wem-player-time wem-current-time",
                                        aria_label: "当前时间",
                                        "0:00"
                                    }
                                    label {
                                        class: "wem-seek-control",
                                        title: "播放进度",
                                        span { class: "wem-seek-track" }
                                        span { class: "wem-seek-buffered" }
                                        span { class: "wem-seek-played" }
                                        input {
                                            class: "wem-seek-input",
                                            r#type: "range",
                                            min: "0",
                                            max: "1000",
                                            step: "1",
                                            value: "0",
                                            aria_label: "播放进度",
                                            "data-wem-action": "seek",
                                        }
                                    }
                                    output {
                                        class: "wem-player-time wem-duration-time",
                                        aria_label: "总时长",
                                        "0:00"
                                    }
                                }
                                div { class: "wem-audio-controls",
                                    button {
                                        r#type: "button",
                                        class: "wem-player-button wem-volume-button",
                                        title: "静音",
                                        aria_label: "静音",
                                        "data-wem-action": "mute",
                                        span { class: "wem-volume-glyph", Glyph { name: "volume" } }
                                        span { class: "wem-muted-glyph", Glyph { name: "muted" } }
                                    }
                                    label { class: "wem-volume-control", title: "音量",
                                        input {
                                            class: "wem-volume-input",
                                            r#type: "range",
                                            min: "0",
                                            max: "100",
                                            step: "1",
                                            value: "100",
                                            aria_label: "音量",
                                            "data-wem-action": "volume",
                                        }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "wem-rate-button",
                                        title: "切换播放速度",
                                        aria_label: "播放速度 1 倍",
                                        "data-wem-action": "rate",
                                        span { class: "wem-rate-value", "1×" }
                                    }
                                }
                            }
                            if let Some(warning) = audio.warning.as_ref() {
                                InlineNotice { class: "wem-player-warning".to_string(), tone: "warning".to_string(),
                                    Glyph { name: "warning" }
                                    span { "{warning}" }
                                }
                            }
                        }

                        WorkspaceCard {
                            class: if target_menu_open() {
                                "wem-convert-panel has-open-select".to_string()
                            } else {
                                "wem-convert-panel".to_string()
                            },
                            aria_label: "转换与导出".to_string(),
                            PanelHeading {
                                eyebrow: "CONVERT",
                                title: "转换与导出",
                                glyph: "convert",
                            }
                            div { class: "wem-field",
                                span { id: "wem-output-format-label", "输出格式" }
                                div {
                                    class: if target_menu_open() {
                                        "wem-select-wrap is-open"
                                    } else {
                                        "wem-select-wrap"
                                    },
                                    if target_menu_open() {
                                        div {
                                            class: "wem-select-dismiss",
                                            aria_hidden: "true",
                                            onclick: move |_| target_menu_open.set(false),
                                        }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "wem-select-trigger",
                                        disabled: busy(),
                                        aria_labelledby: "wem-output-format-label",
                                        aria_haspopup: "listbox",
                                        aria_expanded: target_menu_open(),
                                        onclick: move |_| target_menu_open.toggle(),
                                        onkeydown: move |event| {
                                            if event.key() == Key::Escape {
                                                target_menu_open.set(false);
                                            }
                                        },
                                        span { class: "wem-select-value",
                                            span { class: "wem-select-dot", i {} }
                                            span { "{selected_label}" }
                                        }
                                        span { class: "wem-select-chevron",
                                            Glyph { name: "chevron" }
                                        }
                                    }
                                    div {
                                        class: "wem-select-menu",
                                        role: "listbox",
                                        aria_label: "输出格式",
                                        onkeydown: move |event| {
                                            if event.key() == Key::Escape {
                                                target_menu_open.set(false);
                                            }
                                        },
                                        for option in &target_options {
                                            button {
                                                r#type: "button",
                                                class: if option.target == selected_target {
                                                    "wem-select-option is-selected"
                                                } else {
                                                    "wem-select-option"
                                                },
                                                role: "option",
                                                aria_selected: option.target == selected_target,
                                                onclick: {
                                                    let next = option.target;
                                                    move |_| {
                                                        if let Some(tab_id) = active_tab_id() {
                                                            update_tab(tabs, tab_id, |tab| {
                                                                tab.target = next;
                                                                tab.output = None;
                                                            });
                                                        }
                                                        status.set(AppStatus::default());
                                                        target_menu_open.set(false);
                                                    }
                                                },
                                                span { class: "wem-select-option-dot" }
                                                span { class: "wem-select-option-label", "{option.label}" }
                                                span { class: "wem-select-check",
                                                    Glyph { name: "check" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            p { class: "wem-target-detail", "{selected_detail}" }
                            button {
                                r#type: "button",
                                class: "wem-convert-button",
                                disabled: busy(),
                                onclick: move |_| {
                                    convert_document(
                                        tabs,
                                        active_tab_id,
                                        selected_target,
                                        busy,
                                        status,
                                        generation,
                                    );
                                },
                                Glyph { name: "convert" }
                                span { if busy() { "正在转换…" } else { "开始转换" } }
                            }

                            if let Some(result) = output_snapshot.as_ref() {
                                div { class: "wem-output-card",
                                    div { class: "wem-output-top",
                                        span { class: "wem-output-icon", {output_glyph(&result.file.name)} }
                                        div {
                                            strong { title: "{result.file.name}", "{result.file.name}" }
                                            span { "{format_bytes(result.file.data.len() as u64)} · 已生成" }
                                        }
                                        span { class: "wem-output-check", "✓" }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "wem-export-button",
                                        onclick: move |_| save_output(tabs, active_tab_id, status),
                                        Glyph { name: "download" }
                                        "导出文件"
                                    }
                                }
                            } else {
                                div { class: "wem-output-placeholder",
                                    Glyph { name: "spark" }
                                    div {
                                        strong { "输出将在这里出现" }
                                        span { "转换完成后可直接导出" }
                                    }
                                }
                            }
                        }
                    } else {
                        EmptyWorkspace {
                            busy: busy(),
                            on_files: move |files| {
                                load_files(
                                    files,
                                    tabs,
                                    active_tab_id,
                                    next_tab_id,
                                    busy,
                                    status,
                                    generation,
                                );
                            },
                        }
                    }
                }

                if !status_snapshot.message.is_empty() {
                    div {
                        class: "wem-status wem-status--{status_snapshot.tone.class()}",
                        role: "status",
                        span { class: "wem-status-dot" }
                        "{status_snapshot.message}"
                    }
                }

                if dragging() {
                    div { class: "wem-drop-overlay", aria_hidden: "true",
                        div {
                            Glyph { name: "open" }
                            strong { "松开以打开音频" }
                            span { "WEM、WAV、OGG、M4A 或 AAC" }
                        }
                    }
                }
            }
        }
    }
}

fn load_files(
    files: Vec<FileData>,
    tabs: Signal<Vec<AudioTab>>,
    active_tab_id: Signal<Option<u64>>,
    next_tab_id: Signal<u64>,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
    generation: Signal<u64>,
) {
    if files.is_empty() {
        return;
    }
    let mut tabs = tabs;
    let mut active_tab_id = active_tab_id;
    let mut next_tab_id = next_tab_id;
    let mut busy = busy;
    let mut status = status;
    let mut generation = generation;
    let pending = files
        .into_iter()
        .map(|file| {
            let id = next_tab_id();
            next_tab_id.set(id.wrapping_add(1).max(1));
            (id, file)
        })
        .collect::<Vec<_>>();
    let file_count = pending.len();
    let request_id = generation().wrapping_add(1).max(1);
    generation.set(request_id);
    busy.set(true);
    status.set(AppStatus::new(
        if file_count == 1 {
            "正在读取并分析音频…".to_string()
        } else {
            format!("正在读取并分析 {file_count} 个音频…")
        },
        StatusTone::Neutral,
    ));
    spawn(async move {
        let mut opened_tabs = Vec::with_capacity(file_count);
        let mut errors = Vec::new();
        for (index, (tab_id, file)) in pending.into_iter().enumerate() {
            if generation() != request_id {
                release_tabs(&opened_tabs);
                return;
            }
            if file_count > 1 {
                status.set(AppStatus::new(
                    format!("正在读取并分析音频（{}/{file_count}）…", index + 1),
                    StatusTone::Neutral,
                ));
            }
            let name = file.name();
            let result = match file.read_bytes().await {
                Ok(bytes) => {
                    let bytes = bytes.to_vec();
                    processing::prepare_audio(PrepareRequest {
                        name: name.clone(),
                        data: bytes.clone(),
                    })
                    .await
                    .and_then(|prepared| install_audio(name.clone(), bytes, prepared))
                }
                Err(error) => Err(format!("无法读取文件：{error}")),
            };
            match result {
                Ok(audio) => {
                    let summary = format!(
                        "Opened {} ({}, {})",
                        audio.name,
                        format_label(audio.info.format),
                        audio.info.codec
                    );
                    push_application_log("WEM", "INFO", "OPEN", &summary);
                    opened_tabs.push(AudioTab::new(tab_id, audio));
                }
                Err(error) => {
                    push_application_log(
                        "WEM",
                        "ERROR",
                        "OPEN",
                        format!("Open {name} failed: {error}"),
                    );
                    errors.push(format!("{name}：{error}"));
                }
            }
        }
        if generation() != request_id {
            release_tabs(&opened_tabs);
            return;
        }
        busy.set(false);
        if !opened_tabs.is_empty() {
            let last_id = opened_tabs.last().map(|tab| tab.id);
            let warning_count = opened_tabs
                .iter()
                .filter(|tab| tab.audio.warning.is_some())
                .count();
            let opened_count = opened_tabs.len();
            tabs.write().extend(opened_tabs);
            active_tab_id.set(last_id);
            let tone = if errors.is_empty() && warning_count == 0 {
                StatusTone::Success
            } else {
                StatusTone::Warning
            };
            status.set(AppStatus::new(
                if errors.is_empty() {
                    if opened_count == 1 {
                        "音频已就绪，可以播放或转换。".to_string()
                    } else {
                        format!("已打开 {opened_count} 个音频。")
                    }
                } else {
                    format!(
                        "已打开 {opened_count} 个音频，{} 个文件打开失败。",
                        errors.len()
                    )
                },
                tone,
            ));
        } else {
            status.set(AppStatus::new(
                if errors.len() == 1 {
                    errors.remove(0)
                } else {
                    format!("{} 个音频均无法打开。", errors.len())
                },
                StatusTone::Error,
            ));
        }
    });
}

fn update_tab(
    mut tabs: Signal<Vec<AudioTab>>,
    tab_id: u64,
    update: impl FnOnce(&mut AudioTab),
) -> bool {
    let mut tabs = tabs.write();
    let Some(tab) = tabs.iter_mut().find(|tab| tab.id == tab_id) else {
        return false;
    };
    update(tab);
    true
}

fn close_tab(
    mut tabs: Signal<Vec<AudioTab>>,
    mut active_tab_id: Signal<Option<u64>>,
    tab_id: u64,
) -> bool {
    let was_active = active_tab_id() == Some(tab_id);
    let mut tabs = tabs.write();
    let Some(index) = tabs.iter().position(|tab| tab.id == tab_id) else {
        return false;
    };
    let removed = tabs.remove(index);
    platform::release_audio_url(&removed.audio.playback_url);
    let next_active = if was_active {
        tabs.get(index)
            .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
            .map(|tab| tab.id)
    } else {
        None
    };
    drop(tabs);
    if was_active {
        active_tab_id.set(next_active);
    }
    true
}

fn release_tabs(tabs: &[AudioTab]) {
    for tab in tabs {
        platform::release_audio_url(&tab.audio.playback_url);
    }
}

fn pause_player() {
    document::eval(
        "document.querySelectorAll('[data-tool=\"wem\"] audio').forEach((audio) => { audio.pause(); audio.currentTime = 0; });",
    );
}

fn install_audio(
    name: String,
    bytes: Vec<u8>,
    prepared: PreparedAudio,
) -> Result<AudioDocument, String> {
    let playback_url = platform::audio_url(&prepared.playback, &prepared.playback_mime)?;
    Ok(AudioDocument {
        name,
        bytes: Arc::from(bytes),
        info: prepared.info,
        playback_url,
        warning: prepared.warning,
    })
}

fn convert_document(
    tabs: Signal<Vec<AudioTab>>,
    active_tab_id: Signal<Option<u64>>,
    target: ExportTarget,
    busy: Signal<bool>,
    status: Signal<AppStatus>,
    generation: Signal<u64>,
) {
    let Some(tab_id) = active_tab_id.peek().as_ref().copied() else {
        return;
    };
    let Some(audio) = tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .map(|tab| tab.audio.clone())
    else {
        return;
    };
    let mut busy = busy;
    let mut status = status;
    let request_id = generation();
    busy.set(true);
    update_tab(tabs, tab_id, |tab| tab.output = None);
    status.set(AppStatus::new("正在转换音频…", StatusTone::Neutral));
    spawn(async move {
        let result = processing::convert_audio(ConvertRequest {
            name: audio.name.clone(),
            data: audio.bytes.as_ref().to_vec(),
            target,
        })
        .await;
        if generation() != request_id {
            return;
        }
        busy.set(false);
        match result {
            Ok(file) => {
                push_application_log(
                    "WEM",
                    "INFO",
                    "CONVERT",
                    format!(
                        "Converted {} -> {} ({} bytes)",
                        audio.name,
                        file.name,
                        file.data.len()
                    ),
                );
                status.set(AppStatus::new(
                    "转换完成，可以导出文件。",
                    StatusTone::Success,
                ));
                update_tab(tabs, tab_id, |tab| {
                    tab.output = Some(OutputDocument {
                        file: Arc::new(file),
                    });
                });
            }
            Err(error) => {
                push_application_log(
                    "WEM",
                    "ERROR",
                    "CONVERT",
                    format!("Convert failed: {error}"),
                );
                status.set(AppStatus::new(error, StatusTone::Error));
            }
        }
    });
}

fn save_output(
    tabs: Signal<Vec<AudioTab>>,
    active_tab_id: Signal<Option<u64>>,
    status: Signal<AppStatus>,
) {
    let Some(tab_id) = active_tab_id.peek().as_ref().copied() else {
        return;
    };
    let Some(result) = tabs
        .peek()
        .iter()
        .find(|tab| tab.id == tab_id)
        .and_then(|tab| tab.output.clone())
    else {
        return;
    };
    let mut status = status;
    spawn(async move {
        match platform::save_bytes(&result.file.name, &result.file.data).await {
            Ok(true) => {
                push_application_log(
                    "WEM",
                    "INFO",
                    "EXPORT",
                    format!("Exported {}", result.file.name),
                );
                status.set(AppStatus::new("文件已导出。", StatusTone::Success));
            }
            Ok(false) => {}
            Err(error) => {
                push_application_log("WEM", "ERROR", "EXPORT", format!("Export failed: {error}"));
                status.set(AppStatus::new(error, StatusTone::Error));
            }
        }
    });
}

#[component]
fn EmptyWorkspace(busy: bool, on_files: EventHandler<Vec<FileData>>) -> Element {
    rsx! {
        section { class: "wem-empty-workspace",
            div { class: "wem-empty-visual", aria_hidden: "true",
                div { class: "wem-disc",
                    span {}
                    i {}
                }
                div { class: "wem-empty-wave",
                    for index in 0..17 {
                        i { style: "height: {18 + wave_height(index) / 3}%;" }
                    }
                }
            }
            div { class: "wem-empty-copy",
                span { class: "wem-panel-eyebrow", "AUDIO WORKSPACE" }
                h2 { "打开音频，立即播放或转换" }
                p { "WEM 可按内部编码导出为 WAV、Ogg 或 AAC；WAV、Ogg 与 AAC 也可以重新封装为 WEM。" }
                div { class: "wem-empty-formats",
                    span { "WEM" }
                    i { "↔" }
                    span { "WAV" }
                    span { "OGG" }
                    span { "M4A / AAC" }
                }
                label {
                    class: if busy { "wem-open-button is-disabled" } else { "wem-open-button" },
                    Glyph { name: "open" }
                    "选择音频"
                    input {
                        class: "wem-file-input",
                        r#type: "file",
                        accept: ".wem,.wav,.ogg,.oga,.m4a,.mp4,.aac,audio/*",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| {
                            let files = event.files();
                            if !files.is_empty() {
                                on_files.call(files);
                            }
                        },
                    }
                }
                small { "也可以将文件拖到此处" }
            }
        }
    }
}

#[component]
fn PanelHeading(eyebrow: &'static str, title: &'static str, glyph: &'static str) -> Element {
    rsx! {
        div { class: "wem-panel-heading",
            div {
                span { class: "wem-panel-eyebrow", "{eyebrow}" }
                h2 { "{title}" }
            }
            span { class: "wem-panel-heading-icon", Glyph { name: glyph } }
        }
    }
}

#[component]
fn MetadataRow(label: &'static str, value: String) -> Element {
    rsx! {
        div {
            dt { "{label}" }
            dd { "{value}" }
        }
    }
}

fn available_targets(audio: &AudioDocument) -> Vec<TargetOption> {
    match audio.info.format {
        SourceFormat::Wem => {
            let mut targets = vec![
                TargetOption {
                    target: ExportTarget::Automatic,
                    label: "自动（对应音频格式）",
                    detail: "根据 WEM 内部编码选择最合适的开放格式。",
                },
                TargetOption {
                    target: ExportTarget::Wav,
                    label: "WAV 音频",
                    detail: "解码为兼容性最好的 PCM WAV，适合播放与后期处理。",
                },
            ];
            if codec_is_vorbis(&audio.info.codec) {
                targets.push(TargetOption {
                    target: ExportTarget::Ogg,
                    label: "Ogg Vorbis",
                    detail: "恢复原始 Vorbis 音频流，不经过有损重编码。",
                });
            }
            if audio.info.codec.eq_ignore_ascii_case("AAC") {
                targets.push(TargetOption {
                    target: ExportTarget::Aac,
                    label: "M4A / AAC",
                    detail: "提取 WEM 中的 AAC 音频，不经过有损重编码。",
                });
            }
            targets
        }
        SourceFormat::Wav => vec![
            TargetOption {
                target: ExportTarget::Automatic,
                label: "自动（PCM WEM）",
                detail: "保留 PCM 采样并封装为 WEM，质量无损、体积较大。",
            },
            TargetOption {
                target: ExportTarget::PcmWem,
                label: "PCM WEM",
                detail: "保留 PCM 采样并封装为 WEM，质量无损、体积较大。",
            },
            TargetOption {
                target: ExportTarget::AdpcmWem,
                label: "ADPCM WEM",
                detail: "编码为 IMA ADPCM WEM，在质量与体积之间取得平衡。",
            },
        ],
        SourceFormat::Ogg => vec![
            TargetOption {
                target: ExportTarget::Automatic,
                label: "自动（Vorbis WEM）",
                detail: "将 Ogg Vorbis 音频无损重封装为 WEM。",
            },
            TargetOption {
                target: ExportTarget::VorbisWem,
                label: "Vorbis WEM",
                detail: "将 Ogg Vorbis 音频无损重封装为 WEM。",
            },
        ],
        SourceFormat::Aac => vec![
            TargetOption {
                target: ExportTarget::Automatic,
                label: "自动（AAC WEM）",
                detail: "探测 AAC 参数并将音频流封装为 WEM。",
            },
            TargetOption {
                target: ExportTarget::AacWem,
                label: "AAC WEM",
                detail: "探测 AAC 参数并将音频流封装为 WEM。",
            },
        ],
    }
}

fn target_short(target: ExportTarget, audio: &AudioDocument) -> &'static str {
    match target {
        ExportTarget::Automatic => match audio.info.format {
            SourceFormat::Wem if codec_is_vorbis(&audio.info.codec) => "OGG",
            SourceFormat::Wem if audio.info.codec.eq_ignore_ascii_case("AAC") => "M4A",
            SourceFormat::Wem => "WAV",
            SourceFormat::Wav | SourceFormat::Ogg | SourceFormat::Aac => "WEM",
        },
        ExportTarget::Wav => "WAV",
        ExportTarget::Ogg => "OGG",
        ExportTarget::Aac => "M4A",
        ExportTarget::PcmWem
        | ExportTarget::AdpcmWem
        | ExportTarget::VorbisWem
        | ExportTarget::AacWem => "WEM",
    }
}

const fn format_label(format: SourceFormat) -> &'static str {
    match format {
        SourceFormat::Wem => "Wwise WEM",
        SourceFormat::Wav => "WAV Audio",
        SourceFormat::Ogg => "Ogg Vorbis",
        SourceFormat::Aac => "AAC Audio",
    }
}

const fn format_short(format: SourceFormat) -> &'static str {
    match format {
        SourceFormat::Wem => "WEM",
        SourceFormat::Wav => "WAV",
        SourceFormat::Ogg => "OGG",
        SourceFormat::Aac => "AAC",
    }
}

fn channel_label(channels: u16) -> String {
    match channels {
        1 => "单声道".to_string(),
        2 => "立体声".to_string(),
        channels => format!("{channels} 声道"),
    }
}

fn sample_rate_label(sample_rate: u32) -> String {
    if sample_rate % 1_000 == 0 {
        format!("{} kHz", sample_rate / 1_000)
    } else {
        format!("{:.1} kHz", sample_rate as f64 / 1_000.0)
    }
}

fn codec_is_vorbis(codec: &str) -> bool {
    codec.to_ascii_lowercase().contains("vorbis")
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn wave_height(index: usize) -> usize {
    const HEIGHTS: [usize; 16] = [
        24, 42, 68, 36, 76, 52, 88, 45, 64, 30, 74, 56, 92, 48, 70, 34,
    ];
    HEIGHTS[index % HEIGHTS.len()]
}

fn source_glyph(format: SourceFormat) -> Element {
    match format {
        SourceFormat::Wem => rsx! { span { "W" } },
        SourceFormat::Wav => rsx! { span { "∿" } },
        SourceFormat::Ogg => rsx! { span { "O" } },
        SourceFormat::Aac => rsx! { span { "A" } },
    }
}

fn output_glyph(name: &str) -> Element {
    let glyph = if name.to_ascii_lowercase().ends_with(".wem") {
        "W"
    } else {
        "∿"
    };
    rsx! { span { "{glyph}" } }
}

#[component]
fn SkipGlyph(forward: bool) -> Element {
    let path = if forward {
        "M21 12a9 9 0 1 1-2.64-6.36L21 8M21 3v5h-5"
    } else {
        "M3 12a9 9 0 1 0 2.64-6.36L3 8M3 3v5h5"
    };
    rsx! {
        svg {
            class: "wem-skip-glyph",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.65",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "{path}" }
            text {
                x: "12",
                y: "12.25",
                "15"
            }
        }
    }
}

#[component]
fn Glyph(name: &'static str) -> Element {
    let path = match name {
        "open" => {
            "M3 7a3 3 0 0 1 3-3h4l2 2h6a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V7Zm0 3h18"
        }
        "close" => "M6 6l12 12M18 6 6 18",
        "audio" => "M9 18V5l10-2v13M9 18a3 3 0 1 1-3-3h3m10 1a3 3 0 1 1-3-3h3",
        "convert" => "M7 7h11l-3-3m3 3-3 3M17 17H6l3 3m-3-3 3-3",
        "download" => "M12 3v12m0 0 5-5m-5 5-5-5M4 20h16",
        "chevron" => "m8 10 4 4 4-4",
        "check" => "m5 12 4 4L19 6",
        "warning" => "M12 4 3 20h18L12 4Zm0 5v5m0 3h.01",
        "spark" => "m12 3 1.4 4.6L18 9l-4.6 1.4L12 15l-1.4-4.6L6 9l4.6-1.4L12 3Z",
        "play" => "M8 5v14l11-7L8 5Z",
        "pause" => "M9 5v14m6-14v14",
        "volume" => {
            "M4 10v4h4l5 4V6l-5 4H4m12-1c1 1 1.5 2 1.5 3S17 14 16 15m2-8c2 1.5 3 3.2 3 5s-1 3.5-3 5"
        }
        "muted" => "M4 10v4h4l5 4V6l-5 4H4m12 0 5 5m0-5-5 5",
        _ => "",
    };
    rsx! {
        svg {
            class: "wem-glyph",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "{path}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn formats_file_sizes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1_536), "1.5 KB");
    }
}
