mod groups;

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdChevronDown, LdEllipsis, LdFilePlus2, LdMenu, LdPanelRight, LdRedo2, LdSearch, LdSettings,
    LdUndo2, LdX,
};

use crate::components::{FileSelection, ToolbarGroup, lucide_icon};
use crate::domain::{DropMarker, EditorMode, Status, ToolbarDropTarget, ToolbarGroupId};
use crate::file_import::LoadedFileState;
use crate::i18n::{I18n, LanguageOption, Locale};
use crate::platform;
use rton_editor_core::TextFormat;

use groups::{SettingsDialog, ToolbarGroupContent, WebFileOpenControl};

#[derive(Clone, Copy, PartialEq, Eq)]
struct ModeMenuPosition {
    left: i32,
    top: i32,
    width: i32,
}

#[derive(Clone, PartialEq)]
struct ToolbarGroupRowsContext {
    toolbar_rows: Vec<Vec<ToolbarGroupId>>,
    dragged_group_id: Option<ToolbarGroupId>,
    drop_marker: Option<DropMarker<ToolbarGroupId>>,
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
    update_drop_target: EventHandler<ToolbarDropTarget>,
    finish_drag: EventHandler<()>,
    start_group_drag: EventHandler<ToolbarGroupId>,
    open_native_files: EventHandler<()>,
    open_native_folder: EventHandler<()>,
    on_files_staged: EventHandler<()>,
    undo_edit: EventHandler<()>,
    redo_edit: EventHandler<()>,
    on_compact_change: EventHandler<bool>,
    export_text: EventHandler<TextFormat>,
    parse_current: EventHandler<()>,
    export_rton: EventHandler<()>,
    on_dismiss: EventHandler<()>,
}

const EDITOR_MODES: [EditorMode; 4] = [
    EditorMode::RtonHex,
    EditorMode::Json,
    EditorMode::Yaml,
    EditorMode::Toml,
];

const MENU_EXIT_MS: u64 = 200;
const MORE_MENU_EXIT_MS: u64 = 170;
const DIALOG_EXIT_MS: u64 = 200;

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

fn close_more_menu(mut mounted: Signal<bool>, mut closing: Signal<bool>) {
    if !*mounted.peek() || *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        platform::sleep_ms(MORE_MENU_EXIT_MS).await;
        mounted.set(false);
        closing.set(false);
    });
}

fn close_settings_dialog(mut mounted: Signal<bool>, mut closing: Signal<bool>) {
    if !*mounted.peek() || *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        platform::sleep_ms(DIALOG_EXIT_MS).await;
        mounted.set(false);
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
pub(super) fn ToolbarView(
    i18n: I18n,
    toolbar_rows_snapshot: Vec<Vec<ToolbarGroupId>>,
    dragged_toolbar_group_id_snapshot: Option<ToolbarGroupId>,
    toolbar_drop_marker_snapshot: Option<DropMarker<ToolbarGroupId>>,
    active_mode_snapshot: Option<EditorMode>,
    preferred_mode_snapshot: Option<EditorMode>,
    active_file_label: String,
    compact_snapshot: bool,
    encrypt_snapshot: bool,
    locale_snapshot: Locale,
    language_options: Vec<LanguageOption>,
    line_wrapping_snapshot: bool,
    editor_search_panel_visible_snapshot: bool,
    can_undo_snapshot: bool,
    can_redo_snapshot: bool,
    file_drawer_open: Signal<bool>,
    inspector_drawer_open: Signal<bool>,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    encrypt_output: Signal<bool>,
    locale: Signal<Locale>,
    line_wrapping: Signal<bool>,
    editor_search_panel_visible: Signal<bool>,
    status: Signal<Status>,
    update_toolbar_drop_target: EventHandler<ToolbarDropTarget>,
    finish_toolbar_drag: EventHandler<()>,
    start_toolbar_group_drag: EventHandler<ToolbarGroupId>,
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
    let mut mobile_menu_open = use_signal(|| false);
    let mut mobile_menu_closing = use_signal(|| false);
    let mut settings_mounted = use_signal(|| false);
    let mut settings_closing = use_signal(|| false);
    let mut mode_control_mounted = use_signal(|| None::<MountedEvent>);
    let mut mode_menu_position = use_signal(|| None::<ModeMenuPosition>);
    let mut mode_menu_closing = use_signal(|| false);
    let mobile_menu_open_snapshot = *mobile_menu_open.read();
    let mobile_menu_closing_snapshot = *mobile_menu_closing.read();
    let settings_mounted_snapshot = *settings_mounted.read();
    let settings_closing_snapshot = *settings_closing.read();
    let mode_menu_position_snapshot = *mode_menu_position.read();
    let mode_menu_closing_snapshot = *mode_menu_closing.read();
    let file_drawer_open_snapshot = *file_drawer_open.read();
    let inspector_drawer_open_snapshot = *inspector_drawer_open.read();
    let mode_menu_open = mode_menu_position_snapshot.is_some() && !mode_menu_closing_snapshot;
    let mobile_menu_active = mobile_menu_open_snapshot && !mobile_menu_closing_snapshot;
    let settings_open = settings_mounted_snapshot && !settings_closing_snapshot;
    let has_active_document = active_mode_snapshot.is_some();
    let selector_mode = active_mode_snapshot
        .or(preferred_mode_snapshot)
        .unwrap_or(EditorMode::RtonHex);
    let active_mode_label = selector_mode.label();
    let mode_selector_label = i18n.t("toolbar-group-format");
    let toolbar_group_rows_context = ToolbarGroupRowsContext {
        toolbar_rows: toolbar_rows_snapshot,
        dragged_group_id: dragged_toolbar_group_id_snapshot,
        drop_marker: toolbar_drop_marker_snapshot,
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
        update_drop_target: update_toolbar_drop_target,
        finish_drag: finish_toolbar_drag,
        start_group_drag: start_toolbar_group_drag,
        open_native_files,
        open_native_folder,
        on_files_staged,
        undo_edit,
        redo_edit,
        on_compact_change,
        export_text,
        parse_current,
        export_rton,
        on_dismiss: EventHandler::new(move |_| {
            close_more_menu(mobile_menu_open, mobile_menu_closing)
        }),
    };

    rsx! {
        header {
            class: match (
                dragged_toolbar_group_id_snapshot.is_some(),
                mobile_menu_open_snapshot,
            ) {
                (true, true) => "rton-toolbar rton-commandbar ui-island dragging-toolbar mobile-menu-open",
                (true, false) => "rton-toolbar rton-commandbar ui-island dragging-toolbar",
                (false, true) => "rton-toolbar rton-commandbar ui-island mobile-menu-open",
                (false, false) => "rton-toolbar rton-commandbar ui-island",
            },
            button {
                r#type: "button",
                class: if file_drawer_open_snapshot { "rton-command-icon-button rton-sidebar-toggle active" } else { "rton-command-icon-button rton-sidebar-toggle" },
                title: i18n.t("file-list-title"),
                aria_label: i18n.t("file-list-title"),
                aria_controls: "rton-file-drawer",
                aria_expanded: file_drawer_open_snapshot,
                onclick: move |_| {
                    super::set_file_drawer_visibility(
                        !file_drawer_open_snapshot,
                        file_drawer_open,
                        inspector_drawer_open,
                    );
                },
                {lucide_icon(LdMenu)}
            }
            button {
                r#type: "button",
                class: if mode_menu_open { "rton-commandbar-identity is-open" } else { "rton-commandbar-identity" },
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
                            mode_menu_position
                                .set(Some(calculate_mode_menu_position(mounted).await));
                        });
                    }
                },
                span { class: "rton-commandbar-mark", aria_hidden: "true", {mode_mark(Some(selector_mode))} }
                span { class: "rton-commandbar-product", "{active_mode_label}" }
                span { class: "rton-commandbar-mode-caret", aria_hidden: "true", {lucide_icon(LdChevronDown)} }
            }
            div { class: "rton-commandbar-document", title: "{active_file_label}",
                span { class: "rton-commandbar-document-dot" }
                span { class: "rton-commandbar-document-name", "{active_file_label}" }
            }
            div { class: "rton-mobile-command-actions",
                if cfg!(target_arch = "wasm32") {
                    WebFileOpenControl {
                        i18n,
                        class_name: "rton-command-icon-button primary rton-mobile-file-open".to_string(),
                        compact: true,
                        loaded_files,
                        next_loaded_file_id,
                        file_selection,
                        status,
                        on_files_staged
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "rton-command-icon-button primary",
                        title: i18n.t("toolbar-open"),
                        aria_label: i18n.t("toolbar-open"),
                        onclick: move |_| open_native_files.call(()),
                        {lucide_icon(LdFilePlus2)}
                    }
                }
                button {
                    r#type: "button",
                    class: "rton-command-icon-button",
                    disabled: !can_undo_snapshot,
                    title: i18n.t("toolbar-undo"),
                    aria_label: i18n.t("toolbar-undo"),
                    onclick: move |_| undo_edit.call(()),
                    {lucide_icon(LdUndo2)}
                }
                button {
                    r#type: "button",
                    class: "rton-command-icon-button",
                    disabled: !can_redo_snapshot,
                    title: i18n.t("toolbar-redo"),
                    aria_label: i18n.t("toolbar-redo"),
                    onclick: move |_| redo_edit.call(()),
                    {lucide_icon(LdRedo2)}
                }
                button {
                    r#type: "button",
                    class: if editor_search_panel_visible_snapshot { "rton-command-icon-button active" } else { "rton-command-icon-button" },
                    disabled: !has_active_document,
                    title: i18n.t("toolbar-search"),
                    aria_label: i18n.t("toolbar-search"),
                    aria_pressed: editor_search_panel_visible_snapshot,
                    onclick: move |_| editor_search_panel_visible.set(!editor_search_panel_visible_snapshot),
                    {lucide_icon(LdSearch)}
                }
                button {
                    r#type: "button",
                    class: if mobile_menu_active { "rton-command-icon-button active" } else { "rton-command-icon-button" },
                    title: i18n.t("toolbar-more"),
                    aria_label: i18n.t("toolbar-more"),
                    aria_expanded: mobile_menu_active,
                    onclick: move |_| {
                        if mobile_menu_open_snapshot {
                            close_more_menu(mobile_menu_open, mobile_menu_closing);
                        } else {
                            mobile_menu_closing.set(false);
                            mobile_menu_open.set(true);
                        }
                    },
                    {lucide_icon(LdEllipsis)}
                }
            }
            div { class: "rton-commandbar-groups rton-commandbar-groups-inline",
                ToolbarGroupRows { context: toolbar_group_rows_context.clone() }
            }
            if mobile_menu_open_snapshot {
                div {
                    class: if mobile_menu_closing_snapshot {
                        "rton-commandbar-groups rton-commandbar-more-menu mobile-open closing"
                    } else {
                        "rton-commandbar-groups rton-commandbar-more-menu mobile-open"
                    },
                    button {
                        r#type: "button",
                        class: "rton-commandbar-menu-backdrop",
                        aria_label: i18n.t("toolbar-close-menu"),
                        onclick: move |_| close_more_menu(mobile_menu_open, mobile_menu_closing)
                    }
                    div { class: "rton-commandbar-menu-header",
                        div {
                            strong { {i18n.t("toolbar-more")} }
                        }
                        button {
                            r#type: "button",
                            class: "rton-command-icon-button",
                            title: i18n.t("toolbar-close-menu"),
                            aria_label: i18n.t("toolbar-close-menu"),
                            onclick: move |_| close_more_menu(mobile_menu_open, mobile_menu_closing),
                            {lucide_icon(LdX)}
                        }
                    }
                    ToolbarGroupRows { context: toolbar_group_rows_context.clone() }
                }
            }
            button {
                r#type: "button",
                class: if inspector_drawer_open_snapshot { "rton-command-icon-button rton-inspector-toggle active" } else { "rton-command-icon-button rton-inspector-toggle" },
                title: i18n.t("panel-inspector-tabs"),
                aria_label: i18n.t("panel-inspector-tabs"),
                aria_controls: "rton-inspector-drawer",
                aria_expanded: inspector_drawer_open_snapshot,
                onclick: move |_| {
                    super::set_inspector_drawer_visibility(
                        !inspector_drawer_open_snapshot,
                        file_drawer_open,
                        inspector_drawer_open,
                    );
                },
                {lucide_icon(LdPanelRight)}
            }
            button {
                r#type: "button",
                class: if settings_open { "rton-command-icon-button rton-settings-button active" } else { "rton-command-icon-button rton-settings-button" },
                title: i18n.t("settings-title"),
                aria_label: i18n.t("settings-title"),
                aria_haspopup: "dialog",
                aria_expanded: settings_open,
                onclick: move |_| {
                    close_more_menu(mobile_menu_open, mobile_menu_closing);
                    close_mode_menu(mode_menu_position, mode_menu_closing);
                    settings_closing.set(false);
                    settings_mounted.set(true);
                },
                {lucide_icon(LdSettings)}
            }
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
        if settings_mounted_snapshot {
            SettingsDialog {
                i18n,
                closing: settings_closing_snapshot,
                locale_snapshot,
                language_options: language_options.clone(),
                locale,
                status,
                on_close: move |_| close_settings_dialog(settings_mounted, settings_closing)
            }
        }
    }
}

#[component]
fn ToolbarGroupRows(context: ToolbarGroupRowsContext) -> Element {
    let ToolbarGroupRowsContext {
        toolbar_rows,
        dragged_group_id,
        drop_marker,
        i18n,
        active_mode,
        active_file_label,
        compact,
        encrypt,
        line_wrapping_enabled,
        search_visible,
        can_undo,
        can_redo,
        loaded_files,
        next_loaded_file_id,
        file_selection,
        encrypt_output,
        line_wrapping,
        editor_search_panel_visible,
        status,
        update_drop_target,
        finish_drag,
        start_group_drag,
        open_native_files,
        open_native_folder,
        on_files_staged,
        undo_edit,
        redo_edit,
        on_compact_change,
        export_text,
        parse_current,
        export_rton,
        on_dismiss,
    } = context;

    rsx! {
        div { class: "rton-commandbar-group-scroll",
            for (row_index, row) in toolbar_rows.iter().cloned().enumerate() {
                div {
                    key: "toolbar-row-{row_index}",
                    class: "rton-toolbar-row",
                    onmousemove: move |event| {
                        event.stop_propagation();
                        update_drop_target.call(ToolbarDropTarget::RowEnd { row_index });
                    },
                    onmouseup: move |_| finish_drag.call(()),
                    for group_id in row {
                        ToolbarGroup {
                            key: "{group_id.code()}",
                            id: group_id,
                            label: i18n.t(group_id.label_key()),
                            i18n,
                            dragging: dragged_group_id == Some(group_id),
                            drop_placement: drop_marker
                                .filter(|marker| marker.id == group_id)
                                .map(|marker| marker.placement),
                            on_drag_start: start_group_drag,
                            on_drop_target: update_drop_target,
                            on_drag_end: finish_drag,
                            ToolbarGroupContent {
                                group_id,
                                i18n,
                                active_mode_snapshot: active_mode,
                                active_file_label: active_file_label.clone(),
                                compact_snapshot: compact,
                                encrypt_snapshot: encrypt,
                                line_wrapping_snapshot: line_wrapping_enabled,
                                editor_search_panel_visible_snapshot: search_visible,
                                can_undo_snapshot: can_undo,
                                can_redo_snapshot: can_redo,
                                loaded_files,
                                next_loaded_file_id,
                                file_selection,
                                encrypt_output,
                                line_wrapping,
                                editor_search_panel_visible,
                                status,
                                open_native_files: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    open_native_files.call(());
                                }),
                                open_native_folder: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    open_native_folder.call(());
                                }),
                                on_files_staged: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    on_files_staged.call(());
                                }),
                                undo_edit: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    undo_edit.call(());
                                }),
                                redo_edit: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    redo_edit.call(());
                                }),
                                on_compact_change,
                                export_text: EventHandler::new(move |format| {
                                    on_dismiss.call(());
                                    export_text.call(format);
                                }),
                                parse_current: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    parse_current.call(());
                                }),
                                export_rton: EventHandler::new(move |_| {
                                    on_dismiss.call(());
                                    export_rton.call(());
                                })
                            }
                        }
                    }
                }
            }
        }
    }
}
