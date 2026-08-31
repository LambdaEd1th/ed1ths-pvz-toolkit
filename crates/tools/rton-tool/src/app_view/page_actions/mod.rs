mod groups;

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdChevronDown, LdMenu, LdPanelRight};
use rton_editor_core::TextFormat;

use crate::components::{FileSelection, lucide_icon};
use crate::domain::{EditorMode, Status};
use crate::file_import::LoadedFileState;
use crate::i18n::I18n;
use crate::platform;

use groups::ActionGroupContent;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionGroupId {
    File,
    Edit,
    TextExport,
    RtonExport,
    Preferences,
}

impl ActionGroupId {
    fn code(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Edit => "edit",
            Self::TextExport => "text-export",
            Self::RtonExport => "rton-export",
            Self::Preferences => "preferences",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ModeMenuPosition {
    left: i32,
    top: i32,
    width: i32,
}

#[derive(Clone, PartialEq)]
struct ActionToolbarContext {
    i18n: I18n,
    active_mode: Option<EditorMode>,
    active_file_label: String,
    compact: bool,
    encrypt: bool,
    line_wrapping_enabled: bool,
    search_visible: bool,
    can_undo: bool,
    can_redo: bool,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    encrypt_output: Signal<bool>,
    line_wrapping: Signal<bool>,
    editor_search_panel_visible: Signal<bool>,
    status: Signal<Status>,
    open_native_files: EventHandler<()>,
    open_native_folder: EventHandler<()>,
    on_files_staged: EventHandler<()>,
    undo_edit: EventHandler<()>,
    redo_edit: EventHandler<()>,
    on_compact_change: EventHandler<bool>,
    export_text: EventHandler<TextFormat>,
    parse_current: EventHandler<()>,
    export_rton: EventHandler<()>,
}

const EDITOR_MODES: [EditorMode; 4] = [
    EditorMode::RtonHex,
    EditorMode::Json,
    EditorMode::Yaml,
    EditorMode::Toml,
];

const MENU_EXIT_MS: u64 = 200;

fn close_mode_menu(mut position: Signal<Option<ModeMenuPosition>>, mut closing: Signal<bool>) {
    if position.peek().is_none() || *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        platform::sleep_ms(MENU_EXIT_MS).await;
        position.set(None);
        closing.set(false);
    });
}

fn mode_mark(mode: Option<EditorMode>) -> &'static str {
    match mode.unwrap_or(EditorMode::RtonHex) {
        EditorMode::RtonHex => "R",
        EditorMode::Json => "J",
        EditorMode::Yaml => "Y",
        EditorMode::Toml => "T",
    }
}

async fn calculate_mode_menu_position(mounted: Option<MountedEvent>) -> ModeMenuPosition {
    let Some(event) = mounted else {
        return ModeMenuPosition {
            left: 12,
            top: 68,
            width: 112,
        };
    };
    let Ok(rect) = event.get_client_rect().await else {
        return ModeMenuPosition {
            left: 12,
            top: 68,
            width: 112,
        };
    };
    ModeMenuPosition {
        left: rect.origin.x.round().max(8.0) as i32,
        top: (rect.origin.y + rect.height() + 8.0).round().max(8.0) as i32,
        width: rect.width().round().max(1.0) as i32,
    }
}

#[component]
pub(super) fn PageActions(
    i18n: I18n,
    active_mode_snapshot: Option<EditorMode>,
    preferred_mode_snapshot: Option<EditorMode>,
    active_file_label: String,
    compact_snapshot: bool,
    encrypt_snapshot: bool,
    line_wrapping_snapshot: bool,
    editor_search_panel_visible_snapshot: bool,
    can_undo_snapshot: bool,
    can_redo_snapshot: bool,
    file_sheet_open: Signal<bool>,
    inspector_sheet_open: Signal<bool>,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    encrypt_output: Signal<bool>,
    line_wrapping: Signal<bool>,
    editor_search_panel_visible: Signal<bool>,
    status: Signal<Status>,
    open_native_files: EventHandler<()>,
    open_native_folder: EventHandler<()>,
    on_files_staged: EventHandler<()>,
    undo_edit: EventHandler<()>,
    redo_edit: EventHandler<()>,
    on_switch_mode: EventHandler<EditorMode>,
    on_compact_change: EventHandler<bool>,
    export_text: EventHandler<TextFormat>,
    parse_current: EventHandler<()>,
    export_rton: EventHandler<()>,
) -> Element {
    let mut mode_control_mounted = use_signal(|| None::<MountedEvent>);
    let mut mode_menu_position = use_signal(|| None::<ModeMenuPosition>);
    let mut mode_menu_closing = use_signal(|| false);
    let mode_menu_position_snapshot = *mode_menu_position.read();
    let mode_menu_closing_snapshot = *mode_menu_closing.read();
    let file_sheet_open_snapshot = *file_sheet_open.read();
    let inspector_sheet_open_snapshot = *inspector_sheet_open.read();
    let mode_menu_open = mode_menu_position_snapshot.is_some() && !mode_menu_closing_snapshot;
    let selector_mode = active_mode_snapshot
        .or(preferred_mode_snapshot)
        .unwrap_or(EditorMode::RtonHex);
    let active_mode_label = selector_mode.label();
    let mode_selector_label = i18n.t("toolbar-group-format");
    let action_toolbar_context = ActionToolbarContext {
        i18n,
        active_mode: active_mode_snapshot,
        active_file_label: active_file_label.clone(),
        compact: compact_snapshot,
        encrypt: encrypt_snapshot,
        line_wrapping_enabled: line_wrapping_snapshot,
        search_visible: editor_search_panel_visible_snapshot,
        can_undo: can_undo_snapshot,
        can_redo: can_redo_snapshot,
        loaded_files,
        next_loaded_file_id,
        file_selection,
        encrypt_output,
        line_wrapping,
        editor_search_panel_visible,
        status,
        open_native_files,
        open_native_folder,
        on_files_staged,
        undo_edit,
        redo_edit,
        on_compact_change,
        export_text,
        parse_current,
        export_rton,
    };

    rsx! {
        div { class: "ui-island ui-tool-page-actions rton-page-actions",
            button {
                r#type: "button",
                class: if file_sheet_open_snapshot { "rton-page-icon-button active" } else { "rton-page-icon-button" },
                title: i18n.t("file-list-title"),
                aria_label: i18n.t("file-list-title"),
                aria_expanded: file_sheet_open_snapshot,
                onclick: move |_| {
                    super::set_file_sheet_open(
                        !file_sheet_open_snapshot,
                        file_sheet_open,
                        inspector_sheet_open,
                    );
                },
                {lucide_icon(LdMenu)}
            }
            button {
                r#type: "button",
                class: if mode_menu_open { "rton-mode-pill is-open" } else { "rton-mode-pill" },
                title: "{mode_selector_label}: {active_mode_label}",
                aria_label: "{mode_selector_label}: {active_mode_label}",
                aria_haspopup: "listbox",
                aria_expanded: mode_menu_open,
                onmounted: move |event| mode_control_mounted.set(Some(event)),
                onclick: move |_| {
                    if mode_menu_position.peek().is_some() {
                        close_mode_menu(mode_menu_position, mode_menu_closing);
                    } else {
                        let mounted = mode_control_mounted.peek().clone();
                        spawn(async move {
                            mode_menu_closing.set(false);
                            mode_menu_position.set(Some(calculate_mode_menu_position(mounted).await));
                        });
                    }
                },
                span { class: "rton-mode-mark", aria_hidden: "true", {mode_mark(Some(selector_mode))} }
                span { class: "rton-mode-label", "{active_mode_label}" }
                span { class: "rton-mode-caret", aria_hidden: "true", {lucide_icon(LdChevronDown)} }
            }
            div { class: "rton-document-pill", title: "{active_file_label}",
                span { class: "rton-document-dot" }
                span { class: "rton-document-name", "{active_file_label}" }
            }
            button {
                r#type: "button",
                class: if inspector_sheet_open_snapshot { "rton-page-icon-button active" } else { "rton-page-icon-button" },
                title: i18n.t("panel-inspector-tabs"),
                aria_label: i18n.t("panel-inspector-tabs"),
                aria_expanded: inspector_sheet_open_snapshot,
                onclick: move |_| {
                    super::set_inspector_sheet_open(
                        !inspector_sheet_open_snapshot,
                        file_sheet_open,
                        inspector_sheet_open,
                    );
                },
                {lucide_icon(LdPanelRight)}
            }
            ActionGroups { context: action_toolbar_context.clone() }
        }

        if let Some(position) = mode_menu_position_snapshot {
            div {
                class: if mode_menu_closing_snapshot { "rton-mode-menu-backdrop closing" } else { "rton-mode-menu-backdrop" },
                onmousedown: move |_| close_mode_menu(mode_menu_position, mode_menu_closing)
            }
            div {
                class: if mode_menu_closing_snapshot { "rton-mode-menu closing" } else { "rton-mode-menu" },
                role: "listbox",
                aria_label: "{mode_selector_label}",
                style: "left: {position.left}px; top: {position.top}px; width: {position.width}px",
                onmousedown: move |event| event.stop_propagation(),
                for mode in EDITOR_MODES {
                    button {
                        key: "{mode.label()}",
                        r#type: "button",
                        class: if selector_mode == mode { "active" } else { "" },
                        role: "option",
                        aria_selected: selector_mode == mode,
                        onclick: move |_| {
                            close_mode_menu(mode_menu_position, mode_menu_closing);
                            on_switch_mode.call(mode);
                        },
                        span { class: "rton-mode-menu-mark", {mode_mark(Some(mode))} }
                        span { "{mode.label()}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ActionGroups(context: ActionToolbarContext) -> Element {
    const GROUPS: [ActionGroupId; 5] = [
        ActionGroupId::File,
        ActionGroupId::Edit,
        ActionGroupId::TextExport,
        ActionGroupId::RtonExport,
        ActionGroupId::Preferences,
    ];

    rsx! {
        div { class: "rton-action-groups",
            for group_id in GROUPS {
                section {
                    key: "{group_id.code()}",
                    class: "rton-action-group",
                    "data-action-group": "{group_id.code()}",
                    ActionGroupContent {
                        group_id,
                        i18n: context.i18n,
                        active_mode_snapshot: context.active_mode,
                        active_file_label: context.active_file_label.clone(),
                        compact_snapshot: context.compact,
                        encrypt_snapshot: context.encrypt,
                        line_wrapping_snapshot: context.line_wrapping_enabled,
                        editor_search_panel_visible_snapshot: context.search_visible,
                        can_undo_snapshot: context.can_undo,
                        can_redo_snapshot: context.can_redo,
                        loaded_files: context.loaded_files,
                        next_loaded_file_id: context.next_loaded_file_id,
                        file_selection: context.file_selection,
                        encrypt_output: context.encrypt_output,
                        line_wrapping: context.line_wrapping,
                        editor_search_panel_visible: context.editor_search_panel_visible,
                        status: context.status,
                        open_native_files: context.open_native_files,
                        open_native_folder: context.open_native_folder,
                        on_files_staged: context.on_files_staged,
                        undo_edit: context.undo_edit,
                        redo_edit: context.redo_edit,
                        on_compact_change: context.on_compact_change,
                        export_text: context.export_text,
                        parse_current: context.parse_current,
                        export_rton: context.export_rton,
                    }
                }
            }
        }
    }
}
